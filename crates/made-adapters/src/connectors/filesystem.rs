use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use made_core::value_objects::ExecutionOperationId;
use uuid::Uuid;

use super::{
    ConnectorCapabilities, ConnectorCapability, ConnectorDescriptor, ConnectorError, ConnectorKind,
    ReceiptFilesystemConnector, SafeExecutionReceipt,
};

/// Small atomic JSON receipt store for local connector composition.
#[derive(Debug)]
pub struct JsonFileReceiptStore {
    descriptor: ConnectorDescriptor,
    root: PathBuf,
}

impl JsonFileReceiptStore {
    pub fn new(
        id: made_core::value_objects::ExecutionConnectorId,
        root: impl AsRef<Path>,
    ) -> Result<Self, ConnectorError> {
        let root = root.as_ref();
        fs::create_dir_all(root).map_err(|_| ConnectorError::Filesystem)?;
        let root = root
            .canonicalize()
            .map_err(|_| ConnectorError::Filesystem)?;
        let descriptor = ConnectorDescriptor::new(
            id,
            ConnectorKind::Filesystem,
            ConnectorCapabilities::new([
                ConnectorCapability::ReadReceipt,
                ConnectorCapability::WriteReceipt,
                ConnectorCapability::AtomicReceipt,
            ]),
            None,
        )?;
        Ok(Self { descriptor, root })
    }

    fn path(root: &Path, operation_id: &ExecutionOperationId) -> PathBuf {
        root.join(format!("{}.receipt.json", operation_id.as_str()))
    }

    fn load_sync(
        root: &Path,
        operation_id: &ExecutionOperationId,
    ) -> Result<Option<SafeExecutionReceipt>, ConnectorError> {
        let path = Self::path(root, operation_id);
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(ConnectorError::Filesystem),
        };
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| ConnectorError::ReceiptSerialization)
    }

    fn store_sync(root: &Path, receipt: &SafeExecutionReceipt) -> Result<(), ConnectorError> {
        let target = Self::path(root, receipt.operation_id());
        let bytes =
            serde_json::to_vec(receipt).map_err(|_| ConnectorError::ReceiptSerialization)?;
        if target.exists() {
            return match Self::load_sync(root, receipt.operation_id())? {
                Some(existing) if existing == *receipt => Ok(()),
                _ => Err(ConnectorError::ReceiptConflict(
                    receipt.operation_id().clone(),
                )),
            };
        }

        let temporary = root.join(format!(
            ".{}.{}.tmp",
            receipt.operation_id(),
            Uuid::new_v4()
        ));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::hard_link(&temporary, &target)?;
            File::open(root)?.sync_all()
        })();
        let _ = fs::remove_file(&temporary);
        match result {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                match Self::load_sync(root, receipt.operation_id())? {
                    Some(existing) if existing == *receipt => Ok(()),
                    _ => Err(ConnectorError::ReceiptConflict(
                        receipt.operation_id().clone(),
                    )),
                }
            }
            Err(_) => Err(ConnectorError::Filesystem),
        }
    }
}

#[async_trait]
impl ReceiptFilesystemConnector for JsonFileReceiptStore {
    fn descriptor(&self) -> &ConnectorDescriptor {
        &self.descriptor
    }

    async fn load(
        &self,
        operation_id: &ExecutionOperationId,
    ) -> Result<Option<SafeExecutionReceipt>, ConnectorError> {
        let root = self.root.clone();
        let operation_id = operation_id.clone();
        tokio::task::spawn_blocking(move || Self::load_sync(&root, &operation_id))
            .await
            .map_err(|_| ConnectorError::Filesystem)?
    }

    async fn store(&self, receipt: &SafeExecutionReceipt) -> Result<(), ConnectorError> {
        let root = self.root.clone();
        let receipt = receipt.clone();
        tokio::task::spawn_blocking(move || Self::store_sync(&root, &receipt))
            .await
            .map_err(|_| ConnectorError::Filesystem)?
    }
}
