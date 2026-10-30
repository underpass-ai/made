#![cfg(feature = "container-postgres")]

use made_adapters::postgres::PostgresProtocolStore;
use made_core::ports::ProtocolInstanceIndexPort;
use made_core::value_objects::{ProtocolId, ProtocolIdPrefix, ProtocolInstancePageLimit};
use made_tests_integration::postgres_fixture;

#[tokio::test]
async fn postgres_index_pages_more_than_a_thousand_rows_with_literal_prefixes() {
    let (pool, url, _container) = postgres_fixture::start_with_url().await;
    let writer = sqlx::PgPool::connect(&url).await.unwrap();
    let mut transaction = writer.begin().await.unwrap();
    for ordinal in (0..1_100_u16).rev() {
        sqlx::query("INSERT INTO protocol_streams(stream_id, version) VALUES ($1, 0)")
            .bind(format!("bulk-{ordinal:04}"))
            .execute(&mut *transaction)
            .await
            .unwrap();
    }
    for id in [
        "literal%group-a",
        "literal%group-b",
        "literalXgroup-c",
        "under_score-a",
        "underXscore-b",
        "東京-a",
        "東京-b",
    ] {
        sqlx::query("INSERT INTO protocol_streams(stream_id, version) VALUES ($1, 0)")
            .bind(id)
            .execute(&mut *transaction)
            .await
            .unwrap();
    }
    transaction.commit().await.unwrap();
    writer.close().await;

    let store = PostgresProtocolStore::new(pool);
    let limit = ProtocolInstancePageLimit::new(100).unwrap();
    let prefix = ProtocolIdPrefix::new("bulk-").unwrap();
    let mut after = None;
    let mut found = Vec::new();
    loop {
        let page = store
            .ids_after(after.as_ref(), Some(&prefix), limit)
            .await
            .unwrap();
        found.extend(page.ids().iter().cloned());
        if !page.has_more() {
            break;
        }
        after = page.ids().last().cloned();
    }
    assert_eq!(found.len(), 1_100);
    assert_eq!(found.first().unwrap().as_str(), "bulk-0000");
    assert_eq!(found.last().unwrap().as_str(), "bulk-1099");

    assert_page(&store, "literal%", &["literal%group-a", "literal%group-b"]).await;
    assert_page(&store, "under_", &["under_score-a"]).await;
    assert_page(&store, "東京", &["東京-a", "東京-b"]).await;
}

async fn assert_page(store: &PostgresProtocolStore, prefix: &str, expected: &[&str]) {
    let page = store
        .ids_after(
            None,
            Some(&ProtocolIdPrefix::new(prefix).unwrap()),
            ProtocolInstancePageLimit::new(100).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        page.ids()
            .iter()
            .map(ProtocolId::as_str)
            .collect::<Vec<_>>(),
        expected
    );
    assert!(!page.has_more());
}
