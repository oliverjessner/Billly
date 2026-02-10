use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use ring::{aead, pbkdf2, rand::{SecureRandom, SystemRandom}};
use std::num::NonZeroU32;

const APP_SECRET: &[u8] = b"billly-secret-v1";
const PBKDF2_ITERATIONS: u32 = 100_000;
const NONCE_LEN: usize = 12;
const SALT_LEN: usize = 16;

pub struct CryptoService;

impl CryptoService {
    pub fn encrypt_api_key(api_key: &str) -> Result<String> {
        Self::encrypt_symmetric(api_key)
    }

    pub fn decrypt_api_key(encrypted: &str) -> Result<String> {
        if encrypted.starts_with("enc:") {
            return Self::decrypt_symmetric(encrypted);
        }
        if encrypted.starts_with("keychain:") {
            return Err(anyhow!("Keychain storage is disabled"));
        }
        Err(anyhow!("Unknown encrypted format"))
    }

    fn encrypt_symmetric(plaintext: &str) -> Result<String> {
        let rng = SystemRandom::new();
        let mut salt = [0u8; SALT_LEN];
        rng.fill(&mut salt)
            .map_err(|_| anyhow!("Failed to generate salt"))?;

        let key = derive_key(&salt)?;
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rng.fill(&mut nonce_bytes)
            .map_err(|_| anyhow!("Failed to generate nonce"))?;

        let nonce = aead::Nonce::assume_unique_for_key(nonce_bytes);
        let mut in_out = plaintext.as_bytes().to_vec();
        let tag_len = aead::AES_256_GCM.tag_len();
        in_out.resize(in_out.len() + tag_len, 0);

        key.seal_in_place_append_tag(nonce, aead::Aad::empty(), &mut in_out)
            .map_err(|_| anyhow!("Encryption failed"))?;

        let payload = format!(
            "enc:{}:{}:{}",
            general_purpose::STANDARD.encode(salt),
            general_purpose::STANDARD.encode(nonce_bytes),
            general_purpose::STANDARD.encode(in_out)
        );
        Ok(payload)
    }

    fn decrypt_symmetric(ciphertext: &str) -> Result<String> {
        let parts: Vec<&str> = ciphertext.split(':').collect();
        if parts.len() != 4 {
            return Err(anyhow!("Invalid encrypted payload"));
        }
        let salt = general_purpose::STANDARD
            .decode(parts[1])
            .map_err(|e| anyhow!("Decode salt: {}", e))?;
        let nonce_bytes = general_purpose::STANDARD
            .decode(parts[2])
            .map_err(|e| anyhow!("Decode nonce: {}", e))?;
        let mut data = general_purpose::STANDARD
            .decode(parts[3])
            .map_err(|e| anyhow!("Decode ciphertext: {}", e))?;

        let key = derive_key(&salt)?;
        let nonce = aead::Nonce::assume_unique_for_key(
            nonce_bytes
                .as_slice()
                .try_into()
                .map_err(|_| anyhow!("Invalid nonce length"))?,
        );

        let decrypted = key
            .open_in_place(nonce, aead::Aad::empty(), &mut data)
            .map_err(|_| anyhow!("Decryption failed"))?;
        let text = String::from_utf8(decrypted.to_vec())?;
        Ok(text)
    }
}

fn derive_key(salt: &[u8]) -> Result<aead::LessSafeKey> {
    let mut key_bytes = [0u8; 32];
    let iterations = NonZeroU32::new(PBKDF2_ITERATIONS).ok_or_else(|| anyhow!("Invalid iterations"))?;
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        iterations,
        salt,
        APP_SECRET,
        &mut key_bytes,
    );
    let unbound = aead::UnboundKey::new(&aead::AES_256_GCM, &key_bytes)
        .map_err(|_| anyhow!("Invalid key material"))?;
    Ok(aead::LessSafeKey::new(unbound))
}
