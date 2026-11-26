use std::fmt::Write as _;
use std::fs;
use std::io::{BufReader, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

#[cfg(test)]
pub fn sha256_hex(data: &[u8]) -> String {
    format_digest(&Sha256::digest(data))
}

fn format_digest(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

/// Stream-hash `path` in 16 KiB chunks via `sha2::Sha256` and return its lowercase 64-character hex digest.
pub fn sha256_file(path: &Path) -> Result<String, String> {
    let file =
        fs::File::open(path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 16 * 1024];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Read error while hashing {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format_digest(&hasher.finalize()))
}

/// Extract the lowercase 64-char SHA-256 hex digest for `asset_name` from a `SHA256SUMS.txt` payload.
pub fn parse_sha256sums(content: &str, asset_name: &str) -> Option<String> {
    let target = asset_name.trim().trim_start_matches("./");
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some(ws_idx) = trimmed.find(|c: char| c.is_ascii_whitespace()) else {
            continue;
        };
        let hash = &trimmed[..ws_idx];
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        let file_part = trimmed[ws_idx..]
            .trim_start()
            .trim_start_matches('*')
            .trim_start_matches("./");
        if file_part == target {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
}

/// Verify a file on disk matches the expected 64-character SHA-256 hex digest.
pub fn verify_file_sha256(path: &Path, expected_hex: &str) -> Result<(), String> {
    let expected = expected_hex.trim();
    if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid expected SHA-256 digest".to_string());
    }
    let actual = sha256_file(path)?;
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(format!(
            "SHA-256 checksum mismatch (expected {}, got {actual})",
            expected.to_ascii_lowercase()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_nist_and_block_boundary_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(&[b'a'; 55]),
            "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318"
        );
        assert_eq!(
            sha256_hex(&[b'a'; 56]),
            "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a"
        );
        assert_eq!(
            sha256_hex(&[b'a'; 64]),
            "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb"
        );
        assert_eq!(
            sha256_hex(&[b'a'; 1000]),
            "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn sums_parsing_and_file_verification() {
        let sums = "\
# Release checksums\r\n\
E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855  ShotCraft-v0.6.2.dmg\r\n\
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad *./ShotCraft-v0.6.2-Setup.exe\r\n\
badhash  ShotCraft-v0.6.2-broken.exe\r\n";
        assert_eq!(
            parse_sha256sums(sums, "ShotCraft-v0.6.2.dmg").as_deref(),
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );
        assert_eq!(
            parse_sha256sums(sums, "ShotCraft-v0.6.2-Setup.exe").as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        assert!(parse_sha256sums(sums, "ShotCraft-v0.6.2-broken.exe").is_none());
        assert!(parse_sha256sums(sums, "missing.deb").is_none());

        let tmp =
            std::env::temp_dir().join(format!("shotcraft_sha_test_{}.bin", std::process::id()));
        fs::write(&tmp, b"abc").unwrap();
        assert!(verify_file_sha256(
            &tmp,
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
        )
        .is_ok());
        assert!(verify_file_sha256(
            &tmp,
            "0000000000000000000000000000000000000000000000000000000000000000"
        )
        .is_err());
        let _ = fs::remove_file(&tmp);
    }
}
