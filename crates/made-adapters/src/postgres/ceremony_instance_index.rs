use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{CeremonyInstanceIdPage, CeremonyInstanceIndexPort};
use made_core::value_objects::{CeremonyId, CeremonyIdPrefix, CeremonyInstancePageLimit};
use sqlx::Row;

use super::ceremony_store::sqlx_error;
use super::PostgresCeremonyStore;

const ALL_QUERY: &str = "SELECT stream_id FROM ceremony_streams \
    ORDER BY stream_id COLLATE \"C\" LIMIT $1";
const AFTER_QUERY: &str = "SELECT stream_id FROM ceremony_streams \
    WHERE stream_id COLLATE \"C\" > $1 COLLATE \"C\" \
    ORDER BY stream_id COLLATE \"C\" LIMIT $2";
const PREFIX_QUERY: &str = "SELECT stream_id FROM ceremony_streams \
    WHERE stream_id LIKE $1 ESCAPE E'\\\\' \
    ORDER BY stream_id COLLATE \"C\" LIMIT $2";
const AFTER_PREFIX_QUERY: &str = "SELECT stream_id FROM ceremony_streams \
    WHERE stream_id COLLATE \"C\" > $1 COLLATE \"C\" \
    AND stream_id LIKE $2 ESCAPE E'\\\\' \
    ORDER BY stream_id COLLATE \"C\" LIMIT $3";

#[async_trait]
impl CeremonyInstanceIndexPort for PostgresCeremonyStore {
    async fn ids_after(
        &self,
        after: Option<&CeremonyId>,
        id_prefix: Option<&CeremonyIdPrefix>,
        limit: CeremonyInstancePageLimit,
    ) -> Result<CeremonyInstanceIdPage, DomainError> {
        let fetch_limit = i64::try_from(limit.value().saturating_add(1)).map_err(|_| {
            DomainError::InvariantViolated {
                reason: "postgres: ceremony instance page limit exceeds bigint",
            }
        })?;
        let prefix_pattern = id_prefix.map(escaped_prefix_pattern);
        let rows = match (after, prefix_pattern.as_deref()) {
            (None, None) => {
                sqlx::query(ALL_QUERY)
                    .bind(fetch_limit)
                    .fetch_all(self.pool.inner())
                    .await
            }
            (Some(after), None) => {
                sqlx::query(AFTER_QUERY)
                    .bind(after.as_str())
                    .bind(fetch_limit)
                    .fetch_all(self.pool.inner())
                    .await
            }
            (None, Some(prefix)) => {
                sqlx::query(PREFIX_QUERY)
                    .bind(prefix)
                    .bind(fetch_limit)
                    .fetch_all(self.pool.inner())
                    .await
            }
            (Some(after), Some(prefix)) => {
                sqlx::query(AFTER_PREFIX_QUERY)
                    .bind(after.as_str())
                    .bind(prefix)
                    .bind(fetch_limit)
                    .fetch_all(self.pool.inner())
                    .await
            }
        }
        .map_err(|error| sqlx_error(error, "page ceremony stream index"))?;

        let mut ids = rows
            .into_iter()
            .map(|row| {
                let stream_id: String = row
                    .try_get("stream_id")
                    .map_err(|error| sqlx_error(error, "decode ceremony stream index id"))?;
                CeremonyId::new(stream_id)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let has_more = ids.len() > limit.value();
        ids.truncate(limit.value());
        Ok(CeremonyInstanceIdPage::new(ids, has_more))
    }
}

fn escaped_prefix_pattern(prefix: &CeremonyIdPrefix) -> String {
    let mut pattern = String::with_capacity(prefix.as_str().len().saturating_add(1));
    for character in prefix.as_str().chars() {
        if matches!(character, '\\' | '%' | '_') {
            pattern.push('\\');
        }
        pattern.push(character);
    }
    pattern.push('%');
    pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_like_prefix_escapes_every_pattern_character() {
        let prefix = CeremonyIdPrefix::new(r"team%_\東京").unwrap();
        assert_eq!(escaped_prefix_pattern(&prefix), r"team\%\_\\東京%");
    }
}
