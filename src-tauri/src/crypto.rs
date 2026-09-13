//! API key 的本地加密存储。
//!
//! 方案：密钥 = SHA256(固定盐 + 本机 IOPlatformUUID)，用 AES-256-GCM 加密，
//! 库里存 `enc:v1:<base64(nonce || ciphertext)>`。
//!
//! 为什么不用 Keychain：dev 模式跑的是裸 binary，每次编译签名都变，
//! Keychain 的 ACL 会失效并反复弹授权框，用起来很烦。机器绑定密钥免打扰，
//! 而且 db 文件被拷到别的机器上照样解不开。
//!
//! 定位要诚实：这是**混淆级**保护——挡的是"db 文件被顺手拷走 / 进了备份 / 被人瞄一眼"，
//! 挡不住同机上刻意提权读取的进程（机器 UUID 本来就是可读的）。
//! 要更强就得引入用户口令或 Keychain，那是另一个取舍，当前不做。

use crate::error::{AppError, AppResult};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::Aes256Gcm;
use base64::Engine;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

const PREFIX: &str = "enc:v1:";
const SALT: &[u8] = b"xiic-image-studio::api-key::v1";
const NONCE_LEN: usize = 12;

/// 本机唯一标识（只取一次，进程内缓存）
fn machine_id() -> &'static str {
    static CACHE: OnceLock<String> = OnceLock::new();
    CACHE.get_or_init(|| {
        // macOS：IOPlatformUUID 是稳定的硬件 UUID
        if let Ok(out) = std::process::Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
        {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(line) = text.lines().find(|l| l.contains("IOPlatformUUID")) {
                if let Some(v) = line.split('=').nth(1) {
                    let v = v.trim().trim_matches('"');
                    if !v.is_empty() {
                        return v.to_string();
                    }
                }
            }
        }
        // 兜底：拿不到就退化成 hostname（安全性下降，但至少不是明文）
        let host = std::process::Command::new("hostname")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        format!("fallback:{host}")
    })
}

fn master_key() -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(SALT);
    h.update(machine_id().as_bytes());
    h.finalize().into()
}

/// 12 字节随机 nonce（借 uuid v4 的 getrandom，省一个依赖）
fn random_nonce() -> [u8; NONCE_LEN] {
    let b = uuid::Uuid::new_v4();
    let s = b.as_bytes();
    let mut n = [0u8; NONCE_LEN];
    n.copy_from_slice(&s[..NONCE_LEN]);
    n
}

/// 加密。空串与已是密文的值原样返回（幂等）。
pub fn encrypt(plain: &str) -> AppResult<String> {
    if plain.is_empty() || plain.starts_with(PREFIX) {
        return Ok(plain.to_string());
    }
    let cipher = Aes256Gcm::new_from_slice(&master_key())
        .map_err(|e| AppError::General(format!("加密初始化失败: {e}")))?;
    let nonce_bytes = random_nonce();
    let ct = cipher
        .encrypt(aes_gcm::Nonce::from_slice(&nonce_bytes), plain.as_bytes())
        .map_err(|e| AppError::General(format!("加密失败: {e}")))?;
    let mut raw = nonce_bytes.to_vec();
    raw.extend_from_slice(&ct);
    Ok(format!(
        "{PREFIX}{}",
        base64::engine::general_purpose::STANDARD.encode(raw)
    ))
}

/// 解密。非密文（历史明文数据）原样返回，保证旧库照常能跑。
pub fn decrypt(stored: &str) -> AppResult<String> {
    let Some(b64) = stored.strip_prefix(PREFIX) else {
        return Ok(stored.to_string());
    };
    let raw = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| AppError::General(format!("密钥解密失败（base64 异常）: {e}")))?;
    if raw.len() < NONCE_LEN + 1 {
        return Err(AppError::General("密钥数据损坏（长度不足）".into()));
    }
    let (nonce_bytes, ct) = raw.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new_from_slice(&master_key())
        .map_err(|e| AppError::General(format!("解密初始化失败: {e}")))?;
    let plain = cipher
        .decrypt(aes_gcm::Nonce::from_slice(nonce_bytes), ct)
        .map_err(|_| AppError::General("密钥解密失败（换了机器，或数据被篡改）".into()))?;
    String::from_utf8(plain).map_err(|e| AppError::General(format!("密钥解码失败: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let plain = "sk-test-1234567890";
        let enc = encrypt(plain).unwrap();
        assert!(enc.starts_with(PREFIX));
        assert_ne!(enc, plain);
        assert_eq!(decrypt(&enc).unwrap(), plain);
    }

    #[test]
    fn idempotent_and_legacy() {
        // 已加密的不会被二次加密
        let enc = encrypt("abc").unwrap();
        assert_eq!(encrypt(&enc).unwrap(), enc);
        // 空串保持空串
        assert_eq!(encrypt("").unwrap(), "");
        // 旧明文数据原样可读
        assert_eq!(decrypt("plain-legacy-key").unwrap(), "plain-legacy-key");
    }

    #[test]
    fn tampered_data_fails() {
        let enc = encrypt("abc").unwrap();
        let mut broken = enc.clone();
        broken.push('x');
        assert!(decrypt(&broken).is_err());
    }
}
