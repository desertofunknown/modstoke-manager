use sha2::{Digest, Sha256};
use std::io::{self, Read};

// Revisions, receipts and file records are stored and compared as lowercase hex, so every digest goes
// through here.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

pub(crate) fn sha256_reader(mut reader: impl Read) -> io::Result<String> {
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let length = reader.read(&mut buffer)?;
        if length == 0 {
            return Ok(format!("{:x}", hash.finalize()));
        }
        hash.update(&buffer[..length]);
    }
}
