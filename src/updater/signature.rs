use ed25519_dalek::{Signature, Verifier, VerifyingKey};

/// Optional compile-time pinned Ed25519 public key (64 hex chars / 32 bytes) for verifying `SHA256SUMS.txt.sig`.
pub const RELEASE_SIGNING_PUBKEY_HEX: Option<&str> = option_env!("SHOTCRAFT_RELEASE_PUBKEY");

/// Decode a strict even-length hexadecimal string into raw bytes.
pub fn decode_hex(hex: &str) -> Result<Vec<u8>, String> {
    let trimmed = hex.trim();
    if trimmed.is_empty() || !trimmed.len().is_multiple_of(2) {
        return Err("Invalid hex length".to_string());
    }
    let bytes = trimmed.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.as_chunks::<2>().0 {
        let s = std::str::from_utf8(pair).map_err(|_| "Invalid UTF-8 in hex string".to_string())?;
        let val = u8::from_str_radix(s, 16)
            .map_err(|_| format!("Invalid hexadecimal byte '{s}'"))?;
        out.push(val);
    }
    Ok(out)
}

/// Verify an Ed25519 signature (`sig_hex`, 64 bytes) over `message` against `pubkey_hex` (32 bytes).
pub fn verify_ed25519_signature(
    message: &[u8],
    sig_hex: &str,
    pubkey_hex: &str,
) -> Result<(), String> {
    let pk_bytes =
        decode_hex(pubkey_hex).map_err(|e| format!("Invalid release signing public key: {e}"))?;
    let pk_array: [u8; 32] = pk_bytes
        .try_into()
        .map_err(|_| "Release signing public key must be 32 bytes (64 hex chars)".to_string())?;
    let verifying_key = VerifyingKey::from_bytes(&pk_array)
        .map_err(|e| format!("Malformed Ed25519 public key: {e}"))?;

    let sig_bytes =
        decode_hex(sig_hex).map_err(|e| format!("Invalid release signature hex: {e}"))?;
    let sig_array: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| "Release signature must be 64 bytes (128 hex chars)".to_string())?;
    let signature = Signature::from_bytes(&sig_array);

    verifying_key
        .verify(message, &signature)
        .map_err(|e| format!("Release signature verification failed: {e}"))
}

/// When a signing public key is configured, fail closed unless `sig_text` verifies `sums_bytes`.
pub fn verify_checksums_if_configured(
    sums_bytes: &[u8],
    sig_text: Option<&str>,
    pubkey_hex: Option<&str>,
) -> Result<(), String> {
    let Some(pk) = pubkey_hex.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(());
    };
    let sig = sig_text
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Release is missing SHA256SUMS.txt.sig signature".to_string())?;
    verify_ed25519_signature(sums_bytes, sig, pk)
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use ed25519_dalek::{Signer, SigningKey};

    use super::*;

    fn to_hex(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            let _ = write!(s, "{b:02x}");
        }
        s
    }

    fn deterministic_keypair(seed_byte: u8) -> (SigningKey, String) {
        let signing = SigningKey::from_bytes(&[seed_byte; 32]);
        let pk_hex = to_hex(signing.verifying_key().as_bytes());
        (signing, pk_hex)
    }

    #[test]
    fn ed25519_signature_roundtrip_and_tamper_rejection() {
        let (signing, pk_hex) = deterministic_keypair(0x42);
        let sums = b"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  ShotCraft-v0.6.2.dmg\n";
        let sig = signing.sign(sums);
        let sig_hex = format!("  {}\n", to_hex(&sig.to_bytes()));

        assert!(verify_ed25519_signature(sums, &sig_hex, &pk_hex).is_ok());

        let tampered = b"0000000000000000000000000000000000000000000000000000000000000000  ShotCraft-v0.6.2.dmg\n";
        assert!(verify_ed25519_signature(tampered, &sig_hex, &pk_hex).is_err());

        let (_, other_pk) = deterministic_keypair(0x99);
        assert!(verify_ed25519_signature(sums, &sig_hex, &other_pk).is_err());

        assert!(verify_ed25519_signature(sums, "not-hex", &pk_hex).is_err());
        assert!(verify_ed25519_signature(sums, "abcd", &pk_hex).is_err());
    }

    #[test]
    fn verify_checksums_fails_closed_when_pubkey_configured() {
        let (signing, pk_hex) = deterministic_keypair(0x7A);
        let sums = b"abc  ShotCraft-v0.6.2.dmg\n";
        let sig_hex = to_hex(&signing.sign(sums).to_bytes());

        assert!(verify_checksums_if_configured(sums, None, None).is_ok());
        assert!(verify_checksums_if_configured(sums, None, Some("")).is_ok());

        let missing_err = verify_checksums_if_configured(sums, None, Some(&pk_hex)).unwrap_err();
        assert!(missing_err.contains("SHA256SUMS.txt.sig"));

        assert!(verify_checksums_if_configured(sums, Some(&sig_hex), Some(&pk_hex)).is_ok());
        assert!(
            verify_checksums_if_configured(b"tampered", Some(&sig_hex), Some(&pk_hex)).is_err()
        );
    }
}
