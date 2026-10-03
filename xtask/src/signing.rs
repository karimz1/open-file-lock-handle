//! Release-signing preflight uses the same public key and signature format as the updater.
use super::Result;
use base64::{Engine, engine::general_purpose::STANDARD};
use minisign_verify::{PublicKey, Signature};
use std::{fs, path::Path};

fn verify_signature(encoded_public_key: &str, payload: &[u8], signature: &str) -> Result<()> {
    let public_key_text = String::from_utf8(STANDARD.decode(encoded_public_key)?)?;
    let public_key = PublicKey::decode(&public_key_text)?;
    let signature_text = String::from_utf8(STANDARD.decode(signature.trim())?)?;
    let signature = Signature::decode(&signature_text)?;
    public_key.verify(payload, &signature, false)?;
    Ok(())
}

/// Verify a preflight payload with the public key embedded in the desktop configuration.
/// A successful signing command alone does not prove existing installs trust that key.
pub(super) fn verify(config: &Path, payload: &Path, signature: &Path) -> Result<()> {
    let config: serde_json::Value = serde_json::from_slice(&fs::read(config)?)?;
    let public_key = config
        .pointer("/plugins/updater/pubkey")
        .and_then(serde_json::Value::as_str)
        .ok_or("missing updater public key in desktop configuration")?;
    verify_signature(
        public_key,
        &fs::read(payload)?,
        &fs::read_to_string(signature)?,
    )?;
    println!("Updater signing preflight passed.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUBLIC_KEY: &str = include_str!("../tests/fixtures/updater/public.key");
    const PAYLOAD: &[u8] = include_bytes!("../tests/fixtures/updater/payload");
    const SIGNATURE: &str = include_str!("../tests/fixtures/updater/payload.sig");

    #[test]
    fn accepts_matching_key_and_rejects_tampering_and_wrong_app_key() {
        assert!(verify_signature(PUBLIC_KEY.trim(), PAYLOAD, SIGNATURE).is_ok());
        assert!(verify_signature(PUBLIC_KEY.trim(), b"tampered", SIGNATURE).is_err());
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../../crates/oflh-desktop/tauri.conf.json"))
                .unwrap();
        let app_key = config
            .pointer("/plugins/updater/pubkey")
            .unwrap()
            .as_str()
            .unwrap();
        assert!(verify_signature(app_key, PAYLOAD, SIGNATURE).is_err());
        assert!(verify_signature("invalid base64", PAYLOAD, SIGNATURE).is_err());
        assert!(verify_signature(PUBLIC_KEY.trim(), PAYLOAD, "invalid signature").is_err());
    }
}
