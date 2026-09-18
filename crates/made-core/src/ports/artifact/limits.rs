/// Default upload and read chunk size: 64 KiB.
pub const ARTIFACT_DEFAULT_CHUNK_BYTES: u32 = 64 * 1024;
/// Hard upload and read chunk limit: 1 MiB.
pub const ARTIFACT_MAX_CHUNK_BYTES: u32 = 1024 * 1024;
/// Hard artifact size limit for Corte 5: 1 GiB.
pub const ARTIFACT_MAX_BYTES: u64 = 1024 * 1024 * 1024;
/// Hard metadata page limit.
pub const ARTIFACT_MAX_PAGE_ITEMS: u16 = 100;
