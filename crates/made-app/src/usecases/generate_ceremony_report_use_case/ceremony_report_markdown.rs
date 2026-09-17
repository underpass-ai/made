//! Rendering a report's Markdown.
//!
//! The projection itself (ADR-006): persisted state in, one document
//! out, nothing stored. It lived in the MCP adapter until parity slice
//! F3c, which is why a client pointed at a cluster could not ask for a
//! report at all. Both editions now render through here, so the same
//! session reports the same bytes whichever engine served the call.
//!
//! Every section is the serde form of what the engine actually holds,
//! fenced as JSON. That is deliberate: a report is evidence, and a
//! prose summary of evidence is not the evidence.

use std::fmt::Write as _;

use made_core::entities::{AuditRecord, CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use made_core::error::DomainError;
use made_core::value_objects::CeremonyDefinitionDigest;

/// One session as the report reads it.
pub(super) struct ReportedSession {
    pub(super) definition: CeremonyDefinition,
    pub(super) instance: CeremonyInstance,
    pub(super) journal: Vec<AuditRecord>,
    pub(super) completed: bool,
    pub(super) digest: CeremonyDefinitionDigest,
}

/// The whole document, heading first.
///
/// Fails only where serialization does, which is the engine's own
/// doing and not the caller's — hence `InvariantViolated` rather than
/// anything the caller could act on.
pub(super) fn render_markdown(
    title: Option<&str>,
    sessions: &[ReportedSession],
) -> Result<String, DomainError> {
    let mut markdown = String::new();
    markdown.push_str("# ");
    markdown.push_str(&safe_heading(title.unwrap_or("Ceremony report")));
    markdown.push_str("\n\n");
    write!(
        markdown,
        "Ceremonies: {} · completed: {} · incomplete: {}\n\n",
        sessions.len(),
        sessions.iter().filter(|session| session.completed).count(),
        sessions.iter().filter(|session| !session.completed).count()
    )
    .expect("writing to a String cannot fail");

    for session in sessions {
        let instance = &session.instance;
        markdown.push_str("## Ceremony `");
        markdown.push_str(instance.id().as_str());
        markdown.push_str("`\n\n");
        write!(
            markdown,
            "- Definition: `{}`\n- Version: `{}`\n- Definition digest: `{}`\n- Bound published digest: {}\n- State: `{}`\n- Status: `{}`\n- Created at: `{}`\n- Updated at: `{}`\n- Completed at: {}\n\n",
            session.definition.name(),
            session.definition.version(),
            session.digest,
            instance.bound_definition().map_or_else(|| "not bound".to_owned(), |digest| format!("`{digest}`")),
            instance.current_state(),
            if session.completed { "completed" } else { "incomplete" },
            instance.created_at(),
            instance.updated_at(),
            instance.completed_at().map_or_else(|| "not available".to_owned(), |at| format!("`{at}`")),
        )
        .expect("writing to a String cannot fail");

        if let Some(imported_at) = imported_at(&session.journal) {
            write!(
                markdown,
                "> Imported from a store written before ceremonies were event streams, at \
                 `{imported_at}`. What happened before the import was recorded without \
                 payloads and cannot be recovered, so the journal below opens with the \
                 import and the state above is what the earlier store held.\n\n"
            )
            .expect("writing to a String cannot fail");
        }

        section(&mut markdown, "Definition", &session.definition)?;
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
        section(&mut markdown, "Audit journal", &session.journal)?;
    }
    Ok(markdown)
}

/// When the session's stream opens with an import rather than a start.
///
/// The report says so because the sections above it are true and the
/// ones below it are short: an imported session's state is whatever
/// the old store held, and its journal begins the day it was imported.
/// A report that did not say so would read as though nothing had ever
/// happened before that record.
fn imported_at(journal: &[AuditRecord]) -> Option<&time::OffsetDateTime> {
    match journal.first()?.event()? {
        CeremonyEvent::InstanceImported(imported) => Some(&imported.imported_at),
        _ => None,
    }
}

fn section<T: serde::Serialize + ?Sized>(
    markdown: &mut String,
    heading: &str,
    value: &T,
) -> Result<(), DomainError> {
    let json = serde_json::to_string_pretty(value).map_err(|_| DomainError::InvariantViolated {
        reason: "a ceremony report section cannot be rendered",
    })?;
    markdown.push_str("### ");
    markdown.push_str(heading);
    markdown.push_str("\n\n");
    fenced_json(markdown, &json);
    markdown.push('\n');
    Ok(())
}

/// A fence longer than the longest backtick run inside it.
///
/// What a step or an evidence pack carries is the host's, and a host
/// can write backticks. A three-backtick fence around a payload that
/// contains one would end the block early and let the payload's own
/// text out into the document.
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

/// The caller's own title, escaped as the untrusted Markdown it is.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_is_escaped_as_untrusted_markdown() {
        assert_eq!(
            safe_heading("Review <unsafe> # heading"),
            "Review &lt;unsafe&gt; \\# heading"
        );
        assert_eq!(safe_heading("two\nlines"), "two lines");
    }

    #[test]
    fn a_fence_outgrows_the_longest_backtick_run_it_wraps() {
        let mut markdown = String::new();
        fenced_json(&mut markdown, "\"a ```fenced``` answer\"");

        assert!(markdown.starts_with("````json\n"));
        assert!(markdown.ends_with("````\n\n"));
    }

    #[test]
    fn a_payload_without_backticks_still_gets_three() {
        let mut markdown = String::new();
        fenced_json(&mut markdown, "{}");

        assert!(markdown.starts_with("```json\n"));
    }
}
