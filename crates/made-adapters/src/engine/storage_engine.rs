use std::fmt;

use made_core::error::DomainError;

use super::{ReadTx, WriteTx};

/// One opened ceremony store, shareable across tasks.
pub(crate) trait Engine: fmt::Debug + Send + Sync {
    fn begin_read(&self) -> Result<Box<dyn ReadTx + '_>, DomainError>;
    fn begin_write(&self) -> Result<Box<dyn WriteTx + '_>, DomainError>;
}
