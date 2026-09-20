use time::OffsetDateTime;

use crate::ports::{BindOutcome, BindReplacement, IntegratorBindingPort};
use crate::value_objects::{
    HostActivationMode, HostAddress, HostDestination, HostKind, IntegratorBinding,
    IntegratorBindingId, IntegratorFence, IntegratorScope,
};

use super::host_delivery_fixtures::{call, ceremony, failure, incarnation, origin, role};
use super::ConformanceFailure;

/// Storage-independent properties of integrator bindings.
#[derive(Debug)]
pub struct IntegratorBindingConformance;

impl IntegratorBindingConformance {
    pub async fn run(
        bindings: &dyn IntegratorBindingPort,
    ) -> Result<Vec<&'static str>, ConformanceFailure> {
        let mut passed = Vec::new();
        Self::one_scope_holds_one_live_binding(bindings).await?;
        passed.push("one_scope_holds_one_live_binding");
        Self::rebinding_the_same_integrator_changes_nothing(bindings).await?;
        passed.push("rebinding_the_same_integrator_changes_nothing");
        Self::a_replacement_raises_the_fence(bindings).await?;
        passed.push("a_replacement_raises_the_fence");
        Self::a_revoked_binding_stops_being_current(bindings).await?;
        passed.push("a_revoked_binding_stops_being_current");
        Self::scopes_do_not_reach_into_each_other(bindings).await?;
        passed.push("scopes_do_not_reach_into_each_other");
        Ok(passed)
    }

    async fn one_scope_holds_one_live_binding(
        bindings: &dyn IntegratorBindingPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "one_scope_holds_one_live_binding";
        let scope = IntegratorScope::ceremony(ceremony(PROPERTY)?);
        let first = binding(PROPERTY, "first", scope.clone())?;
        call(
            PROPERTY,
            bindings.bind(first.clone(), BindReplacement::Refuse).await,
        )?;

        let intruder = binding(PROPERTY, "second", scope.clone())?;
        let refused = call(
            PROPERTY,
            bindings.bind(intruder, BindReplacement::Refuse).await,
        )?;
        let BindOutcome::AlreadyExists { existing } = refused else {
            return Err(failure(
                PROPERTY,
                "a second integrator took over a scope it was not allowed to",
            ));
        };
        if existing.id() != first.id() {
            return Err(failure(PROPERTY, "the incumbent was not the one reported"));
        }
        let current = call(PROPERTY, bindings.current(&scope).await)?
            .ok_or_else(|| failure(PROPERTY, "the bound scope has no current binding"))?;
        if current.id() != first.id() {
            return Err(failure(PROPERTY, "the refused call changed who is bound"));
        }
        Ok(())
    }

    async fn rebinding_the_same_integrator_changes_nothing(
        bindings: &dyn IntegratorBindingPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "rebinding_the_same_integrator_changes_nothing";
        let scope = IntegratorScope::ceremony(ceremony(PROPERTY)?);
        let offered = binding(PROPERTY, "only", scope.clone())?;
        call(
            PROPERTY,
            bindings
                .bind(offered.clone(), BindReplacement::Refuse)
                .await,
        )?;

        let repeated = call(
            PROPERTY,
            bindings
                .bind(offered.clone(), BindReplacement::Replace)
                .await,
        )?;
        let BindOutcome::AlreadyBound(existing) = repeated else {
            return Err(failure(
                PROPERTY,
                "a host that bound itself twice was treated as a replacement",
            ));
        };
        if existing.fence() != IntegratorFence::FIRST {
            return Err(failure(
                PROPERTY,
                "rebinding the same integrator moved the fence",
            ));
        }
        Ok(())
    }

    async fn a_replacement_raises_the_fence(
        bindings: &dyn IntegratorBindingPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "a_replacement_raises_the_fence";
        let scope = IntegratorScope::ceremony(ceremony(PROPERTY)?);
        let first = binding(PROPERTY, "first", scope.clone())?;
        call(
            PROPERTY,
            bindings.bind(first.clone(), BindReplacement::Refuse).await,
        )?;

        let second = binding(PROPERTY, "second", scope.clone())?;
        let replaced = call(
            PROPERTY,
            bindings
                .bind(second.clone(), BindReplacement::Replace)
                .await,
        )?;
        let BindOutcome::Replaced { previous, current } = replaced else {
            return Err(failure(PROPERTY, "the incumbent was not displaced"));
        };
        if previous.id() != first.id() {
            return Err(failure(PROPERTY, "the displaced binding was not reported"));
        }
        if current.fence() != IntegratorFence::FIRST.next() {
            return Err(failure(
                PROPERTY,
                "a replacement did not outrank what it replaced",
            ));
        }
        let stored = call(PROPERTY, bindings.current(&scope).await)?
            .ok_or_else(|| failure(PROPERTY, "the replaced scope has no current binding"))?;
        if stored.id() != second.id() || stored.fence() != current.fence() {
            return Err(failure(
                PROPERTY,
                "the stored binding is not the replacement that was reported",
            ));
        }
        Ok(())
    }

    async fn a_revoked_binding_stops_being_current(
        bindings: &dyn IntegratorBindingPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "a_revoked_binding_stops_being_current";
        let scope = IntegratorScope::ceremony(ceremony(PROPERTY)?);
        let offered = binding(PROPERTY, "only", scope.clone())?;
        call(
            PROPERTY,
            bindings
                .bind(offered.clone(), BindReplacement::Refuse)
                .await,
        )?;

        let revoked = call(PROPERTY, bindings.revoke(offered.id(), origin()).await)?
            .ok_or_else(|| failure(PROPERTY, "revoking a live binding found nothing"))?;
        if revoked.is_live() {
            return Err(failure(PROPERTY, "a revoked binding is still live"));
        }
        if call(PROPERTY, bindings.current(&scope).await)?.is_some() {
            return Err(failure(
                PROPERTY,
                "a revoked binding is still the current one",
            ));
        }
        let listed = call(PROPERTY, bindings.list(Some(&scope)).await)?;
        if listed.len() != 1 || listed[0].is_live() {
            return Err(failure(
                PROPERTY,
                "a revoked binding stopped being visible at all",
            ));
        }
        let again = call(PROPERTY, bindings.revoke(offered.id(), origin()).await)?;
        if again.is_some_and(|binding| binding.is_live()) {
            return Err(failure(PROPERTY, "revoking twice brought a binding back"));
        }
        Ok(())
    }

    async fn scopes_do_not_reach_into_each_other(
        bindings: &dyn IntegratorBindingPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "scopes_do_not_reach_into_each_other";
        let left = IntegratorScope::ceremony(ceremony(PROPERTY)?);
        let right = IntegratorScope::system_execution(
            crate::value_objects::AgenticSystemExecutionId::new(PROPERTY.replace('_', "-"))
                .map_err(|error| failure(PROPERTY, error.to_string()))?,
        );
        call(
            PROPERTY,
            bindings
                .bind(
                    binding(PROPERTY, "left", left.clone())?,
                    BindReplacement::Refuse,
                )
                .await,
        )?;
        let other = call(
            PROPERTY,
            bindings
                .bind(
                    binding(PROPERTY, "right", right.clone())?,
                    BindReplacement::Refuse,
                )
                .await,
        )?;
        if !matches!(other, BindOutcome::Bound(_)) {
            return Err(failure(
                PROPERTY,
                "a ceremony's binding blocked a system run's own",
            ));
        }
        let listed = call(PROPERTY, bindings.list(Some(&right)).await)?;
        if listed.len() != 1 {
            return Err(failure(
                PROPERTY,
                "listing one scope returned another scope's bindings",
            ));
        }
        Ok(())
    }
}

fn binding(
    property: &'static str,
    suffix: &str,
    scope: IntegratorScope,
) -> Result<IntegratorBinding, ConformanceFailure> {
    Ok(IntegratorBinding::new(
        IntegratorBindingId::new(format!("{property}-{suffix}"))
            .map_err(|error| failure(property, error.to_string()))?,
        scope,
        role(property, "integrator")?,
        HostDestination::new(
            HostKind::new("generic").map_err(|error| failure(property, error.to_string()))?,
            HostAddress::new(format!("{property}-{suffix}-address"))
                .map_err(|error| failure(property, error.to_string()))?,
            HostActivationMode::None,
        ),
        incarnation(property, suffix)?,
        now(),
    ))
}

fn now() -> OffsetDateTime {
    origin()
}
