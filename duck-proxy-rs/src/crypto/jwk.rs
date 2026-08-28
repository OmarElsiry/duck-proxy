use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::rngs::OsRng;
use rsa::traits::PublicKeyParts;
use rsa::{Oaep, RsaPrivateKey, RsaPublicKey};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use thiserror::Error;

/// Errors arising from cryptographic operations.
#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("RSA key generation failed: {0}")]
    KeyGeneration(#[from] rsa::Error),

    #[error("RSA-OAEP-256 encryption failed: {0}")]
    Encryption(String),

    #[error("RSA-OAEP-256 decryption failed: {0}")]
    Decryption(String),

    #[error("Modulus base64url decode error: {0}")]
    Base64Decode(#[from] base64::DecodeError),

    #[error("Invalid modulus byte length: expected {expected}, got {actual}")]
    InvalidModulusLength { expected: usize, actual: usize },

    #[error("Invalid exponent byte sequence: {0}")]
    InvalidExponent(String),
}

/// RFC 7517 / RFC 7518 compliant JSON Web Key representing an RSA-OAEP-256 public key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JwkPublicKey {
    /// Cryptographic algorithm intended for use with the key.
    pub alg: String,

    /// RSA public exponent (base64url-encoded, e.g., "AQAB").
    pub e: String,

    /// Extractable key indicator (WebCrypto compatibility).
    pub ext: bool,

    /// Intended key operations (e.g. ["encrypt"]).
    pub key_ops: Vec<String>,

    /// Key type (must be "RSA").
    pub kty: String,

    /// RSA modulus (unpadded base64url-encoded 2048-bit big-endian integer).
    pub n: String,

    /// Intended use of the public key (e.g. "enc" for encryption).
    #[serde(rename = "use")]
    pub key_use: String,
}

/// Ephemeral 2048-bit RSA keypair generating an exportable RFC 7517 JWK.
#[derive(Clone, Debug)]
pub struct EphemeralKeypair {
    private_key: RsaPrivateKey,
    public_jwk: JwkPublicKey,
}

impl EphemeralKeypair {
    pub const KEY_SIZE_BITS: usize = 2048;
    pub const MODULUS_BYTE_LEN: usize = 256; // 2048 / 8

    /// Generates a new ephemeral 2048-bit RSA keypair and derives its JWK representation.
    pub fn generate() -> Result<Self, CryptoError> {
        let mut rng = OsRng;
        let private_key =
            RsaPrivateKey::new(&mut rng, Self::KEY_SIZE_BITS).map_err(CryptoError::KeyGeneration)?;
        let public_key = RsaPublicKey::from(&private_key);

        let n_bytes = public_key.n().to_bytes_be();
        let e_bytes = public_key.e().to_bytes_be();

        let n_b64 = URL_SAFE_NO_PAD.encode(&n_bytes);
        let e_b64 = URL_SAFE_NO_PAD.encode(&e_bytes);

        let public_jwk = JwkPublicKey {
            alg: "RSA-OAEP-256".to_string(),
            e: e_b64,
            ext: true,
            key_ops: vec!["encrypt".to_string()],
            kty: "RSA".to_string(),
            n: n_b64,
            key_use: "enc".to_string(),
        };

        Ok(Self {
            private_key,
            public_jwk,
        })
    }

    /// Borrows the precomputed public JWK.
    pub fn public_jwk(&self) -> &JwkPublicKey {
        &self.public_jwk
    }

    /// Borrows the private RSA key.
    pub fn private_key(&self) -> &RsaPrivateKey {
        &self.private_key
    }

    /// Derives a fresh `RsaPublicKey` instance from the private key.
    pub fn public_key(&self) -> RsaPublicKey {
        RsaPublicKey::from(&self.private_key)
    }

    /// Decrypts ciphertext encrypted with RSA-OAEP-256 (MGF1 with SHA-256).
    pub fn decrypt_oaep_sha256(&self, ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let padding = Oaep::new::<Sha256>();
        self.private_key
            .decrypt(padding, ciphertext)
            .map_err(|e| CryptoError::Decryption(e.to_string()))
    }

    /// Encrypts plaintext using the public key with RSA-OAEP-256.
    pub fn encrypt_oaep_sha256(&self, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let mut rng = OsRng;
        let padding = Oaep::new::<Sha256>();
        self.public_key()
            .encrypt(&mut rng, padding, plaintext)
            .map_err(|e| CryptoError::Encryption(e.to_string()))
    }
}

impl Default for EphemeralKeypair {
    fn default() -> Self {
        Self::generate().expect("Failed to generate default 2048-bit RSA ephemeral keypair")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;

    #[test]
    fn test_ephemeral_keypair_generation_metadata() {
        let keypair = EphemeralKeypair::generate().expect("Keypair generation failed");
        let jwk = keypair.public_jwk();

        assert_eq!(jwk.alg, "RSA-OAEP-256");
        assert_eq!(jwk.e, "AQAB");
        assert!(jwk.ext);
        assert_eq!(jwk.key_ops, vec!["encrypt".to_string()]);
        assert_eq!(jwk.kty, "RSA");
        assert_eq!(jwk.key_use, "enc");
    }

    #[test]
    fn test_jwk_modulus_length_and_no_padding() {
        let keypair = EphemeralKeypair::generate().expect("Keypair generation failed");
        let jwk = keypair.public_jwk();

        // 1. Must not contain base64 '=' padding
        assert!(
            !jwk.n.contains('='),
            "JWK modulus must be unpadded base64url"
        );
        assert!(
            !jwk.e.contains('='),
            "JWK exponent must be unpadded base64url"
        );

        // 2. Modulus string length: ceil(256 * 4 / 3) = 342 characters
        assert_eq!(
            jwk.n.len(),
            342,
            "2048-bit modulus base64url length must be 342 chars"
        );

        // 3. Modulus decoded byte length must be exactly 256 bytes (2048 bits)
        let modulus_bytes = URL_SAFE_NO_PAD
            .decode(&jwk.n)
            .expect("Failed to decode base64url modulus");
        assert_eq!(
            modulus_bytes.len(),
            256,
            "Modulus byte length must be exactly 256 bytes (2048 bits)"
        );

        // 4. Exponent decoded bytes must be [1, 0, 1] (65537)
        let exp_bytes = URL_SAFE_NO_PAD
            .decode(&jwk.e)
            .expect("Failed to decode base64url exponent");
        assert_eq!(exp_bytes, vec![1, 0, 1]);
    }

    #[test]
    fn test_jwk_json_serialization_exact_schema() {
        let keypair = EphemeralKeypair::generate().expect("Keypair generation failed");
        let jwk = keypair.public_jwk();

        let json_str = serde_json::to_string(jwk).expect("Serialization failed");
        let json_val: serde_json::Value =
            serde_json::from_str(&json_str).expect("JSON parse failed");

        // Verify exact field names
        assert_eq!(json_val["alg"], "RSA-OAEP-256");
        assert_eq!(json_val["e"], "AQAB");
        assert_eq!(json_val["ext"], true);
        assert_eq!(json_val["key_ops"], serde_json::json!(["encrypt"]));
        assert_eq!(json_val["kty"], "RSA");
        assert_eq!(json_val["use"], "enc");
        assert!(json_val["n"].is_string());

        // Ensure "key_use" is renamed to "use" and "key_use" does NOT appear
        assert!(json_val.get("key_use").is_none());

        // Roundtrip deserialization
        let deserialized: JwkPublicKey =
            serde_json::from_str(&json_str).expect("Deserialization failed");
        assert_eq!(&deserialized, jwk);
    }

    #[test]
    fn test_oaep_sha256_encryption_decryption_roundtrip() {
        let keypair = EphemeralKeypair::generate().expect("Keypair generation failed");
        let plaintext = b"duck_stream_durable_session_token_123456789";

        let ciphertext = keypair
            .encrypt_oaep_sha256(plaintext)
            .expect("Encryption failed");

        // Ciphertext for 2048-bit RSA must be 256 bytes
        assert_eq!(ciphertext.len(), 256);

        let decrypted = keypair
            .decrypt_oaep_sha256(&ciphertext)
            .expect("Decryption failed");

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_keypair_uniqueness() {
        let keypair1 = EphemeralKeypair::generate().expect("Keypair 1 failed");
        let keypair2 = EphemeralKeypair::generate().expect("Keypair 2 failed");

        assert_ne!(keypair1.public_jwk().n, keypair2.public_jwk().n);
    }
}
