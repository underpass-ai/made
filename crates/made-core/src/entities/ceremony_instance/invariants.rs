use super::{
    CeremonyDefinition, CeremonyInstance, CeremonyInterventionProvenance,
    CeremonyInterventionTarget, DomainError, RoleAction, RoleId,
};

impl CeremonyInstance {
    pub(super) fn matches_definition(&self, definition: &CeremonyDefinition) -> bool {
        self.definition_name == *definition.name()
            && self.definition_version == *definition.version()
    }

    pub(super) fn require_definition(
        &self,
        definition: &CeremonyDefinition,
    ) -> Result<(), DomainError> {
        if self.matches_definition(definition) {
            Ok(())
        } else {
            Err(DomainError::InvariantViolated {
                reason: "ceremony instance definition mismatch",
            })
        }
    }

    pub(super) fn require_active(
        &self,
        definition: &CeremonyDefinition,
        terminal_reason: &'static str,
    ) -> Result<(), DomainError> {
        self.require_definition(definition)?;
        if self.is_terminal(definition) {
            Err(DomainError::InvariantViolated {
                reason: terminal_reason,
            })
        } else {
            Ok(())
        }
    }

    pub(super) fn require_intervention_target(
        definition: &CeremonyDefinition,
        target: &CeremonyInterventionTarget,
    ) -> Result<(), DomainError> {
        let Some(role_ids) = target.role_ids() else {
            return Ok(());
        };
        for role_id in role_ids {
            if definition.role(role_id).is_none() {
                return Err(DomainError::NotFound {
                    what: "ceremony_intervention.target_role",
                });
            }
            if !definition.role_allows(role_id, &RoleAction::respond_to_intervention()) {
                return Err(DomainError::InvariantViolated {
                    reason: "target role cannot respond to ceremony interventions",
                });
            }
        }
        Ok(())
    }

    pub(super) fn require_intervention_provenance(
        &self,
        definition: &CeremonyDefinition,
        requested_by: &RoleId,
        target: &CeremonyInterventionTarget,
        provenance: &CeremonyInterventionProvenance,
    ) -> Result<(), DomainError> {
        let source = self
            .intervention(provenance.source_intervention_id())
            .ok_or(DomainError::NotFound {
                what: "ceremony_intervention.provenance_source",
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
                what: "ceremony_intervention.provenance_response",
            });
        }
        if definition.role(provenance.selected_role_id()).is_none() {
            return Err(DomainError::NotFound {
                what: "ceremony_intervention.provenance_selected_role",
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
    /// ceremony with no one able to approve anything. Which seats may
    /// decide a guard is a question for whenever guards grow an
    /// authority model; until then, being a seat at this table is the
    /// check, and it is enough to make the receipt name someone.
    pub(super) fn require_declared_role(
        &self,
        definition: &CeremonyDefinition,
        role_id: &RoleId,
    ) -> Result<(), DomainError> {
        self.require_definition(definition)?;
        if definition.role(role_id).is_some() {
            Ok(())
        } else {
            Err(DomainError::NotFound {
                what: "ceremony_role",
            })
        }
    }

    pub(super) fn require_role(
        &self,
        definition: &CeremonyDefinition,
        role_id: &RoleId,
        action: &RoleAction,
    ) -> Result<(), DomainError> {
        self.require_definition(definition)?;
        if definition.role_allows(role_id, action) {
            Ok(())
        } else {
            Err(DomainError::InvariantViolated {
                reason: "ceremony role is not allowed to perform action",
            })
        }
    }
}
