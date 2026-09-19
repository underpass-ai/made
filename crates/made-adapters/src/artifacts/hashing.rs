use std::io::{self, Read};

use made_core::value_objects::ArtifactDigest;
use sha2::{Digest, Sha256};

pub(crate) fn digest_bytes(bytes: &[u8]) -> ArtifactDigest {
    ArtifactDigest::new(format!("sha256:{:x}", Sha256::digest(bytes)))
        .expect("sha256 formatting is canonical")
}

pub(crate) fn digest_reader(mut reader: impl Read) -> io::Result<(ArtifactDigest, u64)> {
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        total += read as u64;
    }
    Ok((
        ArtifactDigest::new(format!("sha256:{:x}", hasher.finalize()))
            .expect("sha256 formatting is canonical"),
        total,
    ))
}

pub(super) fn stable_key(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
