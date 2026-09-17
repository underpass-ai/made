use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Mutex;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{CeremonyEventTransportPort, PositionedRecord};

use crate::ceremony_event_wire::CeremonyEventWire;

/// Append-only JSON Lines sink for embedded ceremony-event publication.
#[derive(Debug)]
pub struct JsonLinesCeremonyEventSink {
    writer: Mutex<BufWriter<File>>,
}

impl JsonLinesCeremonyEventSink {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|_| DomainError::InvariantViolated {
                reason: "ceremony event sink file could not be opened",
            })?;
        Ok(Self {
            writer: Mutex::new(BufWriter::new(file)),
        })
    }
}

#[async_trait]
impl CeremonyEventTransportPort for JsonLinesCeremonyEventSink {
    async fn deliver(&self, record: &PositionedRecord) -> Result<(), DomainError> {
        let encoded = serde_json::to_vec(&CeremonyEventWire::from(record)).map_err(|_| {
            DomainError::InvariantViolated {
                reason: "ceremony event sink record could not be encoded",
            }
        })?;
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| DomainError::InvariantViolated {
                reason: "ceremony event sink lock is poisoned",
            })?;
        writer
            .write_all(&encoded)
            .and_then(|()| writer.write_all(b"\n"))
            .and_then(|()| writer.flush())
            .map_err(|_| DomainError::InvariantViolated {
                reason: "ceremony event sink record could not be appended",
            })
    }
}
