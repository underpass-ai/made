use super::{
    composition, count_outcomes, AuditActorKind, CeremonyContext, CeremonyId,
    CeremonyInstancePageLimit, CeremonyName, CeremonyVersion, ClaimCeremonyWorkInput, Config,
    DurationMs, Instant, LeaseOwnerId, StartCeremonyInput,
};

pub struct Counts {
    pub seeded: u64,
    pub completed: u64,
    pub reconciliation_required: u64,
    pub claim_failures: u64,
    pub item_failures: u64,
    pub recovered: u64,
}

pub async fn run(
    config: &Config,
    harness: &composition::Harness,
    deadline: Instant,
    run_id: u128,
) -> Result<Counts, Box<dyn std::error::Error>> {
    let previous_ids = harness.stream.ids().await?;
    let mut after = previous_ids.last().cloned();
    let mut seeded = 0_u64;
    let mut completed = 0_u64;
    let mut reconciliation_required = 0_u64;
    let mut claim_failures = 0_u64;
    let mut item_failures = 0_u64;
    let mut recovered = 0_u64;
    let mut recovery_cursor = None;

    loop {
        let recovery = harness
            .driver
            .recover_page(recovery_cursor.as_ref())
            .await?;
        recovery_cursor = recovery.next_cursor().cloned();
        recovered += recovery.outcomes().len() as u64;
        item_failures += recovery.failures().len() as u64;
        count_outcomes(
            recovery.outcomes(),
            &mut completed,
            &mut reconciliation_required,
        );
        if Instant::now() >= deadline {
            break;
        }
        let mut newest = None;
        for index in 0..config.workers {
            let ceremony_id = CeremonyId::new(format!("soak-{run_id:032}-{seeded:012}"))?;
            harness
                .start
                .execute(StartCeremonyInput::new(
                    ceremony_id.clone(),
                    CeremonyName::new("worker_soak")?,
                    CeremonyVersion::v1(),
                    CeremonyContext::empty(),
                    "worker-soak",
                    AuditActorKind::Service,
                ))
                .await?;
            seeded += 1;
            newest = Some(ceremony_id);
            if index + 1 == config.workers || Instant::now() >= deadline {
                break;
            }
        }
        let outcome = harness
            .host
            .run_claim_page(ClaimCeremonyWorkInput::new(
                after.clone(),
                CeremonyInstancePageLimit::new(config.workers)?,
                LeaseOwnerId::new(format!("worker-soak-{run_id}"))?,
                DurationMs::from_millis(120_000),
                AuditActorKind::Engine,
            ))
            .await?;
        claim_failures += outcome.claim_failures().len() as u64;
        item_failures += outcome.batch().failures().len() as u64;
        count_outcomes(
            outcome.batch().outcomes(),
            &mut completed,
            &mut reconciliation_required,
        );
        after = newest;
    }

    Ok(Counts {
        seeded,
        completed,
        reconciliation_required,
        claim_failures,
        item_failures,
        recovered,
    })
}
