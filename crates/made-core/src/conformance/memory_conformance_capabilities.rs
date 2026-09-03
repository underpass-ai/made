use crate::ports::{MemoryReaderPort, MemoryWriterPort};

use super::memory_conformance::MemoryConformance;
use super::MemoryConformanceFailure;

impl MemoryConformance {
    /// Verify that repeated capability reads are stable.
    pub(super) fn capabilities_are_stable(
        writer: &dyn MemoryWriterPort,
        reader: &dyn MemoryReaderPort,
    ) -> Result<(), MemoryConformanceFailure> {
        const PROPERTY: &str = "capabilities_are_stable";
        if writer.capabilities() != writer.capabilities() {
            return Err(MemoryConformanceFailure::new(
                PROPERTY,
                "the writer answered its own capabilities differently twice",
            ));
        }
        if reader.capabilities() != reader.capabilities() {
            return Err(MemoryConformanceFailure::new(
                PROPERTY,
                "the reader answered its own capabilities differently twice",
            ));
        }
        Ok(())
    }
}
