use crate::entities::CouncilJournalEvent;
use crate::value_objects::{AuthorizationEvidence, CouncilJournalPosition};
use serde::{Deserialize, Serialize};

/// A durable council fact sealed at one position, returned unchanged on replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CouncilJournalRecord {
    position: CouncilJournalPosition,
    event: CouncilJournalEvent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    authorization: Option<AuthorizationEvidence>,
}
impl CouncilJournalRecord {
    #[must_use]
    pub fn new(position: CouncilJournalPosition, event: CouncilJournalEvent) -> Self {
        Self {
            position,
            event,
            authorization: None,
        }
    }
    #[must_use]
    pub fn authorized(
        position: CouncilJournalPosition,
        event: CouncilJournalEvent,
        authorization: AuthorizationEvidence,
    ) -> Self {
        Self {
            position,
            event,
            authorization: Some(authorization),
        }
    }
    #[must_use]
    pub fn position(&self) -> CouncilJournalPosition {
        self.position
    }
    #[must_use]
    pub fn event(&self) -> &CouncilJournalEvent {
        &self.event
    }
    #[must_use]
    pub fn authorization(&self) -> Option<&AuthorizationEvidence> {
        self.authorization.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::Specialty;

    #[test]
    fn historical_record_without_authorization_keeps_its_exact_json() {
        let historical = r#"{"position":1,"event":{"kind":"council_deleted","fact":"research"}}"#;
        let record: CouncilJournalRecord = serde_json::from_str(historical).unwrap();

        assert!(record.authorization().is_none());
        assert_eq!(serde_json::to_string(&record).unwrap(), historical);
        assert_eq!(
            record.event(),
            &CouncilJournalEvent::CouncilDeleted(Specialty::new("research").unwrap())
        );
    }
}
