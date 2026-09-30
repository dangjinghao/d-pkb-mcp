//! SHA-256 helpers shared by tools.

use sha2::{Digest, Sha256};
use tokio::{fs::File, io::AsyncReadExt};

pub(crate) async fn sha256_large_file(path: &std::path::Path) -> std::io::Result<String> {
    let mut file = File::open(path).await?;
    if !file.metadata().await?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "path must be a regular file",
        ));
    }
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 65_536];
    loop {
        let count = file.read(&mut buffer).await?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

pub(crate) fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&Sha256::digest(data));
    digest
}

pub(crate) fn sha256_hex(data: &[u8]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in sha256_bytes(data) {
        hex.push(HEX_DIGITS[(byte >> 4) as usize] as char);
        hex.push(HEX_DIGITS[(byte & 0x0f) as usize] as char);
    }
    hex
}
