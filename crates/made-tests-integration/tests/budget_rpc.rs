use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{
    BudgetLimits, BudgetMeasurement, BudgetReservationEstimate, ClaimCeremonyStepRequest,
    GetBudgetReportRequest, GetCeremonyInstanceRequest, ListPendingBudgetReservationsRequest,
    PrepareCeremonyChildrenRequest, PublishCeremonyDefinitionRequest,
    StartPublishedCeremonyRequest,
};
use made_tests_integration::grpc_fixture::GrpcFixture;
use tonic::{transport::Channel, Code};

const DEFINITION: &str =
    include_str!("../../../tests/e2e/ceremonies/editorial-planning-meeting.yaml");

const CHILD_DEFINITION: &str = r#"
version: "1.0"
name: budget_rpc_child
states:
  - id: OPEN
    initial: true
steps:
  - id: work
    state: OPEN
    handler: external
roles:
  - id: WORKER
    allowed_actions: [work]
"#;

const PARENT_DEFINITION: &str = r#"
version: "1.0"
name: budget_rpc_parent
states:
  - id: OPEN
    initial: true
steps:
  - id: fanout
    state: OPEN
    handler: delegated_children
    spawn:
      children:
        - ceremony: budget_rpc_child
          version: "1.0"
          inputs: {}
      max_children: 1
      max_depth: 1
roles:
  - id: COORDINATOR
    allowed_actions: [fanout]
"#;

#[tokio::test]
async fn budgeted_admission_requires_a_real_estimate_and_reports_the_reservation() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);
    client
        .publish_ceremony_definition(PublishCeremonyDefinitionRequest {
            definition_yaml: DEFINITION.to_owned(),
        })
        .await
        .unwrap();
    let ceremony_id = "budget-rpc-root";
    let started = client
        .start_published_ceremony(start_request(ceremony_id))
        .await
        .unwrap()
        .into_inner()
        .instance
        .unwrap();
    assert_eq!(started.budget_account_id, ceremony_id);

    let missing_amount = client
        .claim_ceremony_step(claim(
            ceremony_id,
            "missing-amount",
            BudgetMeasurement {
                quality: "estimated".to_owned(),
                amount: None,
            },
        ))
        .await
        .unwrap_err();
    assert_eq!(missing_amount.code(), Code::InvalidArgument);

    let unknown = client
        .claim_ceremony_step(claim(ceremony_id, "unknown", measurement("unknown", 0)))
        .await
        .unwrap_err();
    assert_eq!(unknown.code(), Code::FailedPrecondition);
    let after_refusal = client
        .get_ceremony_instance(GetCeremonyInstanceRequest {
            ceremony_id: ceremony_id.to_owned(),
        })
        .await
        .unwrap()
        .into_inner()
        .instance
        .unwrap();
    assert_eq!(
        after_refusal
            .steps
            .iter()
            .find(|step| step.step_id == "open_room")
            .unwrap()
            .status,
        "pending"
    );

    let admitted = client
        .claim_ceremony_step(claim(
            ceremony_id,
            "estimated",
            measurement("estimated", 20),
        ))
        .await
        .unwrap()
        .into_inner();
    let admission = admitted.budget.unwrap();
    assert_eq!(admission.account_id, ceremony_id);
    assert!(!admission.operation_id.is_empty());
    assert!(!admission.reservation_id.is_empty());

    let report = client
        .get_budget_report(GetBudgetReportRequest {
            ceremony_id: ceremony_id.to_owned(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(report.account_id, ceremony_id);
    assert_eq!(report.balance.unwrap().reserved.unwrap().tokens, 20);

    let pending = client
        .list_pending_budget_reservations(ListPendingBudgetReservationsRequest {
            after_reservation_id: String::new(),
            limit: 10,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(pending.reservations.len(), 1);
    assert_eq!(
        pending.reservations[0].reservation_id,
        admission.reservation_id
    );
}

#[tokio::test]
async fn explicit_zero_ceilings_are_rejected_before_creating_a_ceremony() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);
    client
        .publish_ceremony_definition(PublishCeremonyDefinitionRequest {
            definition_yaml: DEFINITION.to_owned(),
        })
        .await
        .unwrap();
    for dimension in ["duration", "tokens", "cost", "tool_calls"] {
        let rejected_id = format!("budget-zero-{dimension}");
        let mut request = start_request(&rejected_id);
        let limits = request.budget_limits.as_mut().unwrap();
        match dimension {
            "duration" => limits.duration_micros = Some(0),
            "tokens" => {
                limits.tokens = Some(0);
                limits.tool_calls = Some(10);
            }
            "cost" => limits.cost_micros = Some(0),
            "tool_calls" => limits.tool_calls = Some(0),
            _ => unreachable!(),
        }
        let error = client.start_published_ceremony(request).await.unwrap_err();
        assert_eq!(error.code(), Code::InvalidArgument, "{dimension}");
        let absent = client
            .get_ceremony_instance(GetCeremonyInstanceRequest {
                ceremony_id: rejected_id,
            })
            .await
            .unwrap_err();
        assert_eq!(absent.code(), Code::NotFound, "{dimension}");
    }
}

#[tokio::test]
async fn budgeted_child_preparation_reserves_before_claim_and_retries_once() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);
    publish_fanout(&mut client).await;
    let ceremony_id = "budget-rpc-fanout";
    start_fanout(&mut client, ceremony_id, true).await;

    let missing = client
        .prepare_ceremony_children(prepare_children(ceremony_id, None))
        .await
        .unwrap_err();
    assert_eq!(missing.code(), Code::FailedPrecondition);
    let unchanged = client
        .get_ceremony_instance(GetCeremonyInstanceRequest {
            ceremony_id: ceremony_id.to_owned(),
        })
        .await
        .unwrap()
        .into_inner()
        .instance
        .unwrap();
    assert_eq!(unchanged.steps[0].status, "pending");
    let empty_budget = client
        .get_budget_report(GetBudgetReportRequest {
            ceremony_id: ceremony_id.to_owned(),
        })
        .await
        .unwrap()
        .into_inner()
        .balance
        .unwrap();
    assert_eq!(empty_budget.reserved.unwrap().tokens, 0);

    let estimate = BudgetReservationEstimate {
        duration: Some(measurement("unknown", 0)),
        tokens: Some(measurement("estimated", 5)),
        cost: Some(measurement("unknown", 0)),
        tool_calls: Some(measurement("unknown", 0)),
    };
    let prepared = client
        .prepare_ceremony_children(prepare_children(ceremony_id, Some(estimate.clone())))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(prepared.child_ids.len(), 1);
    let replayed = client
        .prepare_ceremony_children(prepare_children(ceremony_id, Some(estimate)))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(replayed.child_ids, prepared.child_ids);
    let budget = client
        .get_budget_report(GetBudgetReportRequest {
            ceremony_id: ceremony_id.to_owned(),
        })
        .await
        .unwrap()
        .into_inner()
        .balance
        .unwrap();
    assert_eq!(budget.reserved.unwrap().tokens, 5);

    assert_unbudgeted_rejects_estimate(&mut client).await;
}

#[tokio::test]
async fn budgeted_child_preparation_recovers_after_claim_before_plan() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);
    publish_fanout(&mut client).await;
    let ceremony_id = "budget-rpc-claim-before-plan";
    start_fanout(&mut client, ceremony_id, true).await;
    let estimate = BudgetReservationEstimate {
        duration: Some(measurement("unknown", 0)),
        tokens: Some(measurement("estimated", 7)),
        cost: Some(measurement("unknown", 0)),
        tool_calls: Some(measurement("unknown", 0)),
    };
    client
        .claim_ceremony_step(ClaimCeremonyStepRequest {
            ceremony_id: ceremony_id.to_owned(),
            step_id: "fanout".to_owned(),
            actor_kind: "agent".to_owned(),
            lease_owner_id: "budget-rpc-host".to_owned(),
            idempotency_key: "budget-rpc-recovery".to_owned(),
            lease_ttl_ms: 60_000,
            budget_reservation: Some(estimate.clone()),
            execution_profile: None,
        })
        .await
        .unwrap();
    let mut competing = prepare_children(ceremony_id, Some(estimate.clone()));
    competing.lease_owner_id = "budget-rpc-competing-host".to_owned();
    competing.idempotency_key = "budget-rpc-competing".to_owned();
    let refused = client
        .prepare_ceremony_children(competing)
        .await
        .unwrap_err();
    assert_eq!(refused.code(), Code::FailedPrecondition);
    let before_recovery = client
        .get_ceremony_instance(GetCeremonyInstanceRequest {
            ceremony_id: ceremony_id.to_owned(),
        })
        .await
        .unwrap()
        .into_inner()
        .instance
        .unwrap();
    assert_eq!(before_recovery.steps[0].status, "in_progress");
    assert!(before_recovery.child_groups.is_empty());
    let mut retry = prepare_children(ceremony_id, Some(estimate));
    retry.idempotency_key = "budget-rpc-recovery".to_owned();
    let mut changed_lease = retry.clone();
    changed_lease.lease_ttl_ms = 30_000;
    let refused = client
        .prepare_ceremony_children(changed_lease)
        .await
        .unwrap_err();
    assert_eq!(refused.code(), Code::FailedPrecondition);
    let prepared = client
        .prepare_ceremony_children(retry.clone())
        .await
        .unwrap()
        .into_inner();
    assert_eq!(prepared.child_ids.len(), 1);
    let replayed = client
        .prepare_ceremony_children(retry)
        .await
        .unwrap()
        .into_inner();
    assert_eq!(replayed.child_ids, prepared.child_ids);
    let budget = client
        .get_budget_report(GetBudgetReportRequest {
            ceremony_id: ceremony_id.to_owned(),
        })
        .await
        .unwrap()
        .into_inner()
        .balance
        .unwrap();
    assert_eq!(budget.reserved.unwrap().tokens, 7);
}

async fn publish_fanout(client: &mut MadeServiceClient<Channel>) {
    for definition in [CHILD_DEFINITION, PARENT_DEFINITION] {
        client
            .publish_ceremony_definition(PublishCeremonyDefinitionRequest {
                definition_yaml: definition.to_owned(),
            })
            .await
            .unwrap();
    }
}

async fn start_fanout(client: &mut MadeServiceClient<Channel>, ceremony_id: &str, budgeted: bool) {
    client
        .start_published_ceremony(StartPublishedCeremonyRequest {
            ceremony_id: ceremony_id.to_owned(),
            ceremony: "budget_rpc_parent".to_owned(),
            version: "1.0".to_owned(),
            context: Some(prost_types::Struct::default()),
            actor_id: "operator".to_owned(),
            actor_kind: "service".to_owned(),
            budget_limits: budgeted.then_some(BudgetLimits {
                duration_micros: None,
                tokens: Some(100),
                cost_micros: None,
                tool_calls: None,
                currency: String::new(),
            }),
        })
        .await
        .unwrap();
}

async fn assert_unbudgeted_rejects_estimate(client: &mut MadeServiceClient<Channel>) {
    let ceremony_id = "budget-rpc-unbudgeted-fanout";
    start_fanout(client, ceremony_id, false).await;
    let foreign_estimate = BudgetReservationEstimate {
        duration: Some(measurement("unknown", 0)),
        tokens: Some(measurement("estimated", 1)),
        cost: Some(measurement("unknown", 0)),
        tool_calls: Some(measurement("unknown", 0)),
    };
    let refused = client
        .prepare_ceremony_children(prepare_children(ceremony_id, Some(foreign_estimate)))
        .await
        .unwrap_err();
    assert_eq!(refused.code(), Code::InvalidArgument);
    let unchanged = client
        .get_ceremony_instance(GetCeremonyInstanceRequest {
            ceremony_id: ceremony_id.to_owned(),
        })
        .await
        .unwrap()
        .into_inner()
        .instance
        .unwrap();
    assert_eq!(unchanged.steps[0].status, "pending");
}

fn prepare_children(
    ceremony_id: &str,
    budget_reservation: Option<BudgetReservationEstimate>,
) -> PrepareCeremonyChildrenRequest {
    PrepareCeremonyChildrenRequest {
        ceremony_id: ceremony_id.to_owned(),
        step_id: "fanout".to_owned(),
        lease_owner_id: "budget-rpc-host".to_owned(),
        idempotency_key: "budget-rpc-fanout".to_owned(),
        lease_ttl_ms: 60_000,
        actor_kind: "agent".to_owned(),
        budget_reservation,
    }
}

fn start_request(ceremony_id: &str) -> StartPublishedCeremonyRequest {
    StartPublishedCeremonyRequest {
        ceremony_id: ceremony_id.to_owned(),
        ceremony: "editorial_planning_meeting".to_owned(),
        version: "1.0".to_owned(),
        context: Some(prost_types::Struct {
            fields: [(
                "meeting_brief".to_owned(),
                prost_types::Value {
                    kind: Some(prost_types::value::Kind::StringValue(
                        "budget acceptance".to_owned(),
                    )),
                },
            )]
            .into_iter()
            .collect(),
        }),
        actor_id: "operator".to_owned(),
        actor_kind: "service".to_owned(),
        budget_limits: Some(BudgetLimits {
            duration_micros: None,
            tokens: Some(100),
            cost_micros: None,
            tool_calls: None,
            currency: String::new(),
        }),
    }
}

fn claim(ceremony_id: &str, key: &str, tokens: BudgetMeasurement) -> ClaimCeremonyStepRequest {
    ClaimCeremonyStepRequest {
        ceremony_id: ceremony_id.to_owned(),
        step_id: "open_room".to_owned(),
        actor_kind: "agent".to_owned(),
        lease_owner_id: "budget-host".to_owned(),
        idempotency_key: key.to_owned(),
        lease_ttl_ms: 60_000,
        budget_reservation: Some(BudgetReservationEstimate {
            duration: Some(measurement("unknown", 0)),
            tokens: Some(tokens),
            cost: Some(measurement("unknown", 0)),
            tool_calls: Some(measurement("unknown", 0)),
        }),
        execution_profile: None,
    }
}

fn measurement(quality: &str, amount: u64) -> BudgetMeasurement {
    BudgetMeasurement {
        quality: quality.to_owned(),
        amount: Some(amount),
    }
}
