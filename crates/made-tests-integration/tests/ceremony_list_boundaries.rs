//! Shared list semantics on direct RPC, both MCP backends and the typed facade.

use std::fmt::Write;
use std::sync::Arc;

use made_adapters::memory::InMemoryCeremonyEventStore;
use made_app::usecases::{
    CeremonyReportIds, DeferCeremonyGuardInput, GenerateCeremonyReportInput,
    RequestCeremonyInterventionInput,
};
use made_core::ports::CeremonyEventStorePort;
use made_core::value_objects::{
    Attributes, AuditActorKind, CeremonyGuardDeferralContent, CeremonyId,
    CeremonyInterventionContent, CeremonyInterventionId, CeremonyInterventionKind,
    CeremonyInterventionTarget, GuardName, InterventionRoleIds, ReconsiderationConditions, RoleId,
};
use made_embedded::EmbeddedMade;
use made_mcp::backend::MadeMcpGrpcTlsConfig;
use made_mcp::{EmbeddedMadeMcpBackend, GrpcMadeMcpBackend, MadeMcpServer};
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{
    DeferCeremonyGuardRequest, GenerateCeremonyReportRequest, RequestCeremonyInterventionRequest,
};
use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};
use serde_json::{json, Value};
use tonic::Code;

struct Surfaces {
    fixture: GrpcFixture,
    servers: [MadeMcpServer; 2],
    facade: EmbeddedMade,
    stores: [Arc<InMemoryCeremonyEventStore>; 2],
}

impl Surfaces {
    async fn start(count: usize) -> Self {
        let stores = [
            Arc::new(InMemoryCeremonyEventStore::new()),
            Arc::new(InMemoryCeremonyEventStore::new()),
        ];
        let fixture = GrpcFixture::start_with(
            GrpcFixtureWiring::new().with_ceremony_store(stores[0].clone()),
        )
        .await;
        let facade = EmbeddedMade::builder()
            .with_ceremony_store(stores[1].clone())
            .build();
        let servers = [
            MadeMcpServer::with_backend(GrpcMadeMcpBackend::new(
                format!("http://{}", fixture.addr),
                MadeMcpGrpcTlsConfig::disabled(),
            )),
            MadeMcpServer::with_backend(EmbeddedMadeMcpBackend::new(facade.clone())),
        ];
        let mut definition = String::from(
            "version: '1.0'\nname: list_boundaries\nstates:\n  - {id: OPEN, initial: true}\n  - {id: DONE, terminal: true}\ntransitions:\n  - {from: OPEN, to: DONE, trigger: finish, guards: [ready]}\nguards:\n  ready: {type: human, check: manual_approval}\nroles:\n",
        );
        for role in roles(101) {
            writeln!(definition, "  - {{id: {role}, allowed_actions: [finish, request_intervention, respond_to_intervention]}}").unwrap();
        }
        for server in &servers {
            for id in sessions(count) {
                let result = call(
                    server,
                    "made_start_ceremony",
                    json!({
                        "ceremony_id": id, "actor_id": "operator", "actor_kind": "human",
                        "definition_yaml": definition,
                    }),
                )
                .await;
                assert_success(&result);
            }
        }
        Self {
            fixture,
            servers,
            facade,
            stores,
        }
    }

    async fn head(&self) -> Vec<u64> {
        let mut heads = Vec::new();
        for store in &self.stores {
            heads.push(store.head(&ceremony()).await.unwrap().value());
        }
        heads
    }
}

fn ceremony() -> CeremonyId {
    CeremonyId::new("session-000").unwrap()
}
fn role() -> RoleId {
    RoleId::new("ROLE-000").unwrap()
}
fn sessions(count: usize) -> Vec<String> {
    (0..count).map(|i| format!("session-{i:03}")).collect()
}
fn roles(count: usize) -> Vec<String> {
    (0..count).map(|i| format!("ROLE-{i:03}")).collect()
}
fn conditions(count: usize) -> Vec<String> {
    (0..count).map(|i| format!("condition-{i}")).collect()
}

async fn call(server: &MadeMcpServer, tool: &str, arguments: Value) -> Value {
    let request = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": tool, "arguments": arguments}});
    let response = server.handle_json_line(&request.to_string()).await.unwrap();
    serde_json::from_str::<Value>(&response).unwrap()["result"].clone()
}

fn assert_success(result: &Value) {
    assert_ne!(result["isError"], true, "{result}");
}

fn assert_invalid(result: &Value) {
    assert_eq!(result["isError"], true, "{result}");
    assert_eq!(
        result["structuredContent"]["code"], "invalid_request",
        "{result}"
    );
    assert_eq!(result["structuredContent"]["retryable"], false, "{result}");
}

fn deferral_args(values: &[String]) -> Value {
    json!({"ceremony_id": "session-000", "guard_name": "ready", "role_id": "ROLE-000",
        "role_kind": "human", "statement": "Not yet.", "reason": "Need evidence.",
        "reconsider_when": values})
}

fn intervention_args(id: &str, values: &[String]) -> Value {
    json!({"ceremony_id": "session-000", "intervention_id": id, "role_id": "ROLE-000",
        "role_kind": "human", "kind": "opinion", "message": "Please review.",
        "target_role_ids": values})
}

fn deferral_input(content: CeremonyGuardDeferralContent) -> DeferCeremonyGuardInput {
    DeferCeremonyGuardInput::new(
        ceremony(),
        GuardName::new("ready").unwrap(),
        content,
        role(),
        AuditActorKind::Human,
    )
}

fn intervention_input(
    id: &str,
    target: CeremonyInterventionTarget,
) -> RequestCeremonyInterventionInput {
    RequestCeremonyInterventionInput::new(
        ceremony(),
        CeremonyInterventionId::new(id).unwrap(),
        role(),
        AuditActorKind::Human,
        CeremonyInterventionKind::Opinion,
        target,
        CeremonyInterventionContent::new("Please review.", Attributes::empty()).unwrap(),
    )
}

#[tokio::test]
async fn report_lists_are_nonempty_bounded_distinct_and_preserve_order_on_all_surfaces() {
    assert_eq!(CeremonyReportIds::MAX_ITEMS, 100);
    let surfaces = Surfaces::start(100).await;
    let mut direct = MadeServiceClient::new(surfaces.fixture.channel.clone());
    let head = surfaces.head().await;
    for invalid in [
        Vec::new(),
        sessions(101),
        vec!["session-000".into(); 2],
        vec!["session-000".into(), " session-000 ".into()],
    ] {
        let error = direct
            .generate_ceremony_report(GenerateCeremonyReportRequest {
                ceremony_ids: invalid.clone(),
                title: String::new(),
            })
            .await
            .unwrap_err();
        assert_eq!(error.code(), Code::InvalidArgument);
        for server in &surfaces.servers {
            assert_invalid(
                &call(
                    server,
                    "made_generate_ceremony_report",
                    json!({"ceremony_ids": invalid}),
                )
                .await,
            );
        }
        assert!(GenerateCeremonyReportInput::new(
            invalid
                .into_iter()
                .map(|id| CeremonyId::new(id).unwrap())
                .collect(),
            None
        )
        .is_err());
    }
    let mut at_cap = sessions(100);
    at_cap.reverse();
    let report = direct
        .generate_ceremony_report(GenerateCeremonyReportRequest {
            ceremony_ids: at_cap.clone(),
            title: String::new(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(report.ceremony_ids, at_cap);
    for server in &surfaces.servers {
        let result = call(
            server,
            "made_generate_ceremony_report",
            json!({"ceremony_ids": at_cap}),
        )
        .await;
        assert_success(&result);
        assert_eq!(result["structuredContent"]["ceremony_ids"], json!(at_cap));
    }
    let report = surfaces
        .facade
        .report(
            GenerateCeremonyReportInput::new(
                at_cap
                    .iter()
                    .map(|id| CeremonyId::new(id).unwrap())
                    .collect(),
                None,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        report
            .bindings()
            .iter()
            .map(|binding| binding.ceremony_id().as_str())
            .collect::<Vec<_>>(),
        at_cap.iter().map(String::as_str).collect::<Vec<_>>()
    );
    assert_eq!(surfaces.head().await, head, "reports never append");
}

#[tokio::test]
async fn reconsideration_lists_reject_empty_oversized_and_normalized_duplicates_atomically() {
    assert_eq!(ReconsiderationConditions::MAX_ITEMS, 100);
    let surfaces = Surfaces::start(2).await;
    let mut direct = MadeServiceClient::new(surfaces.fixture.channel.clone());
    let head = surfaces.head().await;
    for invalid in [
        Vec::new(),
        conditions(101),
        vec!["wait".into(); 2],
        vec!["wait".into(), " wait ".into()],
    ] {
        let error = direct
            .defer_ceremony_guard(DeferCeremonyGuardRequest {
                ceremony_id: "session-000".into(),
                guard_name: "ready".into(),
                role_id: "ROLE-000".into(),
                role_kind: "human".into(),
                statement: "Not yet.".into(),
                reason: "Need evidence.".into(),
                reconsider_when: invalid.clone(),
            })
            .await
            .unwrap_err();
        assert_eq!(error.code(), Code::InvalidArgument);
        for server in &surfaces.servers {
            assert_invalid(
                &call(server, "made_defer_ceremony_guard", deferral_args(&invalid)).await,
            );
        }
        assert!(
            CeremonyGuardDeferralContent::new("Not yet.", "Need evidence.", invalid.clone())
                .is_err()
        );
        // Historical serde is deliberately tolerant, but it is no bypass into a new command.
        let restored = serde_json::from_value(json!({"statement": "Not yet.", "reason": "Need evidence.", "reconsider_when": invalid})).unwrap();
        assert!(surfaces
            .facade
            .defer_guard(deferral_input(restored))
            .await
            .is_err());
    }
    assert_eq!(surfaces.head().await, head);
    let at_cap = conditions(100);
    direct
        .defer_ceremony_guard(DeferCeremonyGuardRequest {
            ceremony_id: "session-001".into(),
            guard_name: "ready".into(),
            role_id: "ROLE-000".into(),
            role_kind: "human".into(),
            statement: "Not yet.".into(),
            reason: "Need evidence.".into(),
            reconsider_when: at_cap.clone(),
        })
        .await
        .unwrap();
    let mut mcp_deferral = deferral_args(&at_cap);
    mcp_deferral["statement"] = json!("MCP waits for evidence.");
    for server in &surfaces.servers {
        assert_success(&call(server, "made_defer_ceremony_guard", mcp_deferral.clone()).await);
    }
    surfaces
        .facade
        .defer_guard(DeferCeremonyGuardInput::new(
            CeremonyId::new("session-001").unwrap(),
            GuardName::new("ready").unwrap(),
            CeremonyGuardDeferralContent::new(
                "Facade waits for evidence.",
                "Need evidence.",
                at_cap,
            )
            .unwrap(),
            role(),
            AuditActorKind::Human,
        ))
        .await
        .unwrap();
}

#[tokio::test]
async fn recipients_are_bounded_distinct_and_empty_means_table_on_all_surfaces() {
    assert_eq!(InterventionRoleIds::MAX_ITEMS, 100);
    let surfaces = Surfaces::start(1).await;
    let mut direct = MadeServiceClient::new(surfaces.fixture.channel.clone());
    let head = surfaces.head().await;
    for invalid in [
        roles(101),
        vec!["ROLE-000".into(); 2],
        vec!["ROLE-000".into(), " ROLE-000 ".into()],
    ] {
        let error = direct
            .request_ceremony_intervention(RequestCeremonyInterventionRequest {
                ceremony_id: "session-000".into(),
                intervention_id: "invalid".into(),
                role_id: "ROLE-000".into(),
                role_kind: "human".into(),
                kind: "opinion".into(),
                message: "Please review.".into(),
                target_role_ids: invalid.clone(),
                ..Default::default()
            })
            .await
            .unwrap_err();
        assert_eq!(error.code(), Code::InvalidArgument);
        for server in &surfaces.servers {
            assert_invalid(
                &call(
                    server,
                    "made_request_ceremony_intervention",
                    intervention_args("invalid", &invalid),
                )
                .await,
            );
        }
        assert!(CeremonyInterventionTarget::roles(
            invalid.into_iter().map(|id| RoleId::new(id).unwrap())
        )
        .is_err());
    }
    for legacy_roles in [Vec::new(), roles(101)] {
        let restored =
            serde_json::from_value(json!({"kind": "roles", "role_ids": legacy_roles})).unwrap();
        assert!(surfaces
            .facade
            .request_intervention(intervention_input("invalid-legacy", restored))
            .await
            .is_err());
    }
    assert_eq!(surfaces.head().await, head);
    for (label, values) in [("table", Vec::new()), ("cap", roles(100))] {
        direct
            .request_ceremony_intervention(RequestCeremonyInterventionRequest {
                ceremony_id: "session-000".into(),
                intervention_id: format!("direct-{label}"),
                role_id: "ROLE-000".into(),
                role_kind: "human".into(),
                kind: "opinion".into(),
                message: "Please review.".into(),
                target_role_ids: values.clone(),
                ..Default::default()
            })
            .await
            .unwrap();
        for server in &surfaces.servers {
            assert_success(
                &call(
                    server,
                    "made_request_ceremony_intervention",
                    intervention_args(&format!("mcp-{label}"), &values),
                )
                .await,
            );
        }
        let target = if values.is_empty() {
            CeremonyInterventionTarget::table()
        } else {
            CeremonyInterventionTarget::roles(values.into_iter().map(|id| RoleId::new(id).unwrap()))
                .unwrap()
        };
        surfaces
            .facade
            .request_intervention(intervention_input(&format!("facade-{label}"), target))
            .await
            .unwrap();
    }
    assert_recipient_scopes(&surfaces.facade).await;
}

async fn assert_recipient_scopes(facade: &EmbeddedMade) {
    let instance = facade.instance(&ceremony()).await.unwrap();
    for id in ["mcp-table", "facade-table"] {
        assert!(instance
            .intervention(&CeremonyInterventionId::new(id).unwrap())
            .unwrap()
            .target()
            .is_table());
    }
    for id in ["mcp-cap", "facade-cap"] {
        assert_eq!(
            instance
                .intervention(&CeremonyInterventionId::new(id).unwrap())
                .unwrap()
                .target()
                .role_ids()
                .unwrap()
                .len(),
            100
        );
    }
}
