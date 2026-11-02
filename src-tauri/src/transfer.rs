//! Hàng đợi truyền tải: giới hạn số tác vụ song song, báo tiến độ, huỷ, thử lại.
//! Đồng thời lập kế hoạch cho cả thư mục (tạo cây thư mục rồi xếp từng file vào hàng đợi).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use crate::auth::TokenSource;
use crate::drive::{export_target, is_google_native, DriveApi, DriveFile, FOLDER_MIME, SHORTCUT_MIME};
use crate::error::{AppError, Result};
use crate::fsutil::{reserve_unique_dir, reserve_unique_file, sanitize_filename};
use crate::upload::{ProgressFn, UploadRequest};

pub const EVENT: &str = "transfer-update";
const MAX_CONCURRENT: usize = 3;
const EMIT_INTERVAL: Duration = Duration::from_millis(120);
const PLAN_BATCH: usize = 100;

#[derive(Clone, Copy, Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum TransferKind {
    Upload,
    Download,
}

#[derive(Clone, Copy, Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum TransferState {
    Queued,
    Running,
    Done,
    Error,
    Cancelled,
}

impl TransferState {
    fn is_terminal(self) -> bool {
        matches!(self, TransferState::Done | TransferState::Error | TransferState::Cancelled)
    }
}

#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TransferInfo {
    pub id: String,
    pub account_id: String,
    pub kind: TransferKind,
    pub name: String,
    /// 0 = chưa biết (ví dụ file Google Docs xuất ra).
    pub total: u64,
    pub done: u64,
    pub state: TransferState,
    /// Lỗi (state = error) hoặc ghi chú (state = done).
    pub message: Option<String>,
    pub local_path: Option<String>,
    pub remote_id: Option<String>,
    pub parent_id: Option<String>,
    pub created_at: i64,
}

#[derive(Clone, Debug)]
pub enum Job {
    Upload { path: PathBuf, parent_id: String },
    Download { file_id: String, mime_type: String, size: Option<u64>, dest_dir: PathBuf, file_name: String },
    /// Mục chỉ để thông báo lỗi/ghi chú từ bước lập kế hoạch; không chạy được.
    Note,
}

pub struct NewJob {
    pub kind: TransferKind,
    pub name: String,
    pub total: u64,
    pub job: Job,
    pub local_path: Option<String>,
    pub parent_id: Option<String>,
}

pub struct JobCtx {
    pub api: Arc<DriveApi>,
    pub ts: Arc<TokenSource>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DownloadItem {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub size: Option<u64>,
    pub drive_id: Option<String>,
}

/// Nơi nhận sự kiện cập nhật (Tauri trong ứng dụng thật, bộ thu thập trong test).
pub trait Sink: Send + Sync + 'static {
    fn send(&self, items: Vec<TransferInfo>);
}

struct Entry {
    info: TransferInfo,
    job: Job,
    ctx: Option<Arc<JobCtx>>,
    cancel: CancellationToken,
    last_emit: Instant,
}

pub struct TransferManager {
    sink: Arc<dyn Sink>,
    entries: Mutex<Vec<Entry>>,
    sem: Arc<Semaphore>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn display_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| p.to_string_lossy().to_string())
}

fn part_path(final_path: &Path) -> PathBuf {
    let mut os = final_path.as_os_str().to_owned();
    os.push(".part");
    PathBuf::from(os)
}

fn join_err(e: tokio::task::JoinError) -> AppError {
    AppError::msg(format!("Tác vụ nền bị gián đoạn: {e}"))
}

impl TransferManager {
    pub fn new(sink: Arc<dyn Sink>) -> Arc<Self> {
        Arc::new(Self {
            sink,
            entries: Mutex::new(Vec::new()),
            sem: Arc::new(Semaphore::new(MAX_CONCURRENT)),
        })
    }

    pub fn list(&self) -> Vec<TransferInfo> {
        self.entries.lock().unwrap().iter().map(|e| e.info.clone()).collect()
    }

    /// Thêm nhiều job cùng lúc, phát một sự kiện duy nhất rồi khởi chạy worker cho từng job.
    pub fn enqueue_many(self: &Arc<Self>, ctx: Arc<JobCtx>, account_id: &str, jobs: Vec<NewJob>) -> Vec<String> {
        let mut infos = Vec::with_capacity(jobs.len());
        let mut ids = Vec::with_capacity(jobs.len());
        {
            let mut g = self.entries.lock().unwrap();
            for j in jobs {
                let id = uuid::Uuid::new_v4().to_string();
                let info = TransferInfo {
                    id: id.clone(),
                    account_id: account_id.to_string(),
                    kind: j.kind,
                    name: j.name,
                    total: j.total,
                    done: 0,
                    state: TransferState::Queued,
                    message: None,
                    local_path: j.local_path,
                    remote_id: None,
                    parent_id: j.parent_id,
                    created_at: now_ms(),
                };
                infos.push(info.clone());
                ids.push(id);
                g.push(Entry {
                    info,
                    job: j.job,
                    ctx: Some(ctx.clone()),
                    cancel: CancellationToken::new(),
                    last_emit: Instant::now(),
                });
            }
        }
        if !infos.is_empty() {
            self.sink.send(infos);
        }
        for id in &ids {
            self.spawn_worker(id.clone());
        }
        ids
    }

    /// Ghi một dòng thông báo (lỗi hoặc ghi chú) vào danh sách.
    fn note(&self, account_id: &str, kind: TransferKind, name: String, state: TransferState, message: String) {
        let info = TransferInfo {
            id: uuid::Uuid::new_v4().to_string(),
            account_id: account_id.to_string(),
            kind,
            name,
            total: 0,
            done: 0,
            state,
            message: Some(message),
            local_path: None,
            remote_id: None,
            parent_id: None,
            created_at: now_ms(),
        };
        self.entries.lock().unwrap().push(Entry {
            info: info.clone(),
            job: Job::Note,
            ctx: None,
            cancel: CancellationToken::new(),
            last_emit: Instant::now(),
        });
        self.sink.send(vec![info]);
    }

    fn note_error(&self, account_id: &str, kind: TransferKind, name: String, e: &AppError) {
        self.note(account_id, kind, name, TransferState::Error, e.display_full());
    }

    fn spawn_worker(self: &Arc<Self>, id: String) {
        let me = self.clone();
        tokio::spawn(async move { me.run(id).await });
    }

    fn snapshot(&self, id: &str) -> Option<(Job, Option<Arc<JobCtx>>, CancellationToken)> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .find(|e| e.info.id == id)
            .map(|e| (e.job.clone(), e.ctx.clone(), e.cancel.clone()))
    }

    fn update(&self, id: &str, f: impl FnOnce(&mut TransferInfo)) {
        let snap = {
            let mut g = self.entries.lock().unwrap();
            g.iter_mut().find(|e| e.info.id == id).map(|e| {
                f(&mut e.info);
                e.last_emit = Instant::now();
                e.info.clone()
            })
        };
        if let Some(info) = snap {
            self.sink.send(vec![info]);
        }
    }

    fn progress(&self, id: &str, done: u64) {
        let snap = {
            let mut g = self.entries.lock().unwrap();
            g.iter_mut().find(|e| e.info.id == id).and_then(|e| {
                e.info.done = done;
                if e.info.total != 0 && done > e.info.total {
                    e.info.total = done;
                }
                if e.last_emit.elapsed() >= EMIT_INTERVAL {
                    e.last_emit = Instant::now();
                    Some(e.info.clone())
                } else {
                    None
                }
            })
        };
        if let Some(info) = snap {
            self.sink.send(vec![info]);
        }
    }

    fn finish(&self, id: &str, result: Result<()>, remote_id: Option<String>, local_path: Option<String>) {
        self.update(id, |i| match &result {
            Ok(()) => {
                i.state = TransferState::Done;
                if i.total != 0 {
                    i.done = i.total;
                }
                i.message = None;
                if remote_id.is_some() {
                    i.remote_id = remote_id.clone();
                }
                if local_path.is_some() {
                    i.local_path = local_path.clone();
                }
            }
            Err(AppError::Cancelled) => {
                i.state = TransferState::Cancelled;
                i.message = None;
            }
            Err(e) => {
                i.state = TransferState::Error;
                i.message = Some(e.display_full());
            }
        });
    }

    async fn run(self: Arc<Self>, id: String) {
        let Some((job, Some(ctx), cancel)) = self.snapshot(&id) else { return };

        // Chờ đến lượt (hàng đợi FIFO), nhưng huỷ được ngay cả khi chưa bắt đầu.
        let permit = tokio::select! {
            p = self.sem.clone().acquire_owned() => p.ok(),
            _ = cancel.cancelled() => None,
        };
        let Some(_permit) = permit else {
            self.finish(&id, Err(AppError::Cancelled), None, None);
            return;
        };
        if cancel.is_cancelled() {
            self.finish(&id, Err(AppError::Cancelled), None, None);
            return;
        }
        self.update(&id, |i| {
            i.state = TransferState::Running;
            i.done = 0;
            i.message = None;
        });

        let progress: ProgressFn = {
            let me = self.clone();
            let id = id.clone();
            Arc::new(move |n| me.progress(&id, n))
        };

        match job {
            Job::Upload { path, parent_id } => match self.run_upload(&ctx, path, parent_id, progress, &cancel).await {
                Ok(f) => self.finish(&id, Ok(()), Some(f.id), None),
                Err(e) => self.finish(&id, Err(e), None, None),
            },
            Job::Download { file_id, mime_type, size, dest_dir, file_name } => {
                match self.run_download(&ctx, file_id, mime_type, size, dest_dir, file_name, progress, &cancel).await {
                    Ok(p) => self.finish(&id, Ok(()), None, Some(p.to_string_lossy().to_string())),
                    Err(e) => self.finish(&id, Err(e), None, None),
                }
            }
            Job::Note => {}
        }
    }

    async fn run_upload(
        &self,
        ctx: &JobCtx,
        path: PathBuf,
        parent_id: String,
        progress: ProgressFn,
        cancel: &CancellationToken,
    ) -> Result<DriveFile> {
        let name = display_name(&path);
        let mime = mime_guess::from_path(&path).first_or_octet_stream().essence_str().to_string();
        ctx.api
            .upload_file(
                &ctx.ts,
                &UploadRequest { path: &path, name, parent_id: &parent_id, mime },
                progress,
                cancel,
            )
            .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_download(
        &self,
        ctx: &JobCtx,
        file_id: String,
        mime_type: String,
        size: Option<u64>,
        dest_dir: PathBuf,
        file_name: String,
        progress: ProgressFn,
        cancel: &CancellationToken,
    ) -> Result<PathBuf> {
        let mut name = sanitize_filename(&file_name);
        if let Some((_, ext)) = export_target(&mime_type) {
            if !name.to_lowercase().ends_with(&format!(".{ext}")) {
                name = format!("{name}.{ext}");
            }
        }
        let final_path = tokio::task::spawn_blocking(move || reserve_unique_file(&dest_dir, &name))
            .await
            .map_err(join_err)??;
        let part = part_path(&final_path);
        match ctx.api.download_to(&ctx.ts, &file_id, &mime_type, size, &part, progress, cancel).await {
            Ok(_) => {
                tokio::fs::rename(&part, &final_path).await?;
                Ok(final_path)
            }
            Err(e) => {
                let _ = tokio::fs::remove_file(&part).await;
                let _ = tokio::fs::remove_file(&final_path).await;
                Err(e)
            }
        }
    }

    pub fn cancel(&self, id: &str) {
        if let Some(e) = self.entries.lock().unwrap().iter().find(|e| e.info.id == id) {
            e.cancel.cancel();
        }
    }

    pub fn cancel_all(&self) {
        for e in self.entries.lock().unwrap().iter() {
            if !e.info.state.is_terminal() {
                e.cancel.cancel();
            }
        }
    }

    pub fn retry(self: &Arc<Self>, id: &str) -> Result<()> {
        let snap = {
            let mut g = self.entries.lock().unwrap();
            let e = g
                .iter_mut()
                .find(|e| e.info.id == id)
                .ok_or_else(|| AppError::msg("Không tìm thấy mục truyền tải."))?;
            if matches!(e.job, Job::Note) {
                return Err(AppError::msg("Mục này không thử lại được; hãy thực hiện lại thao tác từ đầu."));
            }
            if !matches!(e.info.state, TransferState::Error | TransferState::Cancelled) {
                return Err(AppError::msg("Chỉ thử lại được mục bị lỗi hoặc đã huỷ."));
            }
            e.cancel = CancellationToken::new();
            e.info.state = TransferState::Queued;
            e.info.done = 0;
            e.info.message = None;
            e.last_emit = Instant::now();
            e.info.clone()
        };
        self.sink.send(vec![snap]);
        self.spawn_worker(id.to_string());
        Ok(())
    }

    /// Xoá các mục đã kết thúc khỏi danh sách, trả về phần còn lại.
    pub fn clear_finished(&self) -> Vec<TransferInfo> {
        let mut g = self.entries.lock().unwrap();
        g.retain(|e| !e.info.state.is_terminal());
        g.iter().map(|e| e.info.clone()).collect()
    }

    fn flush(self: &Arc<Self>, ctx: &Arc<JobCtx>, account_id: &str, pending: &mut Vec<NewJob>) {
        if !pending.is_empty() {
            self.enqueue_many(ctx.clone(), account_id, std::mem::take(pending));
        }
    }

    // ───────────── lập kế hoạch tải lên ─────────────

    pub async fn plan_upload(self: Arc<Self>, ctx: Arc<JobCtx>, account_id: String, parent_id: String, paths: Vec<PathBuf>) {
        let mut pending: Vec<NewJob> = Vec::new();
        for p in paths {
            let meta = match tokio::fs::metadata(&p).await {
                Ok(m) => m,
                Err(e) => {
                    self.note_error(&account_id, TransferKind::Upload, display_name(&p), &e.into());
                    continue;
                }
            };
            if meta.is_file() {
                pending.push(upload_job(&p, meta.len(), &parent_id));
                if pending.len() >= PLAN_BATCH {
                    self.flush(&ctx, &account_id, &mut pending);
                }
            } else if meta.is_dir() {
                self.flush(&ctx, &account_id, &mut pending);
                self.plan_upload_dir(&ctx, &account_id, &parent_id, &p, &mut pending).await;
            } else {
                self.note(&account_id, TransferKind::Upload, display_name(&p), TransferState::Error, "Không phải file hay thư mục.".into());
            }
        }
        self.flush(&ctx, &account_id, &mut pending);
    }

    async fn plan_upload_dir(
        self: &Arc<Self>,
        ctx: &Arc<JobCtx>,
        account_id: &str,
        parent_id: &str,
        root: &Path,
        pending: &mut Vec<NewJob>,
    ) {
        let root_name = display_name(root);
        let root_remote = match ctx.api.create_folder(&ctx.ts, &root_name, parent_id).await {
            Ok(f) => f.id,
            Err(e) => {
                self.note_error(account_id, TransferKind::Upload, format!("{root_name}/ — không tạo được thư mục trên Drive"), &e);
                return;
            }
        };
        let root_buf = root.to_path_buf();
        let (entries, skipped) = match tokio::task::spawn_blocking(move || walk(&root_buf)).await {
            Ok(v) => v,
            Err(e) => {
                self.note_error(account_id, TransferKind::Upload, root_name, &join_err(e));
                return;
            }
        };

        let mut dir_map: HashMap<PathBuf, String> = HashMap::new();
        dir_map.insert(root.to_path_buf(), root_remote);
        let mut failed: HashSet<PathBuf> = HashSet::new();

        // walkdir trả về theo thứ tự thư mục cha trước nội dung, nên khi gặp một mục thì thư mục cha đã được tạo.
        for e in entries {
            if e.path.ancestors().skip(1).any(|a| failed.contains(a)) {
                continue;
            }
            let Some(remote_parent) = e.path.parent().and_then(|p| dir_map.get(p)).cloned() else {
                continue;
            };
            if e.is_dir {
                // Đẩy các file đã sẵn sàng vào hàng đợi trước khi tốn thời gian tạo thư mục.
                self.flush(ctx, account_id, pending);
                match ctx.api.create_folder(&ctx.ts, &display_name(&e.path), &remote_parent).await {
                    Ok(f) => {
                        dir_map.insert(e.path.clone(), f.id);
                    }
                    Err(err) => {
                        self.note_error(account_id, TransferKind::Upload, format!("{}/ — không tạo được thư mục", display_name(&e.path)), &err);
                        failed.insert(e.path.clone());
                    }
                }
            } else {
                pending.push(upload_job(&e.path, e.size, &remote_parent));
                if pending.len() >= PLAN_BATCH {
                    self.flush(ctx, account_id, pending);
                }
            }
        }
        if skipped > 0 {
            self.note(
                account_id,
                TransferKind::Upload,
                format!("{root_name}/"),
                TransferState::Done,
                format!("Đã bỏ qua {skipped} mục (symlink, file đặc biệt hoặc không đọc được)."),
            );
        }
    }

    // ───────────── lập kế hoạch tải xuống ─────────────

    pub async fn plan_download(self: Arc<Self>, ctx: Arc<JobCtx>, account_id: String, items: Vec<DownloadItem>, dest_dir: PathBuf) {
        let mut pending: Vec<NewJob> = Vec::new();
        let mut skipped = 0usize;

        for it in items {
            if it.mime_type == FOLDER_MIME {
                let local = match reserve_dir(&dest_dir, &it.name).await {
                    Ok(p) => p,
                    Err(e) => {
                        self.note_error(&account_id, TransferKind::Download, it.name.clone(), &e);
                        continue;
                    }
                };
                let mut stack = vec![(it.id.clone(), it.drive_id.clone(), local)];
                while let Some((remote_id, drive_id, local_dir)) = stack.pop() {
                    let children = match ctx.api.list_all_children(&ctx.ts, &remote_id, drive_id.as_deref()).await {
                        Ok(c) => c,
                        Err(e) => {
                            self.note_error(&account_id, TransferKind::Download, format!("{}/ — không liệt kê được nội dung", display_name(&local_dir)), &e);
                            continue;
                        }
                    };
                    for c in children {
                        if c.is_folder() {
                            match reserve_dir(&local_dir, &c.name).await {
                                Ok(p) => stack.push((c.id.clone(), c.drive_id.clone().or_else(|| drive_id.clone()), p)),
                                Err(e) => self.note_error(&account_id, TransferKind::Download, c.name.clone(), &e),
                            }
                        } else if !downloadable(&c.mime_type) {
                            skipped += 1;
                        } else {
                            pending.push(download_job(&c.id, &c.name, &c.mime_type, c.size, &local_dir));
                            if pending.len() >= PLAN_BATCH {
                                self.flush(&ctx, &account_id, &mut pending);
                            }
                        }
                    }
                }
            } else if !downloadable(&it.mime_type) {
                skipped += 1;
            } else {
                pending.push(download_job(&it.id, &it.name, &it.mime_type, it.size, &dest_dir));
            }
        }
        self.flush(&ctx, &account_id, &mut pending);
        if skipped > 0 {
            self.note(
                &account_id,
                TransferKind::Download,
                "Một số mục không được tải".into(),
                TransferState::Done,
                format!("Đã bỏ qua {skipped} mục không có nội dung tải được (lối tắt, Google Forms/Sites/Maps…)."),
            );
        }
    }
}

async fn reserve_dir(parent: &Path, name: &str) -> Result<PathBuf> {
    let parent = parent.to_path_buf();
    let name = sanitize_filename(name);
    Ok(tokio::task::spawn_blocking(move || reserve_unique_dir(&parent, &name))
        .await
        .map_err(join_err)??)
}

fn downloadable(mime: &str) -> bool {
    if mime == SHORTCUT_MIME {
        return false;
    }
    !is_google_native(mime) || export_target(mime).is_some()
}

fn upload_job(path: &Path, size: u64, parent_id: &str) -> NewJob {
    NewJob {
        kind: TransferKind::Upload,
        name: display_name(path),
        total: size,
        job: Job::Upload { path: path.to_path_buf(), parent_id: parent_id.to_string() },
        local_path: Some(path.to_string_lossy().to_string()),
        parent_id: Some(parent_id.to_string()),
    }
}

fn download_job(id: &str, name: &str, mime: &str, size: Option<u64>, dest_dir: &Path) -> NewJob {
    NewJob {
        kind: TransferKind::Download,
        name: name.to_string(),
        total: if export_target(mime).is_some() { 0 } else { size.unwrap_or(0) },
        job: Job::Download {
            file_id: id.to_string(),
            mime_type: mime.to_string(),
            size,
            dest_dir: dest_dir.to_path_buf(),
            file_name: name.to_string(),
        },
        local_path: Some(dest_dir.to_string_lossy().to_string()),
        parent_id: None,
    }
}

struct WalkEntry {
    path: PathBuf,
    is_dir: bool,
    size: u64,
}

/// Duyệt cây thư mục (không theo symlink). Trả về danh sách và số mục bị bỏ qua.
fn walk(root: &Path) -> (Vec<WalkEntry>, usize) {
    let mut out = Vec::new();
    let mut skipped = 0usize;
    for e in walkdir::WalkDir::new(root).min_depth(1).follow_links(false).sort_by_file_name() {
        let Ok(e) = e else {
            skipped += 1;
            continue;
        };
        let ft = e.file_type();
        if ft.is_dir() {
            out.push(WalkEntry { path: e.into_path(), is_dir: true, size: 0 });
        } else if ft.is_file() {
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            out.push(WalkEntry { path: e.into_path(), is_dir: false, size });
        } else {
            skipped += 1;
        }
    }
    (out, skipped)
}

/// Sink gửi sự kiện tới giao diện Tauri.
pub struct TauriSink(pub tauri::AppHandle);

impl Sink for TauriSink {
    fn send(&self, items: Vec<TransferInfo>) {
        use tauri::Emitter;
        let _ = self.0.emit(EVENT, items);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{query_param, spawn, temp_dir, Resp};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct VecSink(Mutex<Vec<TransferInfo>>);
    impl Sink for VecSink {
        fn send(&self, items: Vec<TransferInfo>) {
            self.0.lock().unwrap().extend(items);
        }
    }

    fn setup(base: &str) -> (Arc<TransferManager>, Arc<JobCtx>, Arc<VecSink>) {
        let sink = Arc::new(VecSink::default());
        let mut api = DriveApi::with_bases(base, base).unwrap();
        api.retry_base = Duration::from_millis(5);
        let ctx = Arc::new(JobCtx { api: Arc::new(api), ts: Arc::new(TokenSource::static_token("t")) });
        (TransferManager::new(sink.clone()), ctx, sink)
    }

    async fn wait_all_terminal(m: &Arc<TransferManager>, expect_at_least: usize) {
        for _ in 0..400 {
            let l = m.list();
            if l.len() >= expect_at_least && l.iter().all(|i| i.state.is_terminal()) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("hết thời gian chờ; trạng thái: {:#?}", m.list());
    }

    #[tokio::test]
    async fn uploads_a_folder_tree_preserving_structure() {
        // root/a.txt, root/sub/b.txt, root/sub/deep/c.txt, root/empty/
        let dir = temp_dir("tm1");
        let root = dir.join("Dự án");
        std::fs::create_dir_all(root.join("sub/deep")).unwrap();
        std::fs::create_dir_all(root.join("empty")).unwrap();
        std::fs::write(root.join("a.txt"), b"AAA").unwrap();
        std::fs::write(root.join("sub/b.txt"), b"BBBB").unwrap();
        std::fs::write(root.join("sub/deep/c.txt"), b"CCCCC").unwrap();

        let folder_n = Arc::new(AtomicUsize::new(0));
        let sess_n = Arc::new(AtomicUsize::new(0));
        let (fa, sa) = (folder_n.clone(), sess_n.clone());
        let mock = spawn(move |req, base| match (req.method.as_str(), req.path()) {
            ("POST", "/drive/v3/files") => {
                let n = fa.fetch_add(1, Ordering::SeqCst) + 1;
                let name = req.json()["name"].as_str().unwrap().to_string();
                Resp::json(200, &format!(r#"{{"id":"D{n}","name":"{name}","mimeType":"{FOLDER_MIME}"}}"#))
            }
            ("POST", "/upload/drive/v3/files") => {
                let n = sa.fetch_add(1, Ordering::SeqCst) + 1;
                Resp::new(200).header("Location", &format!("{base}/session/{n}"))
            }
            ("PUT", p) if p.starts_with("/session/") => Resp::json(200, r#"{"id":"UP"}"#),
            _ => Resp::new(404),
        })
        .await;

        let (mgr, ctx, sink) = setup(&mock.base);
        mgr.clone().plan_upload(ctx, "acc1".into(), "PARENT0".into(), vec![root.clone()]).await;
        wait_all_terminal(&mgr, 3).await;

        let list = mgr.list();
        assert_eq!(list.len(), 3);
        assert!(list.iter().all(|i| i.state == TransferState::Done), "{list:#?}");
        assert!(list.iter().all(|i| i.done == i.total && i.account_id == "acc1"));

        let reqs = mock.requests();
        let folders: Vec<_> = reqs.iter().filter(|r| r.path() == "/drive/v3/files").map(|r| r.json()).collect();
        // thứ tự: root, deep nằm trong sub nên sub phải được tạo trước deep
        let names: Vec<_> = folders.iter().map(|f| f["name"].as_str().unwrap().to_string()).collect();
        assert_eq!(names[0], "Dự án");
        let pos = |n: &str| names.iter().position(|x| x == n).unwrap();
        assert!(pos("sub") < pos("deep"));
        assert_eq!(names.len(), 4, "root + sub + deep + empty: {names:?}");
        let parent_of = |n: &str| folders[pos(n)]["parents"][0].as_str().unwrap().to_string();
        assert_eq!(parent_of("Dự án"), "PARENT0");
        assert_eq!(parent_of("sub"), "D1");
        assert_eq!(parent_of("empty"), "D1");
        let sub_id = format!("D{}", pos("sub") + 1);
        assert_eq!(parent_of("deep"), sub_id);

        let uploads: HashMap<String, String> = reqs
            .iter()
            .filter(|r| r.path() == "/upload/drive/v3/files")
            .map(|r| {
                let j = r.json();
                (j["name"].as_str().unwrap().to_string(), j["parents"][0].as_str().unwrap().to_string())
            })
            .collect();
        assert_eq!(uploads["a.txt"], "D1");
        assert_eq!(uploads["b.txt"], sub_id);
        assert_eq!(uploads["c.txt"], format!("D{}", pos("deep") + 1));
        assert!(!sink.0.lock().unwrap().is_empty(), "phải có sự kiện gửi tới giao diện");
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn downloads_a_folder_tree_safely() {
        let list_calls = Arc::new(AtomicUsize::new(0));
        let lc = list_calls.clone();
        let mock = spawn(move |req, _| {
            if req.path() == "/drive/v3/files" {
                lc.fetch_add(1, Ordering::SeqCst);
                let q = query_param(req, "q").unwrap();
                return if q.starts_with("'ROOT' in parents") {
                    Resp::json(200, &format!(
                        r#"{{"files":[{{"id":"A","name":"a.txt","mimeType":"text/plain","size":"3"}},
                                       {{"id":"S","name":"sub","mimeType":"{FOLDER_MIME}"}},
                                       {{"id":"L","name":"lối tắt","mimeType":"{SHORTCUT_MIME}"}}]}}"#
                    ))
                } else if q.starts_with("'S' in parents") {
                    // tên độc hại: cố thoát khỏi thư mục đích
                    Resp::json(200, r#"{"files":[{"id":"B","name":"../../evil.txt","mimeType":"text/plain","size":"4"}]}"#)
                } else {
                    Resp::json(200, r#"{"files":[]}"#)
                };
            }
            match req.path() {
                "/drive/v3/files/A" => Resp::new(200).body(b"AAA".to_vec()),
                "/drive/v3/files/B" => Resp::new(200).body(b"BBBB".to_vec()),
                _ => Resp::new(404),
            }
        })
        .await;

        let dest = temp_dir("tm2");
        let (mgr, ctx, _sink) = setup(&mock.base);
        mgr.clone()
            .plan_download(
                ctx,
                "acc".into(),
                vec![DownloadItem { id: "ROOT".into(), name: "Thư mục gốc".into(), mime_type: FOLDER_MIME.into(), size: None, drive_id: None }],
                dest.clone(),
            )
            .await;
        // 2 file + 1 ghi chú bỏ qua lối tắt
        wait_all_terminal(&mgr, 3).await;
        let list = mgr.list();
        assert!(list.iter().all(|i| i.state == TransferState::Done), "{list:#?}");

        let base = dest.join("Thư mục gốc");
        assert_eq!(std::fs::read(base.join("a.txt")).unwrap(), b"AAA");
        assert_eq!(std::fs::read(base.join("sub").join(".._.._evil.txt")).unwrap(), b"BBBB");
        assert!(!dest.join("evil.txt").exists() && !dest.parent().unwrap().join("evil.txt").exists());
        assert!(list.iter().any(|i| i.message.as_deref().unwrap_or("").contains("bỏ qua 1")));
        // không còn file .part
        fn has_part(p: &Path) -> bool {
            std::fs::read_dir(p).unwrap().any(|e| {
                let e = e.unwrap();
                e.file_name().to_string_lossy().ends_with(".part") || (e.path().is_dir() && has_part(&e.path()))
            })
        }
        assert!(!has_part(&dest));
        std::fs::remove_dir_all(dest).ok();
    }

    #[tokio::test]
    async fn error_then_retry_succeeds() {
        let mock = spawn(|req, base| match (req.method.as_str(), req.path()) {
            ("POST", _) => Resp::new(200).header("Location", &format!("{base}/session/1")),
            ("PUT", _) => Resp::json(200, r#"{"id":"OK1"}"#),
            _ => Resp::new(404),
        })
        .await;
        let dir = temp_dir("tm3");
        let (mgr, ctx, _s) = setup(&mock.base);
        let ghost = dir.join("sẽ-có-sau.txt");
        // file chưa tồn tại: kế hoạch ghi nhận lỗi
        mgr.clone().plan_upload(ctx.clone(), "a".into(), "root".into(), vec![ghost.clone()]).await;
        let l = mgr.list();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].state, TransferState::Error);
        assert!(mgr.retry(&l[0].id).is_err(), "mục ghi chú không thử lại được");

        // job thật: đưa vào hàng đợi trong khi giữ hết permit để nó ở trạng thái chờ, rồi huỷ
        std::fs::write(&ghost, b"xyz").unwrap();
        let hold = mgr.sem.clone().acquire_many_owned(MAX_CONCURRENT as u32).await.unwrap();
        let ids = mgr.enqueue_many(ctx.clone(), "a", vec![upload_job(&ghost, 3, "root")]);
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(mgr.list().last().unwrap().state, TransferState::Queued);
        mgr.cancel(&ids[0]);
        for _ in 0..100 {
            if mgr.list().last().unwrap().state == TransferState::Cancelled {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(mgr.list().last().unwrap().state, TransferState::Cancelled, "huỷ được cả khi đang xếp hàng");
        drop(hold);

        mgr.retry(&ids[0]).unwrap();
        for _ in 0..200 {
            if mgr.list().last().unwrap().state == TransferState::Done {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let last = mgr.list().last().unwrap().clone();
        assert_eq!(last.state, TransferState::Done);
        assert_eq!(last.remote_id.as_deref(), Some("OK1"));

        // clear_finished giữ lại mục chưa xong (không có) và xoá mục đã xong
        assert!(mgr.clear_finished().is_empty());
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn quota_failure_is_reported_with_hint_on_the_transfer() {
        let mock = spawn(|_, _| {
            Resp::json(403, r#"{"error":{"code":403,"message":"Service Accounts do not have storage quota.","errors":[{"reason":"storageQuotaExceeded"}]}}"#)
        })
        .await;
        let dir = temp_dir("tm4");
        let f = dir.join("x.bin");
        std::fs::write(&f, vec![1u8; 100]).unwrap();
        let (mgr, ctx, _s) = setup(&mock.base);
        mgr.clone().plan_upload(ctx, "a".into(), "root".into(), vec![f]).await;
        wait_all_terminal(&mgr, 1).await;
        let i = &mgr.list()[0];
        assert_eq!(i.state, TransferState::Error);
        let msg = i.message.as_deref().unwrap();
        assert!(msg.contains("Shared Drive") && msg.contains("storage quota"), "{msg}");
        std::fs::remove_dir_all(dir).ok();
    }
}
