use super::council_snapshot::encode;
use super::error::sqlx_to_domain;
use crate::council_data_snapshot::CouncilDataSnapshot;
use made_core::entities::Statistics;
use made_core::error::DomainError;
use sqlx::{Postgres, Transaction};

/// Inserts into an empty destination in the caller's locked transaction.
pub(super) async fn write(
    tx: &mut Transaction<'_, Postgres>,
    snapshot: &CouncilDataSnapshot,
) -> Result<(), DomainError> {
    for council in &snapshot.councils {
        sqlx::query("INSERT INTO councils (specialty, council_id, body) VALUES ($1, $2, $3)")
            .bind(council.specialty().as_str())
            .bind(council.id().as_str())
            .bind(encode(council)?)
            .execute(&mut **tx)
            .await
            .map_err(|e| sqlx_to_domain(e, "import council"))?;
    }
    for agent in &snapshot.agents {
        sqlx::query(
            "INSERT INTO agents (agent_id, specialty, kind, attributes) VALUES ($1, $2, $3, $4)",
        )
        .bind(agent.id.as_str())
        .bind(agent.specialty.as_str())
        .bind(agent.kind.as_str())
        .bind(encode(&agent.attributes)?)
        .execute(&mut **tx)
        .await
        .map_err(|e| sqlx_to_domain(e, "import agent"))?;
    }
    for contract in &snapshot.contracts {
        sqlx::query("INSERT INTO council_contracts (contract_id, body) VALUES ($1, $2)")
            .bind(contract.contract_id().as_str())
            .bind(encode(contract)?)
            .execute(&mut **tx)
            .await
            .map_err(|e| sqlx_to_domain(e, "import contract"))?;
    }
    for deliberation in &snapshot.deliberations {
        sqlx::query("INSERT INTO deliberations (task_id, specialty, phase, winner_proposal_id, body) VALUES ($1, $2, $3, $4, $5)")
            .bind(deliberation.task_id().as_str()).bind(deliberation.specialty().as_str())
            .bind(super::deliberation_repository::phase_name(deliberation.phase()))
            .bind(deliberation.ranking().first().map(made_core::value_objects::ProposalId::as_str))
            .bind(encode(deliberation)?).execute(&mut **tx).await.map_err(|e| sqlx_to_domain(e, "import deliberation"))?;
    }
    write_statistics(tx, &snapshot.statistics).await
}
async fn write_statistics(
    tx: &mut Transaction<'_, Postgres>,
    statistics: &Statistics,
) -> Result<(), DomainError> {
    sqlx::query("INSERT INTO statistics_totals (id, total_deliberations, total_orchestrations, total_duration_ms) VALUES ('singleton', $1, $2, $3) ON CONFLICT (id) DO UPDATE SET total_deliberations = EXCLUDED.total_deliberations, total_orchestrations = EXCLUDED.total_orchestrations, total_duration_ms = EXCLUDED.total_duration_ms")
        .bind(counter(statistics.total_deliberations())?).bind(counter(statistics.total_orchestrations())?).bind(counter(statistics.total_duration().get())?)
        .execute(&mut **tx).await.map_err(|e| sqlx_to_domain(e, "import statistics"))?;
    for (specialty, count) in statistics.per_specialty() {
        sqlx::query(
            "INSERT INTO statistics_by_specialty (specialty, deliberations) VALUES ($1, $2)",
        )
        .bind(specialty.as_str())
        .bind(counter(*count)?)
        .execute(&mut **tx)
        .await
        .map_err(|e| sqlx_to_domain(e, "import specialty statistics"))?;
    }
    Ok(())
}
fn counter(value: u64) -> Result<i64, DomainError> {
    i64::try_from(value).map_err(|_| DomainError::InvariantViolated {
        reason: "council statistics exceed Postgres range",
    })
}
