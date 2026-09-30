//! SHA-256 helpers shared by tools.

use crate::constants::IO_BUFFER_BYTES;
use sha2::{Digest, Sha256};
use tokio::{fs::File, io::AsyncReadExt};

pub(crate) const SHA256_DIGEST_BYTES: usize = 32;
const HEX_CHARS_PER_BYTE: usize = 2;
pub(crate) const SHA256_HEX_CHARS: usize = SHA256_DIGEST_BYTES * HEX_CHARS_PER_BYTE;
const HEX_RADIX: usize = 16;
const NIBBLE_BITS: u32 = 4;
const LOW_NIBBLE_MASK: u8 = 0x0f;

pub(crate) async fn sha256_large_file(path: &std::path::Path) -> std::io::Result<String> {
    let mut file = File::open(path).await?;
    if !file.metadata().await?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "path must be a regular file",
        ));
    }
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; IO_BUFFER_BYTES];
    loop {
        let count = file.read(&mut buffer).await?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

const HEX_DIGITS: &[u8; HEX_RADIX] = b"0123456789abcdef";

pub(crate) fn sha256_bytes(data: &[u8]) -> [u8; SHA256_DIGEST_BYTES] {
    let mut digest = [0u8; SHA256_DIGEST_BYTES];
    digest.copy_from_slice(&Sha256::digest(data));
    digest
}

pub(crate) fn sha256_hex(data: &[u8]) -> String {
    let mut hex = String::with_capacity(SHA256_HEX_CHARS);
    for byte in sha256_bytes(data) {
        hex.push(HEX_DIGITS[(byte >> NIBBLE_BITS) as usize] as char);
        hex.push(HEX_DIGITS[(byte & LOW_NIBBLE_MASK) as usize] as char);
    }
    hex
}
