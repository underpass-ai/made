use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{
    BudgetLimits, BudgetMeasurement, BudgetReservationEstimate, ClaimCeremonyStepRequest,
    GetBudgetReportRequest, GetCeremonyInstanceRequest, ListPendingBudgetReservationsRequest,
    PublishCeremonyDefinitionRequest, StartPublishedCeremonyRequest,
};
use made_tests_integration::grpc_fixture::GrpcFixture;
use tonic::Code;

const DEFINITION: &str =
    include_str!("../../../tests/e2e/ceremonies/editorial-planning-meeting.yaml");

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
    }
}

fn measurement(quality: &str, amount: u64) -> BudgetMeasurement {
    BudgetMeasurement {
        quality: quality.to_owned(),
        amount: Some(amount),
    }
}
