//! Kho lưu khoá service account, mã hoá AES-256-GCM.
//!
//! - Master key (32 byte ngẫu nhiên) nằm trong keychain của hệ điều hành
//!   (Credential Manager / Keychain / Secret Service). Không lưu toàn bộ JSON vào keychain vì
//!   Windows Credential Manager giới hạn ~2.5 KB mỗi mục, mà một khoá SA đã xấp xỉ mức đó.
//! - Nếu keychain không dùng được (ví dụ Linux không có Secret Service), master key rơi về
//!   file `master.key` (quyền 0600) và `weak = true` để giao diện cảnh báo người dùng:
//!   khi đó kho chỉ được bảo vệ bằng quyền truy cập thư mục dữ liệu của người dùng.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::auth::ServiceAccountKey;
use crate::error::{AppError, Result};

const MAGIC: &[u8] = b"SADV1";
const NONCE_LEN: usize = 12;
const KEYRING_SERVICE: &str = "dev.sadrive.app";
const KEYRING_USER: &str = "vault-master-key";
pub const VAULT_FILE: &str = "vault.bin";
const KEY_FILE: &str = "master.key";

/// Số màu có sẵn trong giao diện (xem `src/colors.ts`).
pub const PALETTE_SIZE: u8 = 8;

#[derive(Clone, Serialize, Deserialize)]
pub struct StoredAccount {
    pub id: String,
    pub label: String,
    pub color: u8,
    /// Email người dùng Workspace để giả danh (domain-wide delegation).
    #[serde(default)]
    pub impersonate: Option<String>,
    pub added_at: i64,
    pub key: ServiceAccountKey,
}

#[derive(Clone, Serialize, Deserialize, Default)]
struct VaultData {
    accounts: Vec<StoredAccount>,
}

pub struct Vault {
    path: PathBuf,
    key: [u8; 32],
    pub weak: bool,
    data: RwLock<VaultData>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Upsert {
    Added,
    Updated,
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Vault {
    pub fn open(dir: &Path) -> Result<Vault> {
        fs::create_dir_all(dir)?;
        let path = dir.join(VAULT_FILE);
        let (key, weak) = load_master_key(dir, path.exists())?;
        Self::with_key(path, key, weak)
    }

    pub fn with_key(path: PathBuf, key: [u8; 32], weak: bool) -> Result<Vault> {
        let data = if path.exists() {
            decrypt(&key, &fs::read(&path)?)?
        } else {
            VaultData::default()
        };
        Ok(Vault {
            path,
            key,
            weak,
            data: RwLock::new(data),
        })
    }

    /// Di chuyển kho hiện tại sang `.bak` để bắt đầu lại từ đầu (không xoá dữ liệu).
    pub fn reset(dir: &Path) -> Result<()> {
        let path = dir.join(VAULT_FILE);
        if path.exists() {
            let bak = dir.join(format!("{VAULT_FILE}.{}.bak", now_secs()));
            fs::rename(&path, bak)?;
        }
        Ok(())
    }

    pub fn list(&self) -> Vec<StoredAccount> {
        self.data.read().unwrap().accounts.clone()
    }

    pub fn get(&self, id: &str) -> Option<StoredAccount> {
        self.data.read().unwrap().accounts.iter().find(|a| a.id == id).cloned()
    }

    /// Thêm mới, hoặc cập nhật khoá nếu đã có account cùng `client_email` (xoay khoá).
    pub fn upsert(&self, key: ServiceAccountKey, label: Option<String>) -> Result<(StoredAccount, Upsert)> {
        let mut next = self.data.read().unwrap().clone();
        let label = label.map(|l| l.trim().to_string()).filter(|l| !l.is_empty());
        let result = if let Some(existing) = next.accounts.iter_mut().find(|a| a.key.client_email == key.client_email) {
            existing.key = key;
            if let Some(l) = label {
                existing.label = l;
            }
            (existing.clone(), Upsert::Updated)
        } else {
            let used: Vec<u8> = next.accounts.iter().map(|a| a.color).collect();
            let color = (0..PALETTE_SIZE).find(|c| !used.contains(c)).unwrap_or(next.accounts.len() as u8 % PALETTE_SIZE);
            let default_label = key.client_email.split('@').next().unwrap_or("service-account").to_string();
            let acc = StoredAccount {
                id: uuid::Uuid::new_v4().to_string(),
                label: label.unwrap_or(default_label),
                color,
                impersonate: None,
                added_at: now_secs(),
                key,
            };
            next.accounts.push(acc.clone());
            (acc, Upsert::Added)
        };
        self.commit(next)?;
        Ok(result)
    }

    pub fn update_meta(
        &self,
        id: &str,
        label: Option<String>,
        color: Option<u8>,
        impersonate: Option<Option<String>>,
    ) -> Result<StoredAccount> {
        let mut next = self.data.read().unwrap().clone();
        let acc = next
            .accounts
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| AppError::msg("Không tìm thấy account."))?;
        if let Some(l) = label {
            let l = l.trim();
            if l.is_empty() {
                return Err(AppError::msg("Tên account không được để trống."));
            }
            acc.label = l.to_string();
        }
        if let Some(c) = color {
            acc.color = c % PALETTE_SIZE;
        }
        if let Some(sub) = impersonate {
            acc.impersonate = sub.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        }
        let out = acc.clone();
        self.commit(next)?;
        Ok(out)
    }

    pub fn remove(&self, id: &str) -> Result<()> {
        let mut next = self.data.read().unwrap().clone();
        let before = next.accounts.len();
        next.accounts.retain(|a| a.id != id);
        if next.accounts.len() == before {
            return Err(AppError::msg("Không tìm thấy account."));
        }
        self.commit(next)
    }

    /// Ghi ra đĩa trước, chỉ cập nhật bộ nhớ khi ghi thành công.
    fn commit(&self, next: VaultData) -> Result<()> {
        let bytes = encrypt(&self.key, &next)?;
        let tmp = self.path.with_extension("tmp");
        {
            let mut f = create_private(&tmp)?;
            f.write_all(&bytes)?;
            f.sync_all()?;
        }
        fs::rename(&tmp, &self.path)?;
        *self.data.write().unwrap() = next;
        Ok(())
    }
}

fn encrypt(key: &[u8; 32], data: &VaultData) -> Result<Vec<u8>> {
    let plain = serde_json::to_vec(data)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ct = cipher
        .encrypt(&nonce, Payload { msg: &plain, aad: MAGIC })
        .map_err(|_| AppError::Vault("mã hoá thất bại".into()))?;
    let mut out = Vec::with_capacity(MAGIC.len() + NONCE_LEN + ct.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

fn decrypt(key: &[u8; 32], blob: &[u8]) -> Result<VaultData> {
    if blob.len() < MAGIC.len() + NONCE_LEN + 16 || &blob[..MAGIC.len()] != MAGIC {
        return Err(AppError::VaultCorrupt("file kho khoá có định dạng lạ hoặc bị cắt cụt".into()));
    }
    let nonce = Nonce::from_slice(&blob[MAGIC.len()..MAGIC.len() + NONCE_LEN]);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let plain = cipher
        .decrypt(nonce, Payload { msg: &blob[MAGIC.len() + NONCE_LEN..], aad: MAGIC })
        .map_err(|_| AppError::VaultCorrupt("sai master key hoặc dữ liệu bị hỏng".into()))?;
    serde_json::from_slice(&plain).map_err(|e| AppError::VaultCorrupt(format!("JSON bên trong hỏng: {e}")))
}

fn decode_key(b64: &str) -> Option<[u8; 32]> {
    let v = B64.decode(b64.trim()).ok()?;
    v.try_into().ok()
}

fn load_master_key(dir: &Path, vault_exists: bool) -> Result<([u8; 32], bool)> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER);
    let fallback = dir.join(KEY_FILE);

    // 1) keychain của hệ điều hành
    if let Ok(e) = &entry {
        if let Ok(s) = e.get_password() {
            if let Some(k) = decode_key(&s) {
                return Ok((k, false));
            }
        }
    }
    // 2) file dự phòng đã có từ trước
    if let Ok(s) = fs::read_to_string(&fallback) {
        if let Some(k) = decode_key(&s) {
            return Ok((k, true));
        }
    }
    // Kho đã tồn tại mà không tìm thấy khoá ở đâu: TUYỆT ĐỐI không tự sinh khoá mới
    // (sẽ khiến kho cũ vĩnh viễn không giải mã được). Báo lỗi để người dùng chọn.
    if vault_exists {
        return Err(AppError::VaultCorrupt(
            "không tìm thấy master key trong keychain hay file dự phòng".into(),
        ));
    }

    // 3) sinh khoá mới
    let key: [u8; 32] = Aes256Gcm::generate_key(OsRng).into();
    let b64 = B64.encode(key);
    if let Ok(e) = &entry {
        if e.set_password(&b64).is_ok() {
            // Đọc lại để chắc chắn đã lưu thật (một số backend "thành công" nhưng không lưu).
            let ok = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
                .and_then(|e2| e2.get_password())
                .map(|s| s == b64)
                .unwrap_or(false);
            if ok {
                return Ok((key, false));
            }
        }
    }
    write_private_file(&fallback, b64.as_bytes())?;
    Ok((key, true))
}

/// Tạo (hoặc cắt ngắn) một file chỉ chủ sở hữu đọc/ghi được: 0600 trên Unix.
/// Trên Windows quyền do ACL của thư mục dữ liệu người dùng quyết định.
fn create_private(path: &Path) -> std::io::Result<fs::File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)
    }
    #[cfg(not(unix))]
    {
        fs::File::create(path)
    }
}

fn write_private_file(path: &Path, data: &[u8]) -> std::io::Result<()> {
    create_private(path)?.write_all(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_key(email: &str) -> ServiceAccountKey {
        ServiceAccountKey {
            key_type: "service_account".into(),
            project_id: "p".into(),
            private_key_id: "kid".into(),
            private_key: "-----BEGIN PRIVATE KEY-----\nSECRET-MARKER\n-----END PRIVATE KEY-----\n".into(),
            client_email: email.into(),
            client_id: "1".into(),
        }
    }

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("sadrive-vault-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn roundtrip_and_no_plaintext_on_disk() {
        let dir = tmp();
        let path = dir.join(VAULT_FILE);
        let key = [7u8; 32];
        let v = Vault::with_key(path.clone(), key, false).unwrap();
        let (a, st) = v.upsert(fake_key("a@x.iam.gserviceaccount.com"), Some("Bot A".into())).unwrap();
        assert_eq!(st, Upsert::Added);
        assert_eq!(a.label, "Bot A");

        let raw = fs::read(&path).unwrap();
        let raw_str = String::from_utf8_lossy(&raw);
        assert!(!raw_str.contains("SECRET-MARKER"), "khoá không được nằm dạng rõ trên đĩa");
        assert!(!raw_str.contains("a@x.iam"));

        let v2 = Vault::with_key(path.clone(), key, false).unwrap();
        assert_eq!(v2.list().len(), 1);
        assert_eq!(v2.list()[0].key.private_key_id, "kid");

        // sai khoá -> lỗi VaultCorrupt chứ không panic / không trả rỗng
        assert!(matches!(
            Vault::with_key(path, [8u8; 32], false),
            Err(AppError::VaultCorrupt(_))
        ));
        fs::remove_dir_all(dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn vault_file_is_private_to_the_owner() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tmp();
        let path = dir.join(VAULT_FILE);
        let v = Vault::with_key(path.clone(), [3u8; 32], false).unwrap();
        v.upsert(fake_key("a@x.com"), None).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "vault.bin phải chỉ chủ sở hữu đọc/ghi được, đang là {mode:o}");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn same_email_updates_key_and_colors_are_distinct() {
        let dir = tmp();
        let v = Vault::with_key(dir.join(VAULT_FILE), [1u8; 32], false).unwrap();
        let (a, _) = v.upsert(fake_key("a@x.com"), None).unwrap();
        let (b, _) = v.upsert(fake_key("b@x.com"), None).unwrap();
        assert_ne!(a.color, b.color);
        assert_eq!(a.label, "a");

        let mut rotated = fake_key("a@x.com");
        rotated.private_key_id = "new".into();
        let (a2, st) = v.upsert(rotated, None).unwrap();
        assert_eq!(st, Upsert::Updated);
        assert_eq!(a2.id, a.id);
        assert_eq!(v.list().len(), 2);
        assert_eq!(v.get(&a.id).unwrap().key.private_key_id, "new");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn update_remove_and_impersonate() {
        let dir = tmp();
        let v = Vault::with_key(dir.join(VAULT_FILE), [2u8; 32], false).unwrap();
        let (a, _) = v.upsert(fake_key("a@x.com"), None).unwrap();
        let u = v.update_meta(&a.id, Some(" Mới ".into()), Some(3), Some(Some(" boss@corp.com ".into()))).unwrap();
        assert_eq!(u.label, "Mới");
        assert_eq!(u.color, 3);
        assert_eq!(u.impersonate.as_deref(), Some("boss@corp.com"));
        let u = v.update_meta(&a.id, None, None, Some(None)).unwrap();
        assert!(u.impersonate.is_none());
        assert!(v.update_meta(&a.id, Some("  ".into()), None, None).is_err());
        v.remove(&a.id).unwrap();
        assert!(v.list().is_empty());
        assert!(v.remove(&a.id).is_err());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn truncated_file_is_reported_not_overwritten() {
        let dir = tmp();
        let path = dir.join(VAULT_FILE);
        fs::write(&path, b"SADV1xx").unwrap();
        assert!(matches!(Vault::with_key(path.clone(), [1u8; 32], false), Err(AppError::VaultCorrupt(_))));
        assert_eq!(fs::read(&path).unwrap(), b"SADV1xx", "không được ghi đè file hỏng");
        Vault::reset(&dir).unwrap();
        assert!(!path.exists());
        assert!(fs::read_dir(&dir).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().ends_with(".bak")));
        fs::remove_dir_all(dir).ok();
    }
}
