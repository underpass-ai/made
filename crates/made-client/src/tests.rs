use made_proto::v1::{CeremonyInstanceState, CeremonyLineageState};

use crate::{CeremonyTree, MadeClientError, ProgressCheckpoint};

#[tokio::test]
async fn checkpoint_round_trips_atomically_and_rejects_another_scope() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&root).unwrap();
    let directory = tempfile::tempdir_in(root).unwrap();
    let path = directory.path().join("nested/cursor.json");
    let checkpoint = ProgressCheckpoint::new("ceremony-a", 42);
    checkpoint.save(&path).await.unwrap();
    assert_eq!(
        ProgressCheckpoint::load(&path, "ceremony-a").await.unwrap(),
        checkpoint
    );
    assert!(matches!(
        ProgressCheckpoint::load(&path, "ceremony-b").await,
        Err(MadeClientError::CursorScopeMismatch { .. })
    ));
}

#[test]
fn tree_orders_siblings_and_rejects_a_cycle() {
    let root = instance("root", "", 0);
    let second = instance("second", "root", 2);
    let first = instance("first", "root", 1);
    let tree = CeremonyTree::from_instances(vec![second, root, first]).unwrap();
    let root = &tree.roots()[0];
    assert_eq!(root.instance().ceremony_id, "root");
    assert_eq!(root.children()[0].instance().ceremony_id, "first");
    assert_eq!(root.children()[1].instance().ceremony_id, "second");

    let cycle = CeremonyTree::from_instances(vec![
        instance("left", "right", 1),
        instance("right", "left", 1),
    ]);
    assert!(matches!(cycle, Err(MadeClientError::ProtocolViolation(_))));
}

fn instance(id: &str, parent: &str, position: u32) -> CeremonyInstanceState {
    CeremonyInstanceState {
        ceremony_id: id.to_owned(),
        lineage: Some(CeremonyLineageState {
            root_id: "root".to_owned(),
            parent_id: parent.to_owned(),
            group_id: String::new(),
            position,
            depth: u32::from(!parent.is_empty()),
            remaining_depth: 3,
        }),
        ..CeremonyInstanceState::default()
    }
}
