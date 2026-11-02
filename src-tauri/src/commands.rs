//! Các lệnh Tauri mà giao diện gọi qua `invoke`.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::auth::ServiceAccountKey;
use crate::drive::{is_plausible_email, About, DriveFile, ListPage, ListParams, Permission, SharedDrive};
use crate::error::{AppError, Result};
use crate::state::AppState;
use crate::transfer::{DownloadItem, TransferInfo};
use crate::vault::{StoredAccount, Upsert, Vault};

type St<'a> = State<'a, AppState>;

// ───────────── accounts ─────────────

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub id: String,
    pub label: String,
    pub client_email: String,
    pub project_id: String,
    pub private_key_id: String,
    pub color: u8,
    pub impersonate: Option<String>,
    /// Mili giây tính từ epoch, để giao diện đưa thẳng vào `new Date(...)`. Kho lưu theo giây.
    pub added_at: i64,
}

impl From<&StoredAccount> for AccountInfo {
    fn from(a: &StoredAccount) -> Self {
        Self {
            id: a.id.clone(),
            label: a.label.clone(),
            client_email: a.key.client_email.clone(),
            project_id: a.key.project_id.clone(),
            private_key_id: a.key.private_key_id.clone(),
            color: a.color,
            impersonate: a.impersonate.clone(),
            added_at: a.added_at.saturating_mul(1000),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsState {
    pub accounts: Vec<AccountInfo>,
    /// `true` khi master key phải lưu trong file vì keychain hệ điều hành không dùng được.
    pub weak_key_storage: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddOutcome {
    pub account: AccountInfo,
    /// "added" hoặc "updated" (cùng client_email: khoá được thay bằng khoá mới).
    pub status: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOutcome {
    /// Nơi người dùng đã chọn lưu file khoá.
    pub path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub path: String,
    pub ok: bool,
    pub outcome: Option<AddOutcome>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn accounts_list(state: St<'_>) -> Result<AccountsState> {
    let vault = state.vault().await?;
    Ok(AccountsState {
        accounts: vault.list().iter().map(AccountInfo::from).collect(),
        weak_key_storage: vault.weak,
    })
}

async fn add_key(state: &AppState, key: ServiceAccountKey, label: Option<String>) -> Result<AddOutcome> {
    let vault = state.vault().await?;
    let (acc, st) = vault.upsert(key, label)?;
    if st == Upsert::Updated {
        state.drop_session(&acc.id);
    }
    Ok(AddOutcome {
        account: AccountInfo::from(&acc),
        status: if st == Upsert::Added { "added" } else { "updated" },
    })
}

#[tauri::command]
pub async fn account_add(state: St<'_>, json: String, label: Option<String>) -> Result<AddOutcome> {
    let key = ServiceAccountKey::parse(&json)?;
    add_key(&state, key, label).await
}

#[tauri::command]
pub async fn account_add_from_files(state: St<'_>, paths: Vec<String>) -> Result<Vec<ImportResult>> {
    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        let res: Result<AddOutcome> = async {
            let meta = tokio::fs::metadata(&path).await?;
            if meta.len() > 64 * 1024 {
                return Err(AppError::msg("File quá lớn (> 64 KB) nên không phải khoá service account."));
            }
            let bytes = tokio::fs::read(&path).await?;
            let text = String::from_utf8(bytes).map_err(|_| AppError::msg("File không phải văn bản UTF-8, không phải khoá JSON."))?;
            let key = ServiceAccountKey::parse(&text)?;
            add_key(&state, key, None).await
        }
        .await;
        out.push(match res {
            Ok(o) => ImportResult { path, ok: true, outcome: Some(o), error: None },
            Err(e) => ImportResult { path, ok: false, outcome: None, error: Some(e.display_full()) },
        });
    }
    Ok(out)
}

#[tauri::command]
pub async fn account_update(
    state: St<'_>,
    id: String,
    label: Option<String>,
    color: Option<u8>,
    impersonate: Option<String>,
) -> Result<AccountInfo> {
    let impersonate = match impersonate {
        None => None,
        Some(s) if s.trim().is_empty() => Some(None),
        Some(s) => {
            if !is_plausible_email(s.trim()) {
                return Err(AppError::msg("Email giả danh không hợp lệ."));
            }
            Some(Some(s))
        }
    };
    let vault = state.vault().await?;
    let acc = vault.update_meta(&id, label, color, impersonate)?;
    state.drop_session(&id); // subject có thể đã đổi -> bỏ token cũ
    Ok(AccountInfo::from(&acc))
}

#[tauri::command]
pub async fn account_remove(state: St<'_>, id: String) -> Result<()> {
    let vault = state.vault().await?;
    vault.remove(&id)?;
    state.drop_session(&id);
    Ok(())
}

/// Xuất khoá của account ra file JSON (định dạng chuẩn của Google, nhập lại được vào ứng dụng này).
///
/// Hộp thoại lưu do phía Rust mở và đường dẫn không đi qua giao diện: giao diện chỉ nói "xuất account
/// này", còn việc ghi file luôn cần người dùng chọn chỗ lưu trên hộp thoại của hệ điều hành. Khoá cũng
/// không bao giờ được trả về cho JS. Trả `None` nếu người dùng đóng hộp thoại.
#[tauri::command]
pub async fn account_export_key(
    app: tauri::AppHandle,
    window: WebviewWindow,
    state: St<'_>,
    id: String,
) -> Result<Option<ExportOutcome>> {
    let vault = state.vault().await?;
    let acc = vault.get(&id).ok_or_else(|| AppError::msg("Không tìm thấy account (có thể đã bị xoá)."))?;
    // Dựng nội dung trước khi hỏi chỗ lưu để lỗi (nếu có) lộ ra sớm, không bắt người dùng chọn rồi mới báo.
    let json = acc.key.to_key_file()?;

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .set_title("Lưu file khoá service account")
        .set_file_name(acc.key.suggested_file_name())
        .add_filter("Khoá JSON", &["json"])
        .save_file(move |picked| {
            let _ = tx.send(picked);
        });
    let Some(picked) = rx.await.map_err(|_| AppError::msg("Hộp thoại lưu bị đóng đột ngột."))? else {
        return Ok(None);
    };
    let path = picked
        .into_path()
        .map_err(|e| AppError::msg(format!("Đường dẫn đã chọn không dùng được: {e}")))?;

    let shown = path.display().to_string();
    tokio::task::spawn_blocking(move || crate::fsutil::write_private_atomic(&path, json.as_bytes()))
        .await
        .map_err(|e| AppError::msg(format!("Tác vụ ghi file bị gián đoạn: {e}")))??;
    Ok(Some(ExportOutcome { path: shown }))
}

/// Chuyển kho hiện tại sang file `.bak` để bắt đầu lại (chỉ dùng khi kho không giải mã được).
#[tauri::command]
pub async fn vault_reset(state: St<'_>) -> Result<()> {
    if state.vault_is_open() {
        return Err(AppError::msg("Kho khoá đang mở bình thường, không cần đặt lại."));
    }
    let dir = state.data_dir.clone();
    tokio::task::spawn_blocking(move || Vault::reset(&dir))
        .await
        .map_err(|e| AppError::Vault(e.to_string()))?
}

// ───────────── Drive ─────────────

#[tauri::command]
pub async fn drive_about(state: St<'_>, account_id: String) -> Result<About> {
    let ts = state.token_source(&account_id).await?;
    state.api.about(&ts).await
}

#[tauri::command]
pub async fn drive_shared_drives(state: St<'_>, account_id: String) -> Result<Vec<SharedDrive>> {
    let ts = state.token_source(&account_id).await?;
    state.api.shared_drives(&ts).await
}

#[tauri::command]
pub async fn drive_list(state: St<'_>, account_id: String, params: ListParams) -> Result<ListPage> {
    let ts = state.token_source(&account_id).await?;
    state.api.list(&ts, &params).await
}

#[tauri::command]
pub async fn drive_create_folder(state: St<'_>, account_id: String, name: String, parent_id: String) -> Result<DriveFile> {
    let ts = state.token_source(&account_id).await?;
    state.api.create_folder(&ts, &name, &parent_id).await
}

#[tauri::command]
pub async fn drive_rename(state: St<'_>, account_id: String, file_id: String, name: String) -> Result<DriveFile> {
    let ts = state.token_source(&account_id).await?;
    state.api.rename(&ts, &file_id, &name).await
}

#[tauri::command]
pub async fn drive_move(
    state: St<'_>,
    account_id: String,
    file_id: String,
    new_parent_id: String,
    old_parent_ids: Vec<String>,
) -> Result<DriveFile> {
    let ts = state.token_source(&account_id).await?;
    state.api.move_file(&ts, &file_id, &new_parent_id, &old_parent_ids).await
}

#[tauri::command]
pub async fn drive_trash(state: St<'_>, account_id: String, file_id: String) -> Result<()> {
    let ts = state.token_source(&account_id).await?;
    state.api.set_trashed(&ts, &file_id, true).await
}

#[tauri::command]
pub async fn drive_restore(state: St<'_>, account_id: String, file_id: String) -> Result<()> {
    let ts = state.token_source(&account_id).await?;
    state.api.set_trashed(&ts, &file_id, false).await
}

#[tauri::command]
pub async fn drive_delete(state: St<'_>, account_id: String, file_id: String) -> Result<()> {
    let ts = state.token_source(&account_id).await?;
    state.api.delete(&ts, &file_id).await
}

#[tauri::command]
pub async fn drive_empty_trash(state: St<'_>, account_id: String) -> Result<()> {
    let ts = state.token_source(&account_id).await?;
    state.api.empty_trash(&ts).await
}

#[tauri::command]
pub async fn drive_permissions(state: St<'_>, account_id: String, file_id: String) -> Result<Vec<Permission>> {
    let ts = state.token_source(&account_id).await?;
    state.api.permissions(&ts, &file_id).await
}

#[tauri::command]
pub async fn drive_share_user(
    state: St<'_>,
    account_id: String,
    file_id: String,
    email: String,
    role: String,
    notify: bool,
) -> Result<Permission> {
    let ts = state.token_source(&account_id).await?;
    state.api.share_with_user(&ts, &file_id, &email, &role, notify).await
}

#[tauri::command]
pub async fn drive_share_anyone(state: St<'_>, account_id: String, file_id: String) -> Result<Permission> {
    let ts = state.token_source(&account_id).await?;
    state.api.share_anyone(&ts, &file_id).await
}

#[tauri::command]
pub async fn drive_unshare(state: St<'_>, account_id: String, file_id: String, permission_id: String) -> Result<()> {
    let ts = state.token_source(&account_id).await?;
    state.api.unshare(&ts, &file_id, &permission_id).await
}

// ───────────── truyền tải ─────────────

#[tauri::command]
pub async fn transfer_upload(state: St<'_>, account_id: String, parent_id: String, paths: Vec<String>) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    crate::drive::check_id(&parent_id)?;
    let ctx = state.job_ctx(&account_id).await?;
    let mgr = state.transfers.clone();
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    // Việc tạo cây thư mục có thể lâu: chạy nền và trả về ngay, tiến độ đến qua sự kiện.
    tokio::spawn(async move { mgr.plan_upload(ctx, account_id, parent_id, paths).await });
    Ok(())
}

#[tauri::command]
pub async fn transfer_download(state: St<'_>, account_id: String, items: Vec<DownloadItem>, dest_dir: String) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    let dest = PathBuf::from(dest_dir);
    if !dest.is_dir() {
        return Err(AppError::msg("Thư mục đích không tồn tại."));
    }
    let ctx = state.job_ctx(&account_id).await?;
    let mgr = state.transfers.clone();
    tokio::spawn(async move { mgr.plan_download(ctx, account_id, items, dest).await });
    Ok(())
}

#[tauri::command]
pub fn transfer_list(state: St<'_>) -> Vec<TransferInfo> {
    state.transfers.list()
}

#[tauri::command]
pub fn transfer_cancel(state: St<'_>, id: String) {
    state.transfers.cancel(&id);
}

#[tauri::command]
pub fn transfer_cancel_all(state: St<'_>) {
    state.transfers.cancel_all();
}

#[tauri::command]
pub async fn transfer_retry(state: St<'_>, id: String) -> Result<()> {
    state.transfers.retry(&id)
}

#[tauri::command]
pub fn transfer_clear_finished(state: St<'_>) -> Vec<TransferInfo> {
    state.transfers.clear_finished()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_info_reports_added_at_in_milliseconds_for_js() {
        // Kho lưu theo giây, giao diện dùng `new Date(addedAt)` (mili giây). Lệch đơn vị từng làm hộp
        // thoại Cài đặt hiện "22/01/1970", và mock của giao diện không phát hiện được vì dùng mili giây.
        let acc = StoredAccount {
            id: "id".into(),
            label: "l".into(),
            color: 0,
            impersonate: None,
            added_at: 1_790_000_000,
            key: ServiceAccountKey {
                key_type: "service_account".into(),
                project_id: "p".into(),
                private_key_id: "k".into(),
                private_key: "x".into(),
                client_email: "a@p.iam.gserviceaccount.com".into(),
                client_id: String::new(),
            },
        };
        let info = AccountInfo::from(&acc);
        assert_eq!(info.added_at, 1_790_000_000_000);
        // Chạy thử với giá trị lớn bất thường: không tràn số.
        let mut huge = acc.clone();
        huge.added_at = i64::MAX;
        assert_eq!(AccountInfo::from(&huge).added_at, i64::MAX);
    }
}
