use std::collections::BTreeMap;
use time::OffsetDateTime;

use super::BudgetLedgerEvent;
use crate::value_objects::{
    BudgetAccountId, BudgetBalance, BudgetDimension, BudgetLedgerVersion, BudgetLimits,
    BudgetMeasurement, BudgetOperationId, BudgetQuantities, BudgetReconciliationId,
    BudgetReservation, BudgetReservationId, BudgetTokenCount, CostMicros, ExecutionDuration,
    MeasuredBudgetQuantities, ToolCallCount,
};
use crate::BudgetError;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BudgetLedger {
    account_id: Option<BudgetAccountId>,
    limits: Option<BudgetLimits>,
    version: BudgetLedgerVersion,
    reservations: BTreeMap<BudgetReservationId, BudgetReservation>,
}

impl BudgetLedger {
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }
    pub fn rehydrate(events: &[BudgetLedgerEvent]) -> Result<Self, BudgetError> {
        let mut ledger = Self::empty();
        for event in events {
            ledger.apply(event.clone())?;
        }
        Ok(ledger)
    }
    #[must_use]
    pub const fn version(&self) -> BudgetLedgerVersion {
        self.version
    }
    #[must_use]
    pub const fn account_id(&self) -> Option<&BudgetAccountId> {
        self.account_id.as_ref()
    }
    #[must_use]
    pub const fn limits(&self) -> Option<&BudgetLimits> {
        self.limits.as_ref()
    }
    pub fn reservations(&self) -> impl Iterator<Item = &BudgetReservation> {
        self.reservations.values()
    }

    pub fn decide_open(
        &self,
        account_id: BudgetAccountId,
        limits: BudgetLimits,
        opened_at: OffsetDateTime,
    ) -> Result<Option<BudgetLedgerEvent>, BudgetError> {
        match (&self.account_id, &self.limits) {
            (None, None) => Ok(Some(BudgetLedgerEvent::Opened {
                account_id,
                limits,
                opened_at,
            })),
            (Some(stored_id), Some(stored_limits))
                if stored_id == &account_id && stored_limits == &limits =>
            {
                Ok(None)
            }
            _ => Err(BudgetError::Persistence(crate::DomainError::Conflict {
                what: "budget_ledger",
            })),
        }
    }

    pub fn decide_reserve(
        &self,
        operation_id: BudgetOperationId,
        quantities: BudgetQuantities,
        reserved_at: OffsetDateTime,
    ) -> Result<Option<BudgetLedgerEvent>, BudgetError> {
        let account_id = self.account_id.clone().ok_or(BudgetError::LedgerNotOpen)?;
        let reservation_id = BudgetReservationId::for_operation(&account_id, &operation_id);
        if let Some(existing) = self.reservations.get(&reservation_id) {
            return if existing.operation_id() == &operation_id
                && existing.quantities() == quantities
            {
                Ok(None)
            } else {
                Err(BudgetError::ReservationConflict(reservation_id))
            };
        }
        self.ensure_capacity(quantities)?;
        Ok(Some(BudgetLedgerEvent::Reserved {
            account_id,
            reservation_id,
            operation_id,
            quantities,
            reserved_at,
        }))
    }

    pub fn decide_reconcile(
        &self,
        reservation_id: BudgetReservationId,
        reconciliation_id: BudgetReconciliationId,
        measured: MeasuredBudgetQuantities,
        reconciled_at: OffsetDateTime,
    ) -> Result<Option<BudgetLedgerEvent>, BudgetError> {
        let account_id = self.account_id.clone().ok_or(BudgetError::LedgerNotOpen)?;
        let reservation = self
            .reservations
            .get(&reservation_id)
            .ok_or_else(|| BudgetError::ReservationNotFound(reservation_id.clone()))?;
        if let Some((stored_id, stored)) = reservation.reconciliation() {
            if stored_id != &reconciliation_id {
                return Err(BudgetError::ReconciliationConflict(reconciliation_id));
            }
            if stored == &measured {
                return Ok(None);
            }
            if !measured.advances(*stored) {
                return Err(BudgetError::ReconciliationConflict(reconciliation_id));
            }
        }
        Ok(Some(BudgetLedgerEvent::Reconciled {
            account_id,
            reservation_id,
            reconciliation_id,
            measured,
            reconciled_at,
        }))
    }

    pub fn apply(&mut self, event: BudgetLedgerEvent) -> Result<(), BudgetError> {
        if let Some(account_id) = &self.account_id {
            if event.account_id() != account_id {
                return Err(BudgetError::Persistence(
                    crate::DomainError::InvariantViolated {
                        reason: "budget event belongs to another account",
                    },
                ));
            }
        }
        match event {
            BudgetLedgerEvent::Opened {
                account_id, limits, ..
            } => {
                if self.account_id.is_some() {
                    return Err(BudgetError::Persistence(
                        crate::DomainError::AlreadyExists {
                            what: "budget_ledger",
                        },
                    ));
                }
                self.account_id = Some(account_id);
                self.limits = Some(limits);
            }
            BudgetLedgerEvent::Reserved {
                reservation_id,
                operation_id,
                quantities,
                reserved_at,
                ..
            } => {
                if self.account_id.is_none() {
                    return Err(BudgetError::LedgerNotOpen);
                }
                let expected_id = BudgetReservationId::for_operation(
                    self.account_id.as_ref().expect("checked above"),
                    &operation_id,
                );
                if reservation_id != expected_id {
                    return Err(BudgetError::ReservationConflict(reservation_id));
                }
                if self.reservations.contains_key(&reservation_id) {
                    return Err(BudgetError::Persistence(
                        crate::DomainError::AlreadyExists {
                            what: "budget_reservation",
                        },
                    ));
                }
                self.ensure_capacity(quantities)?;
                self.reservations.insert(
                    reservation_id.clone(),
                    BudgetReservation::new(reservation_id, operation_id, quantities, reserved_at),
                );
            }
            BudgetLedgerEvent::Reconciled {
                reservation_id,
                reconciliation_id,
                measured,
                ..
            } => {
                let reservation = self
                    .reservations
                    .get_mut(&reservation_id)
                    .ok_or_else(|| BudgetError::ReservationNotFound(reservation_id.clone()))?;
                if let Some((stored_id, stored)) = reservation.reconciliation() {
                    if stored_id != &reconciliation_id || !measured.advances(*stored) {
                        return Err(BudgetError::ReconciliationConflict(reconciliation_id));
                    }
                }
                reservation.reconcile(reconciliation_id, measured);
            }
        }
        self.version = self.version.next();
        Ok(())
    }

    pub fn balance(&self) -> Result<BudgetBalance, BudgetError> {
        let limits = self.limits.clone().ok_or(BudgetError::LedgerNotOpen)?;
        let mut reserved = [0; 4];
        let mut observed = [0; 4];
        let mut estimated = [0; 4];
        let mut unconfirmed = [0; 4];
        for reservation in self.reservations.values() {
            let requested = values(reservation.quantities());
            match reservation.reconciliation() {
                None => add(&mut reserved, requested)?,
                Some((_, measured)) => classify(
                    &mut observed,
                    &mut estimated,
                    &mut unconfirmed,
                    requested,
                    *measured,
                )?,
            }
        }
        let maximum = limit_values(&limits);
        let charged = sums([reserved, observed, estimated, unconfirmed])?;
        let available = zip(maximum, charged, u64::saturating_sub);
        let overrun = zip(charged, maximum, u64::saturating_sub);
        Ok(BudgetBalance::new(
            limits,
            quantities(reserved),
            quantities(observed),
            quantities(estimated),
            quantities(unconfirmed),
            quantities(overrun),
            quantities(available),
        ))
    }

    fn ensure_capacity(&self, request: BudgetQuantities) -> Result<(), BudgetError> {
        let account_id = self.account_id.clone().ok_or(BudgetError::LedgerNotOpen)?;
        let balance = self.balance()?;
        let overrun = values(balance.overrun());
        let dimensions = [
            BudgetDimension::Duration,
            BudgetDimension::Tokens,
            BudgetDimension::Cost,
            BudgetDimension::ToolCalls,
        ];
        if let Some((index, _)) = overrun.iter().enumerate().find(|(_, value)| **value > 0) {
            return Err(BudgetError::Exhausted {
                account_id,
                dimension: dimensions[index],
            });
        }
        let available = values(balance.available());
        let wanted = values(request);
        for (index, dimension) in dimensions.into_iter().enumerate() {
            if wanted[index] > available[index] {
                return Err(BudgetError::Exhausted {
                    account_id,
                    dimension,
                });
            }
        }
        Ok(())
    }
}

fn values(q: BudgetQuantities) -> [u64; 4] {
    [
        q.duration().as_micros(),
        q.tokens().value(),
        q.cost().value(),
        q.tool_calls().value(),
    ]
}

fn limit_values(limits: &BudgetLimits) -> [u64; 4] {
    [
        limits
            .duration()
            .map_or(u64::MAX, ExecutionDuration::as_micros),
        limits.tokens().map_or(u64::MAX, BudgetTokenCount::value),
        limits.cost().map_or(u64::MAX, CostMicros::value),
        limits.tool_calls().map_or(u64::MAX, ToolCallCount::value),
    ]
}
fn quantities(v: [u64; 4]) -> BudgetQuantities {
    BudgetQuantities::new(
        ExecutionDuration::from_micros(v[0]),
        BudgetTokenCount::new(v[1]),
        CostMicros::new(v[2]),
        ToolCallCount::new(v[3]),
    )
}
fn add(target: &mut [u64; 4], value: [u64; 4]) -> Result<(), BudgetError> {
    for index in 0..4 {
        target[index] = target[index]
            .checked_add(value[index])
            .ok_or(BudgetError::Persistence(
                crate::DomainError::InvariantViolated {
                    reason: "budget total overflow",
                },
            ))?;
    }
    Ok(())
}
fn sums(values: [[u64; 4]; 4]) -> Result<[u64; 4], BudgetError> {
    let mut result = [0; 4];
    for value in values {
        add(&mut result, value)?;
    }
    Ok(result)
}
fn zip(left: [u64; 4], right: [u64; 4], op: fn(u64, u64) -> u64) -> [u64; 4] {
    [
        op(left[0], right[0]),
        op(left[1], right[1]),
        op(left[2], right[2]),
        op(left[3], right[3]),
    ]
}
fn classify(
    observed: &mut [u64; 4],
    estimated: &mut [u64; 4],
    unknown: &mut [u64; 4],
    requested: [u64; 4],
    measured: MeasuredBudgetQuantities,
) -> Result<(), BudgetError> {
    classify_one(
        observed,
        estimated,
        unknown,
        0,
        requested[0],
        measured.duration(),
        ExecutionDuration::as_micros,
    )?;
    classify_one(
        observed,
        estimated,
        unknown,
        1,
        requested[1],
        measured.tokens(),
        BudgetTokenCount::value,
    )?;
    classify_one(
        observed,
        estimated,
        unknown,
        2,
        requested[2],
        measured.cost(),
        CostMicros::value,
    )?;
    classify_one(
        observed,
        estimated,
        unknown,
        3,
        requested[3],
        measured.tool_calls(),
        ToolCallCount::value,
    )
}
fn classify_one<T>(
    observed: &mut [u64; 4],
    estimated: &mut [u64; 4],
    unknown: &mut [u64; 4],
    index: usize,
    requested: u64,
    measurement: BudgetMeasurement<T>,
    value_of: fn(T) -> u64,
) -> Result<(), BudgetError> {
    match measurement {
        BudgetMeasurement::Observed(value) => add_at(observed, index, value_of(value)),
        BudgetMeasurement::Estimated(value) => add_at(estimated, index, value_of(value)),
        BudgetMeasurement::Unknown => add_at(unknown, index, requested),
    }
}
fn add_at(target: &mut [u64; 4], index: usize, value: u64) -> Result<(), BudgetError> {
    target[index] = target[index]
        .checked_add(value)
        .ok_or(BudgetError::Persistence(
            crate::DomainError::InvariantViolated {
                reason: "budget total overflow",
            },
        ))?;
    Ok(())
}
