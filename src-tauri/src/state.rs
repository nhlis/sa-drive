//! Trạng thái dùng chung của ứng dụng (được Tauri quản lý).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager};
use tokio::sync::OnceCell;

use crate::auth::TokenSource;
use crate::drive::DriveApi;
use crate::error::{AppError, Result};
use crate::transfer::{JobCtx, TauriSink, TransferManager};
use crate::vault::Vault;

pub struct AppState {
    pub data_dir: PathBuf,
    vault: OnceCell<Arc<Vault>>,
    sessions: Mutex<HashMap<String, Arc<TokenSource>>>,
    pub api: Arc<DriveApi>,
    pub transfers: Arc<TransferManager>,
}

impl AppState {
    pub fn new(app: &AppHandle) -> Result<Self> {
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| AppError::msg(format!("Không xác định được thư mục dữ liệu của ứng dụng: {e}")))?;
        Ok(Self {
            data_dir,
            vault: OnceCell::new(),
            sessions: Mutex::new(HashMap::new()),
            api: Arc::new(DriveApi::new()?),
            transfers: TransferManager::new(Arc::new(TauriSink(app.clone()))),
        })
    }

    /// Mở kho khoá ở lần gọi đầu tiên. Việc này có thể chạm vào keychain của hệ điều hành
    /// (có thể hiện hộp thoại mở khoá) nên chạy ở luồng nền thay vì chặn lúc khởi động.
    pub async fn vault(&self) -> Result<Arc<Vault>> {
        self.vault
            .get_or_try_init(|| async {
                let dir = self.data_dir.clone();
                let opened = tokio::task::spawn_blocking(move || Vault::open(&dir))
                    .await
                    .map_err(|e| AppError::Vault(format!("tác vụ nền bị gián đoạn: {e}")))?;
                opened.map(Arc::new)
            })
            .await
            .cloned()
    }

    pub fn vault_is_open(&self) -> bool {
        self.vault.initialized()
    }

    pub async fn token_source(&self, account_id: &str) -> Result<Arc<TokenSource>> {
        if let Some(ts) = self.sessions.lock().unwrap().get(account_id) {
            return Ok(ts.clone());
        }
        let vault = self.vault().await?;
        let acc = vault
            .get(account_id)
            .ok_or_else(|| AppError::msg("Account không tồn tại (có thể đã bị xoá)."))?;
        let ts = Arc::new(TokenSource::new(acc.key, acc.impersonate, self.api.token_client()));
        self.sessions.lock().unwrap().insert(account_id.to_string(), ts.clone());
        Ok(ts)
    }

    pub async fn job_ctx(&self, account_id: &str) -> Result<Arc<JobCtx>> {
        Ok(Arc::new(JobCtx {
            api: self.api.clone(),
            ts: self.token_source(account_id).await?,
        }))
    }

    /// Bỏ token đang cache (khi đổi khoá / email giả danh / xoá account).
    pub fn drop_session(&self, account_id: &str) {
        self.sessions.lock().unwrap().remove(account_id);
    }
}
