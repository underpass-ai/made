use made_core::value_objects::{AuthorizationAction, AuthorizationScope};
use made_core::DomainError;

use super::EmbeddedMade;

impl EmbeddedMade {
    pub(crate) fn require_authorized_action(
        &self,
        expected: AuthorizationAction,
    ) -> Result<(), DomainError> {
        self.require_any_authorized_action(&[expected])
    }

    pub(super) fn require_any_authorized_action(
        &self,
        expected: &[AuthorizationAction],
    ) -> Result<(), DomainError> {
        if self.authorization.is_none() {
            return Ok(());
        }
        let operation = made_app::services::AuthorizationOperationScope::current().ok_or(
            DomainError::InvariantViolated {
                reason: "protected embedded facade requires an authorized operation context",
            },
        )?;
        if !expected.contains(&operation.evidence().action()) {
            return Err(DomainError::InvariantViolated {
                reason: "authorized operation action does not match the embedded facade method",
            });
        }
        Ok(())
    }

    pub(super) fn require_authorized_protocol_action(
        &self,
        expected: AuthorizationAction,
        protocol_id: &made_core::value_objects::ProtocolId,
    ) -> Result<(), DomainError> {
        self.require_authorized_action(expected)?;
        if self.authorization.is_none() {
            return Ok(());
        }
        let operation = made_app::services::AuthorizationOperationScope::current().ok_or(
            DomainError::InvariantViolated {
                reason: "protected embedded facade requires an authorized operation context",
            },
        )?;
        if protocol_scope_matches(operation.evidence().scope(), protocol_id) {
            return Ok(());
        }
        Err(DomainError::InvariantViolated {
            reason: "authorized operation scope does not admit this protocol",
        })
    }

    pub(super) fn require_authorized_scope_action(
        &self,
        expected: AuthorizationAction,
        scope: &AuthorizationScope,
    ) -> Result<(), DomainError> {
        self.require_authorized_action(expected)?;
        if self.authorization.is_none() {
            return Ok(());
        }
        let operation = made_app::services::AuthorizationOperationScope::current().ok_or(
            DomainError::InvariantViolated {
                reason: "protected embedded facade requires an authorized operation context",
            },
        )?;
        if operation.evidence().scope() == scope {
            return Ok(());
        }
        Err(DomainError::InvariantViolated {
            reason: "authorized operation scope does not admit this resource",
        })
    }

    pub(super) fn require_authorized_global_action(
        &self,
        expected: AuthorizationAction,
    ) -> Result<(), DomainError> {
        self.require_authorized_action(expected)?;
        if self.authorization.is_none() {
            return Ok(());
        }
        let operation = made_app::services::AuthorizationOperationScope::current().ok_or(
            DomainError::InvariantViolated {
                reason: "protected embedded facade requires an authorized operation context",
            },
        )?;
        if operation.evidence().scope() == &AuthorizationScope::Global {
            return Ok(());
        }
        Err(DomainError::InvariantViolated {
            reason: "authorized operation scope is not global",
        })
    }

    pub(crate) fn require_authorized_definition_action(
        &self,
        expected: AuthorizationAction,
        name: &made_core::value_objects::ProtocolName,
        version: Option<&made_core::value_objects::ProtocolVersion>,
    ) -> Result<(), DomainError> {
        self.require_authorized_action(expected)?;
        if self.authorization.is_none() {
            return Ok(());
        }
        let operation = made_app::services::AuthorizationOperationScope::current().ok_or(
            DomainError::InvariantViolated {
                reason: "protected embedded facade requires an authorized operation context",
            },
        )?;
        let matches = matches!(
            operation.evidence().scope(),
            AuthorizationScope::Definition {
                name: admitted_name,
                version: admitted_version,
            } if admitted_name == name && admitted_version.as_ref() == version
        );
        if matches {
            return Ok(());
        }
        Err(DomainError::InvariantViolated {
            reason: "authorized operation scope does not admit this definition",
        })
    }

    pub(super) fn require_definition_scope_when_mounting(
        &self,
        name: &made_core::value_objects::ProtocolName,
        version: &made_core::value_objects::ProtocolVersion,
    ) -> Result<(), DomainError> {
        if self.authorization.is_none() {
            return Ok(());
        }
        let operation = made_app::services::AuthorizationOperationScope::current().ok_or(
            DomainError::InvariantViolated {
                reason: "protected embedded facade requires an authorized operation context",
            },
        )?;
        if operation.evidence().action() == AuthorizationAction::MountDefinition {
            self.require_authorized_definition_action(
                AuthorizationAction::MountDefinition,
                name,
                Some(version),
            )?;
        }
        Ok(())
    }

    pub(super) fn require_authorized_artifact_action(
        &self,
        expected: AuthorizationAction,
        artifact_id: &made_core::value_objects::ArtifactId,
    ) -> Result<(), DomainError> {
        self.require_authorized_action(expected)?;
        if self.authorization.is_none() {
            return Ok(());
        }
        let operation = made_app::services::AuthorizationOperationScope::current().ok_or(
            DomainError::InvariantViolated {
                reason: "protected embedded facade requires an authorized operation context",
            },
        )?;
        if matches!(
            operation.evidence().scope(),
            AuthorizationScope::Artifact {
                artifact_id: admitted,
            } if admitted == artifact_id
        ) {
            return Ok(());
        }
        Err(DomainError::InvariantViolated {
            reason: "authorized operation scope does not admit this artifact",
        })
    }

    pub(super) fn require_authorized_budget_action(
        &self,
        expected: AuthorizationAction,
        account_id: &made_core::value_objects::BudgetAccountId,
    ) -> Result<(), DomainError> {
        self.require_authorized_action(expected)?;
        if self.authorization.is_none() {
            return Ok(());
        }
        let operation = made_app::services::AuthorizationOperationScope::current().ok_or(
            DomainError::InvariantViolated {
                reason: "protected embedded facade requires an authorized operation context",
            },
        )?;
        if matches!(
            operation.evidence().scope(),
            AuthorizationScope::Budget {
                account_id: admitted,
            } if admitted == account_id
        ) {
            return Ok(());
        }
        Err(DomainError::InvariantViolated {
            reason: "authorized operation scope does not admit this budget",
        })
    }

    pub(super) fn require_authorized_council_action(
        &self,
        expected: AuthorizationAction,
        council_id: &made_core::value_objects::CouncilId,
    ) -> Result<(), DomainError> {
        self.require_authorized_action(expected)?;
        if self.authorization.is_none() {
            return Ok(());
        }
        let operation = made_app::services::AuthorizationOperationScope::current().ok_or(
            DomainError::InvariantViolated {
                reason: "protected embedded facade requires an authorized operation context",
            },
        )?;
        if matches!(operation.evidence().scope(), AuthorizationScope::Global)
            || matches!(
                operation.evidence().scope(),
                AuthorizationScope::Council {
                    council_id: admitted,
                } if admitted == council_id
            )
        {
            return Ok(());
        }
        Err(DomainError::InvariantViolated {
            reason: "authorized operation scope does not admit this council",
        })
    }

    pub(super) fn require_authorized_protocol_records(
        &self,
        protocol_id: &made_core::value_objects::ProtocolId,
    ) -> Result<(), DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::ReadProtocolEvents,
            protocol_id,
        )
    }
}

fn protocol_scope_matches(
    scope: &AuthorizationScope,
    protocol_id: &made_core::value_objects::ProtocolId,
) -> bool {
    match scope {
        AuthorizationScope::Protocol {
            protocol_id: admitted,
        }
        | AuthorizationScope::ResolvedProtocol {
            protocol_id: admitted,
            ..
        } => admitted == protocol_id,
        _ => false,
    }
}
