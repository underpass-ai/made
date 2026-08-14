use std::fmt::Write as _;

use choreo_app::usecases::CeremonyInstanceView;
use choreo_core::entities::{AuditRecord, CeremonyDefinition, CeremonyInstance};
use choreo_core::value_objects::{CeremonyDefinitionDigest, CeremonyId};
use choreo_embedded::EmbeddedChoreographer;
use serde_json::{json, Value};

use super::embedded_generate_ceremony_report_request::EmbeddedGenerateCeremonyReportRequest;

#[derive(Debug)]
struct ReportInstance {
    definition: CeremonyDefinition,
    instance: CeremonyInstance,
    journal: Vec<AuditRecord>,
    completed: bool,
    digest: CeremonyDefinitionDigest,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct EmbeddedCeremonyReportPresenter;

impl EmbeddedCeremonyReportPresenter {
    pub(super) async fn present(
        choreographer: &EmbeddedChoreographer,
        request: &EmbeddedGenerateCeremonyReportRequest,
    ) -> Result<Value, String> {
        let mut reports = Vec::with_capacity(request.ceremony_ids().len());
        for ceremony_id in request.ceremony_ids() {
            reports.push(Self::load(choreographer, ceremony_id).await?);
        }

        let completed_count = reports.iter().filter(|report| report.completed).count();
        let report_markdown = render_markdown(request.title(), &reports)?;
        Ok(json!({
            "report_markdown": report_markdown,
            "ceremony_ids": request.ceremony_ids().iter().map(CeremonyId::as_str).collect::<Vec<_>>(),
            "ceremony_count": reports.len(),
            "completed_count": completed_count,
            "incomplete_count": reports.len() - completed_count,
            "definition_bindings": reports.iter().map(|report| json!({
                "ceremony_id": report.instance.id().as_str(),
                "definition_name": report.definition.name().as_str(),
                "definition_version": report.definition.version().as_str(),
                "definition_digest": report.digest.to_hex(),
                "bound_definition_digest": report.instance.bound_definition().map(CeremonyDefinitionDigest::to_hex),
            })).collect::<Vec<_>>(),
            "persisted": false,
        }))
    }

    async fn load(
        choreographer: &EmbeddedChoreographer,
        ceremony_id: &CeremonyId,
    ) -> Result<ReportInstance, String> {
        let instance = choreographer.instance(ceremony_id).await.map_err(|error| {
            format!("ceremony instance `{ceremony_id}` could not be loaded: {error}")
        })?;
        let definition = choreographer
            .definition_for(&instance)
            .await
            .map_err(|error| {
                format!("definition for ceremony `{ceremony_id}` could not be loaded: {error}")
            })?;
        let completed = CeremonyInstanceView::project(&instance, &definition)
            .map_err(|error| {
                format!("ceremony instance `{ceremony_id}` could not be projected: {error}")
            })?
            .is_completed();
        let digest = definition.digest().map_err(|error| {
            format!("definition for ceremony `{ceremony_id}` has no digest: {error}")
        })?;
        let journal = choreographer
            .audit_records(ceremony_id)
            .await
            .map_err(|error| {
                format!("journal for ceremony `{ceremony_id}` could not be loaded: {error}")
            })?;
        Ok(ReportInstance {
            definition,
            instance,
            journal,
            completed,
            digest,
        })
    }
}

fn render_markdown(title: Option<&str>, reports: &[ReportInstance]) -> Result<String, String> {
    let mut markdown = String::new();
    markdown.push_str("# ");
    markdown.push_str(&safe_heading(title.unwrap_or("Ceremony report")));
    markdown.push_str("\n\n");
    write!(
        markdown,
        "Ceremonies: {} · completed: {} · incomplete: {}\n\n",
        reports.len(),
        reports.iter().filter(|report| report.completed).count(),
        reports.iter().filter(|report| !report.completed).count()
    )
    .expect("writing to a String cannot fail");

    for report in reports {
        let instance = &report.instance;
        markdown.push_str("## Ceremony `");
        markdown.push_str(instance.id().as_str());
        markdown.push_str("`\n\n");
        write!(
            markdown,
            "- Definition: `{}`\n- Version: `{}`\n- Definition digest: `{}`\n- Bound published digest: {}\n- State: `{}`\n- Status: `{}`\n- Created at: `{}`\n- Updated at: `{}`\n- Completed at: {}\n\n",
            report.definition.name(),
            report.definition.version(),
            report.digest,
            instance.bound_definition().map_or_else(|| "not bound".to_owned(), |digest| format!("`{digest}`")),
            instance.current_state(),
            if report.completed { "completed" } else { "incomplete" },
            instance.created_at(),
            instance.updated_at(),
            instance.completed_at().map_or_else(|| "not available".to_owned(), |at| format!("`{at}`")),
        )
        .expect("writing to a String cannot fail");

        section(&mut markdown, "Definition", &report.definition)?;
        section(&mut markdown, "Steps and outputs", instance.step_records())?;
        section(&mut markdown, "Transitions", instance.transitions())?;
        section(&mut markdown, "Guard approvals", instance.guard_approvals())?;
        section(&mut markdown, "Guard deferrals", instance.guard_deferrals())?;
        section(
            &mut markdown,
            "Interventions and evidence",
            instance.interventions(),
        )?;
        section(&mut markdown, "Reasons", instance.reasons())?;
        section(&mut markdown, "Audit journal", &report.journal)?;
    }
    Ok(markdown)
}

fn section<T: serde::Serialize + ?Sized>(
    markdown: &mut String,
    heading: &str,
    value: &T,
) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| format!("report section `{heading}` could not be rendered: {error}"))?;
    markdown.push_str("### ");
    markdown.push_str(heading);
    markdown.push_str("\n\n");
    fenced_json(markdown, &json);
    markdown.push('\n');
    Ok(())
}

fn fenced_json(markdown: &mut String, json: &str) {
    let longest = json
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(longest.saturating_add(1).max(3));
    markdown.push_str(&fence);
    markdown.push_str("json\n");
    markdown.push_str(json);
    markdown.push('\n');
    markdown.push_str(&fence);
    markdown.push_str("\n\n");
}

fn safe_heading(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\r' | '\n' => escaped.push(' '),
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '(' | ')' | '#' | '+' | '-' | '.'
            | '!' | '|' => {
                escaped.push('\\');
                escaped.push(character);
            }
            _ => escaped.push(character),
        }
    }
    escaped
}
