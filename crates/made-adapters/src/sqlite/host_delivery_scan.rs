//! Paged reading of the delivery tables.
//!
//! Its own file because every scan here has the same shape and the same
//! trap: what a caller asked for is decided after a row is decoded, so
//! a scan that stopped at the first page would quietly answer "nothing
//! waiting" for a destination with a hundred closed deliveries and one
//! open.

use made_core::error::DomainError;

use crate::engine::{ReadTx, Table};

use super::ceremony_store::decode;
use super::keys::host_delivery_target_prefix;
use crate::delivery::StoredHostDelivery;

/// How many rows one turn of a scan reads before deciding anything.
const PAGE: usize = 256;

/// Walk one table, keeping what `wanted` admits, up to `limit`.
pub(super) fn collect<F>(
    tx: &dyn ReadTx,
    table: Table,
    prefix: Option<&str>,
    after: Option<String>,
    limit: usize,
    wanted: F,
) -> Result<Vec<StoredHostDelivery>, DomainError>
where
    F: Fn(&StoredHostDelivery) -> bool,
{
    let mut kept = Vec::new();
    let mut cursor = after;
    loop {
        let rows = tx.scan_str_page(table, cursor.as_deref(), prefix, PAGE)?;
        if rows.is_empty() {
            return Ok(kept);
        }
        cursor = rows.last().map(|(key, _)| key.clone());
        for (_, value) in rows {
            let stored = decode_row(table, &value, tx)?;
            let Some(stored) = stored else {
                continue;
            };
            if wanted(&stored) {
                kept.push(stored);
                if kept.len() == limit {
                    return Ok(kept);
                }
            }
        }
    }
}

/// Every delivery addressed to one destination that `wanted` admits.
pub(super) fn to_destination<F>(
    tx: &dyn ReadTx,
    target_key: &str,
    limit: usize,
    wanted: F,
) -> Result<Vec<StoredHostDelivery>, DomainError>
where
    F: Fn(&StoredHostDelivery) -> bool,
{
    let prefix = host_delivery_target_prefix(target_key);
    collect(
        tx,
        Table::HostDeliveryTargets,
        Some(&prefix),
        None,
        limit,
        wanted,
    )
}

/// A row of either table as the delivery it stands for.
///
/// The destination index holds identifiers, not deliveries: one
/// delivery has one row of truth, and an index that held a copy would
/// be a second one to keep honest.
fn decode_row(
    table: Table,
    value: &[u8],
    tx: &dyn ReadTx,
) -> Result<Option<StoredHostDelivery>, DomainError> {
    if table == Table::HostDeliveries {
        return decode(value, "decode host delivery").map(Some);
    }
    let id: String = decode(value, "decode host delivery index")?;
    tx.get(Table::HostDeliveries, crate::engine::Key::Str(&id))?
        .map(|bytes| decode(&bytes, "decode host delivery"))
        .transpose()
}
