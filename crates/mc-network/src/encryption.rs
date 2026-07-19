//! 加密层 — 在线模式 AES-CFB8 流加密
//!
//! Minecraft 在线模式登录流程:
//!   1. Server 生成 RSA-1024 密钥对 + 4 字节随机 verify token
//!   2. Server → Client: EncryptionRequest (public_key + verify_token)
//!   3. Client 生成 16 字节 shared secret
//!   4. Client → Server: EncryptionResponse (encrypted shared_secret + verify_token)
//!   5. Server 解密并验证 token, 启用 AES-CFB8 加密
//!   6. Server → Mojang: hasJoined 验证
//!   7. Server → Client: LoginSuccess (encrypted)

use aes::cipher::{BlockEncrypt, KeyInit};
use aes::Aes128;
use rsa::pkcs8::EncodePublicKey;
use rsa::{Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey};
use sha1::{Digest, Sha1};

/// 加密状态
pub enum EncryptionState {
    /// 未加密 — 离线模式或握手阶段
    None,
    /// 在线模式加密已启用 (AES-CFB8 已激活)
    Enabled,
}

/// AES-CFB8 流密码 — 8-bit 反馈模式
///
/// Minecraft 将 16-byte shared secret 同时作为 AES 密钥和 CFB8 IV。
/// CFB8 的 encrypt 和 decrypt 使用相同操作。
pub struct AesCfb8 {
    aes: Aes128,
    state: [u8; 16],
}

impl AesCfb8 {
    pub fn new(key: &[u8; 16]) -> Self {
        let aes = Aes128::new_from_slice(key).expect("AES-128 requires 16-byte key");
        Self { aes, state: *key }
    }

    /// CFB8 encrypt: plaintext → ciphertext, feedback = ciphertext
    pub fn encrypt(&mut self, data: &mut [u8]) {
        let mut buf: [u8; 16] = self.state;
        for byte in data.iter_mut() {
            let mut block = aes::Block::from(buf);
            self.aes.encrypt_block(&mut block);
            let keystream = block[0];
            *byte ^= keystream; // plaintext → ciphertext
            buf.copy_within(1..16, 0);
            buf[15] = *byte; // feedback = ciphertext (output)
        }
        self.state = buf;
    }

    /// CFB8 decrypt: ciphertext → plaintext, feedback = ciphertext
    pub fn decrypt(&mut self, data: &mut [u8]) {
        let mut buf: [u8; 16] = self.state;
        for byte in data.iter_mut() {
            let mut block = aes::Block::from(buf);
            self.aes.encrypt_block(&mut block);
            let keystream = block[0];
            let ciphertext = *byte; // save ciphertext for feedback
            *byte ^= keystream; // ciphertext → plaintext
            buf.copy_within(1..16, 0);
            buf[15] = ciphertext; // feedback = ciphertext (input)
        }
        self.state = buf;
    }

    /// Encrypt and then decrypt (for tests)
    pub fn process(&mut self, data: &mut [u8]) {
        self.encrypt(data);
    }
}

/// 为在线模式登录生成的密钥材料
pub struct EncryptionKeys {
    pub private_key: RsaPrivateKey,
    pub public_key_der: Vec<u8>,
    pub verify_token: [u8; 4],
}

/// 生成 RSA-1024 密钥对和验证令牌
pub fn generate_keys() -> Result<EncryptionKeys, String> {
    let mut rng = rand::rngs::OsRng;
    let private_key = RsaPrivateKey::new(&mut rng, 1024)
        .map_err(|e| format!("Failed to generate RSA key: {}", e))?;
    let public_key = RsaPublicKey::from(&private_key);
    let public_key_der = public_key
        .to_public_key_der()
        .map_err(|e| format!("Failed to encode public key: {}", e))?
        .as_bytes()
        .to_vec();

    let verify_token: [u8; 4] = rand::random();

    Ok(EncryptionKeys {
        private_key,
        public_key_der,
        verify_token,
    })
}

/// 解密客户端发来的 shared secret 和 verify token
///
/// 使用 RSA 私钥解密 PKCS#1 v1.5 密文。
pub fn decrypt_client_secrets(
    private_key: &RsaPrivateKey,
    encrypted_secret: &[u8],
    encrypted_token: &[u8],
) -> Result<([u8; 16], [u8; 4]), String> {
    let shared_secret: [u8; 16] = private_key
        .decrypt(Pkcs1v15Encrypt, encrypted_secret)
        .map_err(|e| format!("Failed to decrypt shared secret: {}", e))?
        .try_into()
        .map_err(|_| "Shared secret must be 16 bytes".to_string())?;

    let token: [u8; 4] = private_key
        .decrypt(Pkcs1v15Encrypt, encrypted_token)
        .map_err(|e| format!("Failed to decrypt verify token: {}", e))?
        .try_into()
        .map_err(|_| "Verify token must be 4 bytes".to_string())?;

    Ok((shared_secret, token))
}

/// 计算 Mojang session server 的 serverId hash
///
/// hash = SHA1(server_id + shared_secret + encoded_public_key)
pub fn compute_server_hash(server_id: &str, shared_secret: &[u8; 16], public_key: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(server_id.as_bytes());
    hasher.update(shared_secret);
    hasher.update(public_key);
    let result = hasher.finalize();
    format!("{:x}", result)
}

/// 验证 Mojang 会话 (请求 hasJoined API)
///
/// POST https://sessionserver.mojang.com/session/minecraft/hasJoined
pub async fn verify_mojang_session(
    username: &str,
    server_hash: &str,
) -> Result<MojangProfile, String> {
    let url = "https://sessionserver.mojang.com/session/minecraft/hasJoined";
    let body = serde_json::json!({
        "username": username,
        "serverId": server_hash,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Mojang API request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Mojang API returned {}", resp.status()));
    }

    let profile: MojangProfile = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Mojang response: {}", e))?;

    Ok(profile)
}

/// Mojang 返回的玩家档案
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MojangProfile {
    pub id: String,       // UUID with dashes
    pub name: String,     // username
    #[serde(default)]
    pub properties: Vec<MojangProperty>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MojangProperty {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub signature: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_keys() {
        let keys = generate_keys().expect("generate keys");
        assert_eq!(keys.verify_token.len(), 4);
        assert!(!keys.public_key_der.is_empty());
    }

    #[test]
    fn test_aes_encrypt_block_works() {
        use aes::cipher::{BlockEncrypt, KeyInit};
        let key = [0u8; 16];
        let aes = Aes128::new_from_slice(&key).unwrap();
        let mut block = aes::cipher::generic_array::GenericArray::from([1u8; 16]);
        let original = block;
        aes.encrypt_block(&mut block);
        assert_ne!(block, original, "AES encrypt_block should change block content");
    }

    #[test]
    fn test_aes_cfb8_single_byte() {
        let key = [1u8; 16];
        let mut data = [0x48u8];
        let mut enc = AesCfb8::new(&key);
        enc.encrypt(&mut data);
        let mut dec = AesCfb8::new(&key);
        dec.decrypt(&mut data);
        assert_eq!(data[0], 0x48, "Single byte should roundtrip");
    }

    #[test]
    fn test_aes_cfb8_two_bytes() {
        let key = [0xAAu8; 16];
        let mut data = [72u8, 101u8]; // "He"
        let mut enc = AesCfb8::new(&key);
        enc.encrypt(&mut data);
        let ct = data;
        let mut dec = AesCfb8::new(&key);
        let mut data2 = ct;
        dec.decrypt(&mut data2);
        assert_eq!(data2, [72, 101], "Two bytes should roundtrip");
    }

    #[test]
    fn test_cfb8_deterministic() {
        // Verify CFB8 produces deterministic output
        let key = [0x10u8; 16];
        let mut c1 = AesCfb8::new(&key);
        let mut d1 = b"Test".to_vec();
        c1.encrypt(&mut d1);
        let mut c2 = AesCfb8::new(&key);
        let mut d2 = b"Test".to_vec();
        c2.encrypt(&mut d2);
        assert_eq!(d1, d2, "Same input + key should produce same ciphertext");
    }

    #[test]
    fn test_aes_cfb8_roundtrip() {
        let key: [u8; 16] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                              0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10];
        let mut enc = AesCfb8::new(&key);
        let mut data = b"Hello, Minecraft!".to_vec();
        enc.encrypt(&mut data);
        let mut dec = AesCfb8::new(&key);
        dec.decrypt(&mut data);
        assert_eq!(&data, b"Hello, Minecraft!");
    }

    #[test]
    fn test_compute_server_hash() {
        let shared_secret: [u8; 16] = [0; 16];
        let public_key: &[u8] = b"test_key";
        let hash = compute_server_hash("", &shared_secret, public_key);
        assert!(!hash.is_empty());
    }

    #[test]
    fn test_encrypt_decrypt_secrets() {
        let keys = generate_keys().unwrap();
        let shared_secret: [u8; 16] = [0x42; 16];
        let verify_token: [u8; 4] = [0xAB; 4];

        let public_key = RsaPublicKey::from(&keys.private_key);
        let enc_secret = public_key.encrypt(&mut rand::rngs::OsRng, Pkcs1v15Encrypt, &shared_secret).unwrap();
        let enc_token = public_key.encrypt(&mut rand::rngs::OsRng, Pkcs1v15Encrypt, &verify_token).unwrap();

        let (dec_secret, dec_token) = decrypt_client_secrets(&keys.private_key, &enc_secret, &enc_token).unwrap();
        assert_eq!(dec_secret, shared_secret);
        assert_eq!(dec_token, verify_token);
    }
}
