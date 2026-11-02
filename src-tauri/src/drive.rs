//! Client cho Google Drive API v3 (chỉ những phần ứng dụng cần).
//! Tên field trong `fields=` đã được đối chiếu với tài liệu chính thức của Drive API v3.

use std::time::Duration;

use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;

use crate::auth::TokenSource;
use crate::error::{AppError, Result};

pub const API_BASE: &str = "https://www.googleapis.com";
pub const FOLDER_MIME: &str = "application/vnd.google-apps.folder";
pub const SHORTCUT_MIME: &str = "application/vnd.google-apps.shortcut";

pub const FILE_FIELDS: &str = "id,name,mimeType,size,modifiedTime,createdTime,parents,\
owners(displayName,emailAddress,me),ownedByMe,shared,trashed,webViewLink,driveId,md5Checksum,\
capabilities(canDelete,canTrash,canUntrash,canRename,canEdit,canDownload,canAddChildren,canShare,canMoveItemWithinDrive),\
shortcutDetails(targetId,targetMimeType)";

const PERMISSION_FIELDS: &str =
    "id,type,role,emailAddress,displayName,domain,deleted,permissionDetails(inherited,permissionType,role)";

const MAX_RETRIES: u32 = 5;
pub(crate) const API_TIMEOUT: Duration = Duration::from_secs(60);

// ───────────────────────── kiểu dữ liệu ─────────────────────────

fn de_opt_u64<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Option<u64>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum N {
        S(String),
        U(u64),
        F(f64),
    }
    match Option::<N>::deserialize(d)? {
        None => Ok(None),
        Some(N::S(s)) => s.trim().parse().map(Some).map_err(serde::de::Error::custom),
        Some(N::U(u)) => Ok(Some(u)),
        Some(N::F(f)) => Ok(Some(f as u64)),
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Owner {
    pub display_name: Option<String>,
    pub email_address: Option<String>,
    pub me: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub can_delete: Option<bool>,
    pub can_trash: Option<bool>,
    pub can_untrash: Option<bool>,
    pub can_rename: Option<bool>,
    pub can_edit: Option<bool>,
    pub can_download: Option<bool>,
    pub can_add_children: Option<bool>,
    pub can_share: Option<bool>,
    pub can_move_item_within_drive: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutDetails {
    pub target_id: Option<String>,
    pub target_mime_type: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct DriveFile {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub size: Option<u64>,
    pub modified_time: Option<String>,
    pub created_time: Option<String>,
    #[serde(default)]
    pub parents: Vec<String>,
    #[serde(default)]
    pub owners: Vec<Owner>,
    pub owned_by_me: Option<bool>,
    #[serde(default)]
    pub shared: bool,
    #[serde(default)]
    pub trashed: bool,
    pub web_view_link: Option<String>,
    pub drive_id: Option<String>,
    pub md5_checksum: Option<String>,
    pub capabilities: Option<Capabilities>,
    pub shortcut_details: Option<ShortcutDetails>,
}

impl DriveFile {
    pub fn is_folder(&self) -> bool {
        self.mime_type == FOLDER_MIME
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListPage {
    #[serde(default)]
    pub files: Vec<DriveFile>,
    pub next_page_token: Option<String>,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ListMode {
    /// Nội dung một thư mục (mặc định: gốc My Drive).
    Children,
    /// Mọi mục do chính account này sở hữu (`'me' in owners`).
    Owned,
    /// Được người khác chia sẻ trực tiếp cho account.
    Shared,
    /// Thùng rác.
    Trash,
    /// Tìm theo tên.
    Search,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ListParams {
    pub mode: ListMode,
    pub parent_id: Option<String>,
    pub drive_id: Option<String>,
    pub query: Option<String>,
    pub folders_only: Option<bool>,
    pub page_token: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SharedDrive {
    pub id: String,
    pub name: String,
    pub color_rgb: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AboutUser {
    pub display_name: Option<String>,
    pub email_address: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct StorageQuota {
    /// `None` nghĩa là không giới hạn (Google bỏ field này).
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub limit: Option<u64>,
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub usage: Option<u64>,
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub usage_in_drive: Option<u64>,
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub usage_in_drive_trash: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct About {
    pub user: Option<AboutUser>,
    pub storage_quota: Option<StorageQuota>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct PermissionDetail {
    pub inherited: Option<bool>,
    pub permission_type: Option<String>,
    pub role: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub role: String,
    pub email_address: Option<String>,
    pub display_name: Option<String>,
    pub domain: Option<String>,
    pub deleted: Option<bool>,
    pub permission_details: Option<Vec<PermissionDetail>>,
}

// ───────────────────────── truy vấn & kiểm tra ─────────────────────────

/// Escape một chuỗi để đặt trong dấu nháy đơn của tham số `q`.
pub fn escape_q(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

/// ID của Drive chỉ gồm [A-Za-z0-9_-]; chặn mọi thứ khác trước khi ghép vào đường dẫn URL.
pub fn check_id(id: &str) -> Result<&str> {
    let ok = !id.is_empty()
        && id.len() <= 256
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if ok {
        Ok(id)
    } else {
        Err(AppError::msg(format!("ID không hợp lệ: {id:?}")))
    }
}

impl ListParams {
    pub fn build_query(&self) -> Result<String> {
        let mut parts: Vec<String> = Vec::new();
        match self.mode {
            ListMode::Children => {
                let parent = self.parent_id.as_deref().filter(|s| !s.is_empty()).unwrap_or("root");
                check_id(parent)?;
                parts.push(format!("'{}' in parents", escape_q(parent)));
                parts.push("trashed = false".into());
            }
            ListMode::Owned => {
                parts.push("'me' in owners".into());
                parts.push("trashed = false".into());
            }
            ListMode::Shared => {
                parts.push("sharedWithMe = true".into());
                parts.push("trashed = false".into());
            }
            ListMode::Trash => parts.push("trashed = true".into()),
            ListMode::Search => {
                let q = self.query.as_deref().unwrap_or("").trim();
                if q.is_empty() {
                    return Err(AppError::msg("Từ khoá tìm kiếm đang trống."));
                }
                parts.push(format!("name contains '{}'", escape_q(q)));
                parts.push("trashed = false".into());
            }
        }
        if self.folders_only.unwrap_or(false) {
            parts.push(format!("mimeType = '{FOLDER_MIME}'"));
        }
        Ok(parts.join(" and "))
    }
}

/// Bảng xuất cho file Google Docs/Sheets/Slides/Drawings (không tải thẳng được).
pub fn export_target(mime: &str) -> Option<(&'static str, &'static str)> {
    match mime {
        "application/vnd.google-apps.document" => Some((
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "docx",
        )),
        "application/vnd.google-apps.spreadsheet" => Some((
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "xlsx",
        )),
        "application/vnd.google-apps.presentation" => Some((
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            "pptx",
        )),
        "application/vnd.google-apps.drawing" => Some(("image/png", "png")),
        _ => None,
    }
}

pub fn is_google_native(mime: &str) -> bool {
    mime.starts_with("application/vnd.google-apps.")
}

// ───────────────────────── lỗi từ Google ─────────────────────────

pub fn parse_drive_error_body(status: u16, body: &str) -> AppError {
    #[derive(Deserialize)]
    struct Env {
        error: Option<Inner>,
    }
    #[derive(Deserialize)]
    struct Inner {
        message: Option<String>,
        status: Option<String>,
        errors: Option<Vec<Item>>,
        details: Option<Vec<serde_json::Value>>,
    }
    #[derive(Deserialize)]
    struct Item {
        reason: Option<String>,
        message: Option<String>,
    }

    if let Ok(Env { error: Some(e) }) = serde_json::from_str::<Env>(body) {
        let first = e.errors.as_ref().and_then(|v| v.first());
        let reason = first
            .and_then(|i| i.reason.clone())
            .or_else(|| {
                e.details.as_ref().and_then(|d| {
                    d.iter()
                        .find_map(|x| x.get("reason").and_then(|r| r.as_str()).map(String::from))
                })
            })
            .or(e.status.clone())
            .unwrap_or_else(|| format!("http_{status}"));
        let message = e
            .message
            .or_else(|| first.and_then(|i| i.message.clone()))
            .unwrap_or_else(|| "(không có mô tả)".into());
        return AppError::Drive { status, reason, message };
    }
    let snippet: String = body.chars().take(300).collect();
    AppError::Drive {
        status,
        reason: format!("http_{status}"),
        message: if snippet.trim().is_empty() {
            StatusCode::from_u16(status)
                .ok()
                .and_then(|s| s.canonical_reason())
                .unwrap_or("lỗi không xác định")
                .to_string()
        } else {
            snippet
        },
    }
}

pub(crate) async fn error_from_response(resp: reqwest::Response) -> AppError {
    let status = resp.status().as_u16();
    let text = resp.text().await.unwrap_or_default();
    parse_drive_error_body(status, &text)
}

fn retry_after(resp: &reqwest::Response) -> Option<Duration> {
    resp.headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|s| Duration::from_secs(s.min(60)))
}

// ───────────────────────── client ─────────────────────────

pub struct DriveApi {
    pub(crate) http: reqwest::Client,
    /// Client riêng cho upload resumable: không tự theo redirect, vì Drive dùng
    /// mã 308 (Resume Incomplete) vốn trùng mã "Permanent Redirect".
    pub(crate) upload_http: reqwest::Client,
    pub(crate) api_base: String,
    pub(crate) upload_base: String,
    pub(crate) retry_base: Duration,
}

/// Cài crypto provider cho rustls (một lần cho cả tiến trình). Phải gọi trước khi tạo reqwest::Client.
pub fn init_tls() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

fn builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .user_agent(concat!("sa-drive/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(90))
        .pool_idle_timeout(Duration::from_secs(30))
        .tcp_keepalive(Duration::from_secs(30))
}

impl DriveApi {
    pub fn new() -> Result<Self> {
        Self::with_bases(API_BASE, API_BASE)
    }

    pub fn with_bases(api_base: &str, upload_base: &str) -> Result<Self> {
        init_tls();
        Ok(Self {
            http: builder().build()?,
            upload_http: builder().redirect(reqwest::redirect::Policy::none()).build()?,
            api_base: api_base.trim_end_matches('/').to_string(),
            upload_base: upload_base.trim_end_matches('/').to_string(),
            retry_base: Duration::from_millis(800),
        })
    }

    /// Client dùng chung cho việc lấy token.
    pub fn token_client(&self) -> reqwest::Client {
        self.http.clone()
    }

    pub(crate) fn backoff(&self, attempt: u32, hint: Option<Duration>) -> Duration {
        let exp = self.retry_base.saturating_mul(1u32 << attempt.min(5));
        hint.unwrap_or(exp).min(Duration::from_secs(30))
    }

    /// Gửi request, tự làm mới token khi gặp 401 và tự thử lại (backoff mũ) khi gặp
    /// 429 / 5xx / giới hạn tần suất / lỗi mạng tạm thời. Chỉ trả về `Ok` với mã 2xx.
    pub(crate) async fn execute<F>(
        &self,
        client: &reqwest::Client,
        ts: &TokenSource,
        is_post: bool,
        build: F,
    ) -> Result<reqwest::Response>
    where
        F: Fn(&reqwest::Client, &str) -> reqwest::RequestBuilder,
    {
        let mut attempt = 0u32;
        let mut refreshed = false;
        loop {
            let token = ts.token().await?;
            match build(client, &token).send().await {
                Ok(resp) if resp.status().is_success() => return Ok(resp),
                Ok(resp) => {
                    let hint = retry_after(&resp);
                    let status = resp.status();
                    let err = error_from_response(resp).await;
                    if status == StatusCode::UNAUTHORIZED && !refreshed {
                        refreshed = true;
                        ts.invalidate().await;
                        continue;
                    }
                    if err.is_transient() && attempt < MAX_RETRIES {
                        tokio::time::sleep(self.backoff(attempt, hint)).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(err);
                }
                Err(e) => {
                    let transient = e.is_connect() || e.is_timeout() || e.is_request();
                    // POST không idempotent: chỉ thử lại khi chắc chắn chưa gửi được (lỗi kết nối).
                    let safe = !is_post || e.is_connect();
                    if transient && safe && attempt < MAX_RETRIES {
                        tokio::time::sleep(self.backoff(attempt, None)).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(e.into());
                }
            }
        }
    }

    async fn json<T: DeserializeOwned>(
        &self,
        ts: &TokenSource,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        let url = format!("{}{}", self.api_base, path);
        let is_post = method == Method::POST;
        let resp = self
            .execute(&self.http, ts, is_post, |c, tok| {
                let mut rb = c
                    .request(method.clone(), &url)
                    .bearer_auth(tok)
                    .query(query)
                    .timeout(API_TIMEOUT);
                if let Some(b) = &body {
                    rb = rb.json(b);
                }
                rb
            })
            .await?;
        Ok(resp.json::<T>().await?)
    }

    async fn empty(
        &self,
        ts: &TokenSource,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<serde_json::Value>,
    ) -> Result<()> {
        let url = format!("{}{}", self.api_base, path);
        let is_post = method == Method::POST;
        self.execute(&self.http, ts, is_post, |c, tok| {
            let mut rb = c.request(method.clone(), &url).bearer_auth(tok).query(query).timeout(API_TIMEOUT);
            if let Some(b) = &body {
                rb = rb.json(b);
            }
            rb
        })
        .await?;
        Ok(())
    }

    // ── thông tin account ──

    pub async fn about(&self, ts: &TokenSource) -> Result<About> {
        self.json(
            ts,
            Method::GET,
            "/drive/v3/about",
            &[("fields", "user(displayName,emailAddress),storageQuota(limit,usage,usageInDrive,usageInDriveTrash)")],
            None,
        )
        .await
    }

    pub async fn shared_drives(&self, ts: &TokenSource) -> Result<Vec<SharedDrive>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Page {
            #[serde(default)]
            drives: Vec<SharedDrive>,
            next_page_token: Option<String>,
        }
        let mut out = Vec::new();
        let mut token: Option<String> = None;
        loop {
            let mut q = vec![
                ("pageSize", "100"),
                ("fields", "nextPageToken,drives(id,name,colorRgb)"),
            ];
            if let Some(t) = &token {
                q.push(("pageToken", t.as_str()));
            }
            let page: Page = self.json(ts, Method::GET, "/drive/v3/drives", &q, None).await?;
            out.extend(page.drives);
            match page.next_page_token {
                Some(t) if !t.is_empty() => token = Some(t),
                _ => break,
            }
        }
        out.sort_by_key(|d| d.name.to_lowercase());
        Ok(out)
    }

    // ── tệp & thư mục ──

    pub async fn list(&self, ts: &TokenSource, p: &ListParams) -> Result<ListPage> {
        let q = p.build_query()?;
        let fields = format!("nextPageToken,incompleteSearch,files({FILE_FIELDS})");
        let mut query: Vec<(&str, String)> = vec![
            ("q", q),
            ("pageSize", "200".into()),
            ("fields", fields),
            ("supportsAllDrives", "true".into()),
        ];
        match p.drive_id.as_deref().filter(|d| !d.is_empty()) {
            Some(d) => {
                check_id(d)?;
                query.push(("corpora", "drive".into()));
                query.push(("driveId", d.into()));
                query.push(("includeItemsFromAllDrives", "true".into()));
            }
            None => {
                let all = !matches!(p.mode, ListMode::Trash);
                query.push(("includeItemsFromAllDrives", all.to_string()));
            }
        }
        if let Some(t) = p.page_token.as_deref().filter(|t| !t.is_empty()) {
            query.push(("pageToken", t.into()));
        }
        let q_ref: Vec<(&str, &str)> = query.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.json(ts, Method::GET, "/drive/v3/files", &q_ref, None).await
    }

    /// Lấy toàn bộ con trực tiếp của một thư mục (qua mọi trang).
    pub async fn list_all_children(&self, ts: &TokenSource, folder_id: &str, drive_id: Option<&str>) -> Result<Vec<DriveFile>> {
        let mut out = Vec::new();
        let mut token: Option<String> = None;
        loop {
            let page = self
                .list(
                    ts,
                    &ListParams {
                        mode: ListMode::Children,
                        parent_id: Some(folder_id.into()),
                        drive_id: drive_id.map(String::from),
                        query: None,
                        folders_only: None,
                        page_token: token.clone(),
                    },
                )
                .await?;
            out.extend(page.files);
            match page.next_page_token {
                Some(t) if !t.is_empty() => token = Some(t),
                _ => break,
            }
        }
        Ok(out)
    }

    pub async fn create_folder(&self, ts: &TokenSource, name: &str, parent_id: &str) -> Result<DriveFile> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::msg("Tên thư mục không được để trống."));
        }
        let mut body = json!({ "name": name, "mimeType": FOLDER_MIME });
        if !parent_id.is_empty() && parent_id != "root" {
            body["parents"] = json!([check_id(parent_id)?]);
        }
        self.json(
            ts,
            Method::POST,
            "/drive/v3/files",
            &[("supportsAllDrives", "true"), ("fields", FILE_FIELDS)],
            Some(body),
        )
        .await
    }

    /// Tạo file rỗng (0 byte). Upload resumable không dùng được cho file 0 byte.
    pub async fn create_empty_file(&self, ts: &TokenSource, name: &str, parent_id: &str, mime: &str) -> Result<DriveFile> {
        let mut body = json!({ "name": name, "mimeType": mime });
        if !parent_id.is_empty() && parent_id != "root" {
            body["parents"] = json!([check_id(parent_id)?]);
        }
        self.json(
            ts,
            Method::POST,
            "/drive/v3/files",
            &[("supportsAllDrives", "true"), ("fields", FILE_FIELDS)],
            Some(body),
        )
        .await
    }

    pub async fn rename(&self, ts: &TokenSource, id: &str, name: &str) -> Result<DriveFile> {
        let id = check_id(id)?;
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::msg("Tên không được để trống."));
        }
        self.json(
            ts,
            Method::PATCH,
            &format!("/drive/v3/files/{id}"),
            &[("supportsAllDrives", "true"), ("fields", FILE_FIELDS)],
            Some(json!({ "name": name })),
        )
        .await
    }

    /// ID thật của thư mục gốc My Drive. Bí danh `root` được tài liệu đảm bảo cho `files.get`/`files.create`
    /// nhưng không cam kết cho `addParents`, nên đổi sang ID thật trước khi di chuyển.
    async fn root_id(&self, ts: &TokenSource) -> Result<String> {
        #[derive(Deserialize)]
        struct IdOnly {
            id: String,
        }
        let r: IdOnly = self.json(ts, Method::GET, "/drive/v3/files/root", &[("fields", "id")], None).await?;
        check_id(&r.id)?;
        Ok(r.id)
    }

    pub async fn move_file(&self, ts: &TokenSource, id: &str, new_parent: &str, old_parents: &[String]) -> Result<DriveFile> {
        let id = check_id(id)?;
        check_id(new_parent)?;
        for p in old_parents {
            check_id(p)?;
        }
        let new_parent = if new_parent == "root" { self.root_id(ts).await? } else { new_parent.to_string() };
        let remove = old_parents.join(",");
        let mut q = vec![
            ("supportsAllDrives", "true"),
            ("fields", FILE_FIELDS),
            ("addParents", new_parent.as_str()),
        ];
        if !remove.is_empty() {
            q.push(("removeParents", remove.as_str()));
        }
        self.json(ts, Method::PATCH, &format!("/drive/v3/files/{id}"), &q, Some(json!({}))).await
    }

    pub async fn set_trashed(&self, ts: &TokenSource, id: &str, trashed: bool) -> Result<()> {
        let id = check_id(id)?;
        self.empty(
            ts,
            Method::PATCH,
            &format!("/drive/v3/files/{id}"),
            &[("supportsAllDrives", "true"), ("fields", "id")],
            Some(json!({ "trashed": trashed })),
        )
        .await
    }

    /// Xoá vĩnh viễn, không qua thùng rác.
    pub async fn delete(&self, ts: &TokenSource, id: &str) -> Result<()> {
        let id = check_id(id)?;
        self.empty(ts, Method::DELETE, &format!("/drive/v3/files/{id}"), &[("supportsAllDrives", "true")], None).await
    }

    pub async fn empty_trash(&self, ts: &TokenSource) -> Result<()> {
        self.empty(ts, Method::DELETE, "/drive/v3/files/trash", &[], None).await
    }

    // ── chia sẻ ──

    pub async fn permissions(&self, ts: &TokenSource, file_id: &str) -> Result<Vec<Permission>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Page {
            #[serde(default)]
            permissions: Vec<Permission>,
            next_page_token: Option<String>,
        }
        let file_id = check_id(file_id)?;
        let fields = format!("nextPageToken,permissions({PERMISSION_FIELDS})");
        let mut out = Vec::new();
        let mut token: Option<String> = None;
        loop {
            let mut q = vec![("supportsAllDrives", "true"), ("pageSize", "100"), ("fields", fields.as_str())];
            if let Some(t) = &token {
                q.push(("pageToken", t.as_str()));
            }
            let page: Page = self.json(ts, Method::GET, &format!("/drive/v3/files/{file_id}/permissions"), &q, None).await?;
            out.extend(page.permissions.into_iter().filter(|p| p.deleted != Some(true)));
            match page.next_page_token {
                Some(t) if !t.is_empty() => token = Some(t),
                _ => break,
            }
        }
        Ok(out)
    }

    pub async fn share_with_user(&self, ts: &TokenSource, file_id: &str, email: &str, role: &str, notify: bool) -> Result<Permission> {
        let file_id = check_id(file_id)?;
        if !matches!(role, "reader" | "commenter" | "writer") {
            return Err(AppError::msg("Vai trò chia sẻ không hợp lệ."));
        }
        let email = email.trim();
        if !is_plausible_email(email) {
            return Err(AppError::msg("Email không hợp lệ."));
        }
        let notify = notify.to_string();
        self.json(
            ts,
            Method::POST,
            &format!("/drive/v3/files/{file_id}/permissions"),
            &[
                ("supportsAllDrives", "true"),
                ("sendNotificationEmail", notify.as_str()),
                ("fields", PERMISSION_FIELDS),
            ],
            Some(json!({ "type": "user", "role": role, "emailAddress": email })),
        )
        .await
    }

    /// "Bất kỳ ai có liên kết đều xem được". Không gửi `sendNotificationEmail` (Google không cho với type=anyone).
    pub async fn share_anyone(&self, ts: &TokenSource, file_id: &str) -> Result<Permission> {
        let file_id = check_id(file_id)?;
        self.json(
            ts,
            Method::POST,
            &format!("/drive/v3/files/{file_id}/permissions"),
            &[("supportsAllDrives", "true"), ("fields", PERMISSION_FIELDS)],
            Some(json!({ "type": "anyone", "role": "reader", "allowFileDiscovery": false })),
        )
        .await
    }

    pub async fn unshare(&self, ts: &TokenSource, file_id: &str, permission_id: &str) -> Result<()> {
        let file_id = check_id(file_id)?;
        // permissionId có thể là chuỗi số hoặc "anyoneWithLink"
        let pid = check_id(permission_id)?;
        self.empty(
            ts,
            Method::DELETE,
            &format!("/drive/v3/files/{file_id}/permissions/{pid}"),
            &[("supportsAllDrives", "true")],
            None,
        )
        .await
    }
}

pub fn is_plausible_email(s: &str) -> bool {
    let mut parts = s.split('@');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(local), Some(domain), None) => {
            !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.') && !s.contains(char::is_whitespace)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(mode: ListMode) -> ListParams {
        ListParams { mode, parent_id: None, drive_id: None, query: None, folders_only: None, page_token: None }
    }

    #[test]
    fn escapes_quotes_and_backslashes() {
        assert_eq!(escape_q("it's"), "it\\'s");
        assert_eq!(escape_q("a\\b"), "a\\\\b");
        assert_eq!(escape_q("x' or trashed=true or name='"), "x\\' or trashed=true or name=\\'");
    }

    #[test]
    fn builds_queries_per_mode() {
        let mut p = params(ListMode::Children);
        assert_eq!(p.build_query().unwrap(), "'root' in parents and trashed = false");
        p.parent_id = Some("1AbC_d-9".into());
        p.folders_only = Some(true);
        assert_eq!(
            p.build_query().unwrap(),
            "'1AbC_d-9' in parents and trashed = false and mimeType = 'application/vnd.google-apps.folder'"
        );
        assert_eq!(params(ListMode::Owned).build_query().unwrap(), "'me' in owners and trashed = false");
        assert_eq!(params(ListMode::Shared).build_query().unwrap(), "sharedWithMe = true and trashed = false");
        assert_eq!(params(ListMode::Trash).build_query().unwrap(), "trashed = true");

        let mut s = params(ListMode::Search);
        assert!(s.build_query().is_err(), "từ khoá rỗng phải bị từ chối");
        s.query = Some("báo cáo 'Q3'".into());
        assert_eq!(s.build_query().unwrap(), "name contains 'báo cáo \\'Q3\\'' and trashed = false");
    }

    #[test]
    fn rejects_injection_in_ids() {
        let mut p = params(ListMode::Children);
        p.parent_id = Some("x' in parents or '1".into());
        assert!(p.build_query().is_err());
        assert!(check_id("../etc").is_err());
        assert!(check_id("abc/def").is_err());
        assert!(check_id("").is_err());
        assert!(check_id("0AFxyz_-09").is_ok());
        assert!(check_id("root").is_ok());
    }

    #[test]
    fn parses_google_error_bodies() {
        let body = r#"{"error":{"code":403,"message":"Service Accounts do not have storage quota.","errors":[{"message":"Service Accounts do not have storage quota.","domain":"usageLimits","reason":"storageQuotaExceeded"}]}}"#;
        match parse_drive_error_body(403, body) {
            AppError::Drive { status, reason, message } => {
                assert_eq!(status, 403);
                assert_eq!(reason, "storageQuotaExceeded");
                assert!(message.contains("storage quota"));
            }
            e => panic!("{e:?}"),
        }
        // Chỉ có details/ErrorInfo
        let body = r#"{"error":{"code":403,"message":"Google Drive API has not been used in project 1 before or it is disabled.","status":"PERMISSION_DENIED","details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"SERVICE_DISABLED"}]}}"#;
        match parse_drive_error_body(403, body) {
            AppError::Drive { reason, .. } => assert_eq!(reason, "SERVICE_DISABLED"),
            e => panic!("{e:?}"),
        }
        // Không phải JSON
        match parse_drive_error_body(502, "<html>Bad gateway</html>") {
            AppError::Drive { reason, message, .. } => {
                assert_eq!(reason, "http_502");
                assert!(message.contains("Bad gateway"));
            }
            e => panic!("{e:?}"),
        }
    }

    #[test]
    fn deserializes_file_with_string_size() {
        let f: DriveFile = serde_json::from_str(
            r#"{"id":"abc","name":"a.txt","mimeType":"text/plain","size":"1234","parents":["p"],"ownedByMe":true,
                "owners":[{"displayName":"Bot","emailAddress":"bot@x.iam.gserviceaccount.com","me":true}],
                "capabilities":{"canDelete":true,"canTrash":false}}"#,
        )
        .unwrap();
        assert_eq!(f.size, Some(1234));
        assert_eq!(f.owners[0].me, Some(true));
        assert_eq!(f.capabilities.unwrap().can_trash, Some(false));
        // folder không có size, thiếu nhiều field
        let g: DriveFile = serde_json::from_str(r#"{"id":"f","name":"d","mimeType":"application/vnd.google-apps.folder"}"#).unwrap();
        assert!(g.is_folder() && g.size.is_none() && g.parents.is_empty());
        // JSON gửi cho giao diện dùng camelCase và size là số
        let v = serde_json::to_value(&f_with_size()).unwrap();
        assert_eq!(v["mimeType"], "text/plain");
        assert_eq!(v["size"], 7);
    }

    fn f_with_size() -> DriveFile {
        DriveFile { id: "i".into(), name: "n".into(), mime_type: "text/plain".into(), size: Some(7), ..Default::default() }
    }

    #[test]
    fn quota_parses_unlimited_and_zero() {
        let a: About = serde_json::from_str(r#"{"user":{"displayName":"x","emailAddress":"e@x"},"storageQuota":{"limit":"0","usage":"0","usageInDrive":"0","usageInDriveTrash":"0"}}"#).unwrap();
        assert_eq!(a.storage_quota.unwrap().limit, Some(0));
        let b: About = serde_json::from_str(r#"{"storageQuota":{"usage":"5368709120","usageInDrive":"100"}}"#).unwrap();
        let q = b.storage_quota.unwrap();
        assert_eq!(q.limit, None, "không có limit = không giới hạn");
        assert_eq!(q.usage, Some(5368709120));
    }

    #[test]
    fn export_mapping_and_email_check() {
        assert_eq!(export_target("application/vnd.google-apps.spreadsheet").unwrap().1, "xlsx");
        assert!(export_target("application/vnd.google-apps.folder").is_none());
        assert!(is_google_native("application/vnd.google-apps.form"));
        assert!(!is_google_native("application/pdf"));
        assert!(is_plausible_email("a.b+c@example.co"));
        assert!(!is_plausible_email("a@b"));
        assert!(!is_plausible_email("a b@c.com"));
        assert!(!is_plausible_email("@c.com"));
        assert!(!is_plausible_email("a@@c.com"));
    }

    #[test]
    fn backoff_grows_and_is_capped() {
        init_tls();
        let api = DriveApi::new().unwrap();
        assert!(api.backoff(0, None) < api.backoff(2, None));
        assert!(api.backoff(30, None) <= Duration::from_secs(30));
        assert_eq!(api.backoff(0, Some(Duration::from_secs(7))), Duration::from_secs(7));
    }
    #[tokio::test]
    async fn move_resolves_root_alias_before_add_parents() {
        use crate::testutil::{query_param, spawn, Resp};
        let mock = spawn(|req, _| match (req.method.as_str(), req.path()) {
            ("GET", "/drive/v3/files/root") => Resp::json(200, r#"{"id":"0ROOTID"}"#),
            ("PATCH", "/drive/v3/files/F1") => Resp::json(200, r#"{"id":"F1","name":"x","mimeType":"text/plain"}"#),
            _ => Resp::new(404),
        })
        .await;
        let api = DriveApi::with_bases(&mock.base, &mock.base).unwrap();
        let ts = TokenSource::static_token("t");
        api.move_file(&ts, "F1", "root", &["OLD".to_string()]).await.unwrap();
        let reqs = mock.requests();
        let patch = reqs.iter().find(|r| r.method == "PATCH").expect("phải gọi PATCH");
        assert_eq!(query_param(patch, "addParents").as_deref(), Some("0ROOTID"));
        assert_eq!(query_param(patch, "removeParents").as_deref(), Some("OLD"));

        // Với ID thật thì không cần hỏi thêm thư mục gốc.
        api.move_file(&ts, "F1", "FOLDER9", &[]).await.unwrap();
        let reqs = mock.requests();
        assert_eq!(reqs.iter().filter(|r| r.path() == "/drive/v3/files/root").count(), 1);
        let last = reqs.last().unwrap();
        assert_eq!(query_param(last, "addParents").as_deref(), Some("FOLDER9"));
    }
}
