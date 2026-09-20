//! Between `google.protobuf.Struct` and JSON, for the agentic-system
//! surface.
//!
//! The design document and the run projection travel as opaque
//! structured data, like every other domain payload in this contract,
//! so what this file does is convert containers — never meaning.

use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyContext, HostActivationMode, HostAddress, HostDestination, HostKind,
};
use made_proto::v1 as pb;
use prost_types::Struct;
use serde_json::Value;

use super::attributes::{attributes_from_struct, struct_from_json};

/// A protobuf struct as JSON.
///
/// Numbers come back by the rule the ingress already fixes: a
/// `Struct` carries every number as a double, and one place decides
/// what that means rather than each surface deciding again.
pub fn struct_to_json(value: Struct) -> Result<Value, DomainError> {
    let attributes = attributes_from_struct(Some(value))?;
    Ok(Value::Object(
        attributes.as_map().clone().into_iter().collect(),
    ))
}

/// JSON as a protobuf struct.
///
/// A non-object has no `Struct` form, and an empty one says so;
/// inventing a wrapper key would put a name in the payload that
/// nobody chose.
#[must_use]
pub fn json_to_struct(value: &Value) -> Struct {
    struct_from_json(value).unwrap_or_default()
}

pub fn context_from_struct(value: Struct) -> Result<CeremonyContext, DomainError> {
    Ok(CeremonyContext::new(attributes_from_struct(Some(value))?))
}

pub fn destination_from_proto(
    destination: pb::AgenticSystemIntegratorDestination,
) -> Result<HostDestination, DomainError> {
    Ok(HostDestination::new(
        HostKind::new(destination.host_kind)?,
        HostAddress::new(destination.address)?,
        activation_mode(&destination.activation_mode)?,
    ))
}

/// An unset mode is `none`: a destination that did not ask to be woken
/// is not woken, and guessing otherwise would run somebody's command.
fn activation_mode(raw: &str) -> Result<HostActivationMode, DomainError> {
    match raw.trim() {
        "" | "none" => Ok(HostActivationMode::None),
        "command" => Ok(HostActivationMode::Command),
        _ => Err(DomainError::InvalidCharacters {
            field: "host_activation_mode",
        }),
    }
}
