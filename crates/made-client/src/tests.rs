use made_proto::v1::{ProtocolInstanceState, ProtocolLineageState};

use crate::{MadeClientError, ProgressCheckpoint, ProtocolTree, RequestContext};

#[tokio::test]
async fn empty_invocation_id_is_rejected() {
    assert!(matches!(
        RequestContext::from_id("   "),
        Err(MadeClientError::InvalidInvocationId)
    ));
}

#[tokio::test]
async fn checkpoint_round_trips_atomically_and_rejects_another_scope() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&root).unwrap();
    let directory = tempfile::tempdir_in(root).unwrap();
    let path = directory.path().join("nested/cursor.json");
    let checkpoint = ProgressCheckpoint::new("protocol-a", 42);
    checkpoint.save(&path).await.unwrap();
    assert_eq!(
        ProgressCheckpoint::load(&path, "protocol-a").await.unwrap(),
        checkpoint
    );
    assert!(matches!(
        ProgressCheckpoint::load(&path, "protocol-b").await,
        Err(MadeClientError::CursorScopeMismatch { .. })
    ));
}

#[test]
fn tree_orders_siblings_and_rejects_a_cycle() {
    let root = instance("root", "", 0);
    let second = instance("second", "root", 2);
    let first = instance("first", "root", 1);
    let tree = ProtocolTree::from_instances(vec![second, root, first]).unwrap();
    let root = &tree.roots()[0];
    assert_eq!(root.instance().protocol_id, "root");
    assert_eq!(root.children()[0].instance().protocol_id, "first");
    assert_eq!(root.children()[1].instance().protocol_id, "second");

    let cycle = ProtocolTree::from_instances(vec![
        instance("left", "right", 1),
        instance("right", "left", 1),
    ]);
    assert!(matches!(cycle, Err(MadeClientError::ProtocolViolation(_))));
}

fn instance(id: &str, parent: &str, position: u32) -> ProtocolInstanceState {
    ProtocolInstanceState {
        protocol_id: id.to_owned(),
        lineage: Some(ProtocolLineageState {
            root_id: "root".to_owned(),
            parent_id: parent.to_owned(),
            group_id: String::new(),
            position,
            depth: u32::from(!parent.is_empty()),
            remaining_depth: 3,
        }),
        ..ProtocolInstanceState::default()
    }
}
