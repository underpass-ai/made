use super::{
    DomainError, ProtocolDefinition, ProtocolInstance, ProtocolInterventionProvenance,
    ProtocolInterventionTarget, RoleAction, RoleId,
};

impl ProtocolInstance {
    pub(super) fn matches_definition(&self, definition: &ProtocolDefinition) -> bool {
        self.definition_name == *definition.name()
            && self.definition_version == *definition.version()
    }

    pub(super) fn require_definition(
        &self,
        definition: &ProtocolDefinition,
    ) -> Result<(), DomainError> {
        if self.matches_definition(definition) {
            Ok(())
        } else {
            Err(DomainError::InvariantViolated {
                reason: "protocol instance definition mismatch",
            })
        }
    }

    pub(super) fn require_active(
        &self,
        definition: &ProtocolDefinition,
        terminal_reason: &'static str,
    ) -> Result<(), DomainError> {
        self.require_definition(definition)?;
        if self.is_terminal(definition) || self.is_ended() {
            Err(DomainError::InvariantViolated {
                reason: terminal_reason,
            })
        } else {
            Ok(())
        }
    }

    pub(super) fn require_admits_new_work(
        &self,
        operation: &'static str,
    ) -> Result<(), DomainError> {
        if self.admits_new_work() {
            Ok(())
        } else {
            Err(DomainError::LifecycleRefused {
                operation,
                phase: self.lifecycle().phase(),
            })
        }
    }

    pub(super) fn require_intervention_target(
        definition: &ProtocolDefinition,
        target: &ProtocolInterventionTarget,
    ) -> Result<(), DomainError> {
        // An exact target names one seat as surely as a role target
        // does, and a question put to an agent whose role may not
        // answer is a question nobody can close.
        if let Some(recipient) = target.exact_recipient() {
            return Self::require_answering_role(definition, recipient.role_id());
        }
        let Some(role_ids) = target.role_ids() else {
            return Ok(());
        };
        for role_id in role_ids {
            Self::require_answering_role(definition, role_id)?;
        }
        Ok(())
    }

    fn require_answering_role(
        definition: &ProtocolDefinition,
        role_id: &RoleId,
    ) -> Result<(), DomainError> {
        if definition.role(role_id).is_none() {
            return Err(DomainError::NotFound {
                what: "protocol_intervention.target_role",
            });
        }
        if !definition.role_allows(role_id, &RoleAction::respond_to_intervention()) {
            return Err(DomainError::InvariantViolated {
                reason: "target role cannot respond to protocol interventions",
            });
        }
        Ok(())
    }

    pub(super) fn require_intervention_provenance(
        &self,
        definition: &ProtocolDefinition,
        requested_by: &RoleId,
        target: &ProtocolInterventionTarget,
        provenance: &ProtocolInterventionProvenance,
    ) -> Result<(), DomainError> {
        let source = self
            .intervention(provenance.source_intervention_id())
            .ok_or(DomainError::NotFound {
                what: "protocol_intervention.provenance_source",
            })?;
        if source.requested_by() != requested_by {
            return Err(DomainError::InvariantViolated {
                reason: "only the source requester can select an intervention response",
            });
        }
        if !source
            .responses()
            .iter()
            .any(|response| response.role_id() == provenance.source_response_role_id())
        {
            return Err(DomainError::NotFound {
                what: "protocol_intervention.provenance_response",
            });
        }
        if definition.role(provenance.selected_role_id()).is_none() {
            return Err(DomainError::NotFound {
                what: "protocol_intervention.provenance_selected_role",
            });
        }
        if !definition.role_allows(
            provenance.selected_role_id(),
            &RoleAction::respond_to_intervention(),
        ) {
            return Err(DomainError::InvariantViolated {
                reason: "selected intervention role cannot respond",
            });
        }
        if !target.accepts(provenance.selected_role_id()) {
            return Err(DomainError::InvariantViolated {
                reason: "intervention target does not include the selected role",
            });
        }
        Ok(())
    }

    /// A seat this session's definition declares.
    ///
    /// Weaker on purpose than [`Self::require_role`]: a definition says
    /// which roles may run a step or fire a transition, and says
    /// nothing about who may approve a human guard. Demanding a
    /// capability that no definition grants would leave every existing
    /// protocol with no one able to approve anything. Which seats may
    /// decide a guard is a question for whenever guards grow an
    /// authority model; until then, being a seat at this table is the
    /// check, and it is enough to make the receipt name someone.
    pub(super) fn require_declared_role(
        &self,
        definition: &ProtocolDefinition,
        role_id: &RoleId,
    ) -> Result<(), DomainError> {
        self.require_definition(definition)?;
        if definition.role(role_id).is_some() {
            Ok(())
        } else {
            Err(DomainError::NotFound {
                what: "protocol_role",
            })
        }
    }

    pub(super) fn require_role(
        &self,
        definition: &ProtocolDefinition,
        role_id: &RoleId,
        action: &RoleAction,
    ) -> Result<(), DomainError> {
        self.require_definition(definition)?;
        if definition.role_allows(role_id, action) {
            Ok(())
        } else {
            Err(DomainError::InvariantViolated {
                reason: "protocol role is not allowed to perform action",
            })
        }
    }
}
