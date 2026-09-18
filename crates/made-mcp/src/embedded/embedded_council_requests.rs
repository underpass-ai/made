use std::collections::BTreeMap;

use made_app::usecases::{CreateCouncilInput, RunCouncilDecisionInput};
use made_core::entities::{Task, TaskConstraints};
use made_core::events::{EventEnvelope, TriggerEvent};
use made_core::ports::AgentDescriptor;
use made_core::value_objects::{
    AgentId, AgentKind, Attributes, CouncilId, CouncilSelector, DurationMs, EventId, NumAgents,
    OutputContract, OutputContractId, OutputFieldRule, OutputFormat, Rounds, Rubric, Specialty,
    TaskDescription, TaskId, ValidationMode,
};
use serde_json::{Map, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use super::embedded_council_context::{external_context, metadata};

pub(super) fn task(arguments: &Value) -> Result<Task, String> {
    let root = object(arguments, "tools/call.arguments")?;
    task_value(required(root, "task")?)
}

pub(super) fn task_value(value: &Value) -> Result<Task, String> {
    let task = object(value, "task")?;
    Ok(Task::new_with_metadata(
        TaskId::new(string(task, "task_id")?).map_err(domain)?,
        Specialty::new(string(task, "specialty")?).map_err(domain)?,
        TaskDescription::new(optional_string(task, "description").unwrap_or_default())
            .map_err(domain)?,
        constraints(task.get("constraints"))?,
        attributes(task.get("attributes"), "task.attributes")?,
        external_context(task.get("external_context"))?,
        metadata(task.get("metadata"))?,
    ))
}

pub(super) fn execution_options(arguments: &Value) -> Result<Attributes, String> {
    let root = object(arguments, "tools/call.arguments")?;
    attributes(root.get("execution_options"), "execution_options")
}

pub(super) fn create_council(arguments: &Value) -> Result<CreateCouncilInput, String> {
    let root = object(arguments, "tools/call.arguments")?;
    let specialty = Specialty::new(string(root, "specialty")?).map_err(domain)?;
    let count = optional_u32(root, "num_agents")?;
    let count = NumAgents::new(count).map_err(domain)?.get();
    let agents = (0..count)
        .map(|index| AgentId::new(format!("agent-{}-{index}", specialty.as_str())).map_err(domain))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CreateCouncilInput {
        council_id: CouncilId::new(Uuid::new_v4().to_string()).map_err(domain)?,
        specialty,
        agents,
    })
}

pub(super) fn specialty(arguments: &Value) -> Result<Specialty, String> {
    Specialty::new(string(
        object(arguments, "tools/call.arguments")?,
        "specialty",
    )?)
    .map_err(domain)
}

pub(super) fn agent_id(arguments: &Value) -> Result<AgentId, String> {
    AgentId::new(string(
        object(arguments, "tools/call.arguments")?,
        "agent_id",
    )?)
    .map_err(domain)
}

pub(super) fn agent(arguments: &Value) -> Result<AgentDescriptor, String> {
    let root = object(arguments, "tools/call.arguments")?;
    let agent = object(required(root, "agent")?, "agent")?;
    let specialty = string(root, "specialty")?;
    Ok(AgentDescriptor {
        id: AgentId::new(string(agent, "agent_id")?).map_err(domain)?,
        specialty: Specialty::new(if specialty.trim().is_empty() {
            string(agent, "specialty")?
        } else {
            specialty
        })
        .map_err(domain)?,
        kind: AgentKind::new(string(agent, "kind")?).map_err(domain)?,
        attributes: attributes(root.get("agent_config"), "agent_config")?,
    })
}

pub(super) fn contract(arguments: &Value) -> Result<OutputContract, String> {
    let root = object(arguments, "tools/call.arguments")?;
    output_contract(required(root, "contract")?)
}

pub(super) fn contract_id(arguments: &Value) -> Result<OutputContractId, String> {
    OutputContractId::new(string(
        object(arguments, "tools/call.arguments")?,
        "contract_id",
    )?)
    .map_err(domain)
}

pub(super) fn run_council_decision(arguments: &Value) -> Result<RunCouncilDecisionInput, String> {
    let root = object(arguments, "tools/call.arguments")?;
    let selector = match (
        optional_string(root, "council_id"),
        optional_string(root, "specialty"),
    ) {
        (Some(_), Some(_)) => {
            return Err("exactly one of `council_id` or `specialty` must be set, not both".into())
        }
        (None, None) => {
            return Err("missing required selector: set `council_id` or `specialty`".into())
        }
        (Some(id), None) => CouncilSelector::ById(CouncilId::new(id).map_err(domain)?),
        (None, Some(specialty)) => {
            CouncilSelector::BySpecialty(Specialty::new(specialty).map_err(domain)?)
        }
    };
    let validation_mode = match optional_string(root, "validation_mode").as_deref() {
        None | Some("" | "VALIDATION_MODE_UNSPECIFIED" | "VALIDATION_MODE_STRICT") => {
            ValidationMode::Strict
        }
        Some("VALIDATION_MODE_WARN") => ValidationMode::Warn,
        Some(other) => return Err(format!("unknown validation_mode `{other}`")),
    };
    Ok(RunCouncilDecisionInput {
        council_selector: selector,
        contract_id: OutputContractId::new(string(root, "contract_id")?).map_err(domain)?,
        task_description: TaskDescription::new(string(root, "description")?).map_err(domain)?,
        external_context: external_context(root.get("external_context"))?,
        validation_mode,
        metadata: metadata(root.get("metadata"))?,
    })
}

pub(super) fn trigger(arguments: &Value) -> Result<TriggerEvent, String> {
    let root = object(arguments, "tools/call.arguments")?;
    let event = object(required(root, "event")?, "event")?;
    let emitted_at = match optional_string(event, "emitted_at") {
        Some(value) if !value.trim().is_empty() => {
            OffsetDateTime::parse(&value, &Rfc3339).map_err(|error| error.to_string())?
        }
        _ => OffsetDateTime::now_utc(),
    };
    let envelope = EventEnvelope::new_with_causation(
        match optional_string(event, "event_id") {
            Some(value) if !value.trim().is_empty() => EventId::new(value).map_err(domain)?,
            _ => EventId::new(Uuid::new_v4().to_string()).map_err(domain)?,
        },
        emitted_at,
        string(event, "source")?,
        optional_id(event, "correlation_id")?,
        optional_id(event, "causation_id")?,
    )
    .map_err(domain)?;
    let specialties = event
        .get("requested_specialties")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "event.requested_specialties must be a non-empty array of strings".to_owned()
        })?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| "event.requested_specialties must contain strings".to_owned())
                .and_then(|value| Specialty::new(value).map_err(domain))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let template = optional_string(event, "task_description_template")
        .filter(|value| !value.trim().is_empty())
        .map(TaskDescription::new)
        .transpose()
        .map_err(domain)?;
    TriggerEvent::new_with_context(
        envelope,
        string(event, "kind")?,
        specialties,
        template,
        constraints(event.get("constraints"))?,
        attributes(event.get("payload"), "event.payload")?,
        external_context(event.get("external_context"))?,
    )
    .map_err(domain)
}

fn constraints(value: Option<&Value>) -> Result<TaskConstraints, String> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(TaskConstraints::default());
    };
    let value = object(value, "constraints")?;
    let rounds = optional_u32(value, "rounds")?;
    let num_agents = optional_u32(value, "num_agents")?;
    let deadline = optional_u64(value, "deadline_ms")?;
    let mut constraints = TaskConstraints::new(
        Rubric::new(object_map(value.get("rubric"), "constraints.rubric")?).map_err(domain)?,
        if rounds == 0 {
            Rounds::default()
        } else {
            Rounds::new(rounds).map_err(domain)?
        },
        if num_agents == 0 {
            None
        } else {
            Some(NumAgents::new(num_agents).map_err(domain)?)
        },
        (deadline != 0).then(|| DurationMs::from_millis(deadline)),
    );
    if let Some(contract) = value
        .get("output_contract")
        .filter(|value| !value.is_null())
    {
        constraints = constraints.with_output_contract(output_contract(contract)?);
    }
    Ok(constraints)
}

fn output_contract(value: &Value) -> Result<OutputContract, String> {
    let value = object(value, "output_contract")?;
    match optional_string(value, "format").as_deref() {
        None | Some("" | "json_object") => {}
        Some(other) => return Err(format!("output_contract.format `{other}` is not supported")),
    }
    let fields: BTreeMap<String, OutputFieldRule> =
        match value.get("fields").and_then(Value::as_object) {
            None => BTreeMap::new(),
            Some(fields) => fields
                .iter()
                .map(|(name, rule)| {
                    let rule = object(rule, "output_contract.fields.<rule>")?;
                    let allowed = rule
                        .get("allowed_string_values")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str);
                    Ok((
                        name.clone(),
                        OutputFieldRule::new(bool_value(rule, "required"), allowed)
                            .map_err(domain)?,
                    ))
                })
                .collect::<Result<_, String>>()?,
        };
    OutputContract::new_with_schema(
        optional_string(value, "contract_id").unwrap_or_default(),
        OutputFormat::JsonObject,
        fields,
        optional_string(value, "json_schema").unwrap_or_default(),
    )
    .map_err(domain)
}

fn object<'a>(value: &'a Value, where_: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{where_}: expected an object"))
}

fn required<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a Value, String> {
    object
        .get(key)
        .ok_or_else(|| format!("missing required `{key}` object"))
}

fn string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing required string `{key}`"))
}

fn optional_string(object: &Map<String, Value>, key: &str) -> Option<String> {
    object.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn optional_u32(object: &Map<String, Value>, key: &str) -> Result<u32, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(0),
        Some(value) => value
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| format!("`{key}` must be a non-negative u32")),
    }
}

fn optional_u64(object: &Map<String, Value>, key: &str) -> Result<u64, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(0),
        Some(value) => value
            .as_u64()
            .ok_or_else(|| format!("`{key}` must be a non-negative u64")),
    }
}

fn bool_value(object: &Map<String, Value>, key: &str) -> bool {
    object.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn object_map(value: Option<&Value>, where_: &str) -> Result<BTreeMap<String, Value>, String> {
    match value {
        None | Some(Value::Null) => Ok(BTreeMap::new()),
        Some(value) => Ok(object(value, where_)?
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()),
    }
}

fn attributes(value: Option<&Value>, where_: &str) -> Result<Attributes, String> {
    Attributes::new(object_map(value, where_)?).map_err(domain)
}

fn optional_id(object: &Map<String, Value>, key: &str) -> Result<Option<EventId>, String> {
    optional_string(object, key)
        .filter(|value| !value.trim().is_empty())
        .map(EventId::new)
        .transpose()
        .map_err(domain)
}

fn domain(error: made_core::error::DomainError) -> String {
    let rendered = error.to_string();
    drop(error);
    rendered
}
