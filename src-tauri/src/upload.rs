//! Tải lên theo giao thức resumable của Google Drive.
//!
//! 1. POST `/upload/drive/v3/files?uploadType=resumable` (kèm metadata) -> header `Location` là URI phiên.
//! 2. PUT từng chunk (bội số 256 KiB) với `Content-Range: bytes a-b/total`.
//!    - 308: nhận chưa đủ, header `Range: bytes=0-N` cho biết Google *thực sự* đã giữ bao nhiêu byte.
//!    - 200/201: xong, body là metadata file.
//! 3. Khi đứt mạng / 5xx: PUT rỗng với `Content-Range: bytes */total` để hỏi trạng thái rồi tiếp tục.
//!    404/410 nghĩa là phiên đã hết hạn -> mở phiên mới.

use std::io::SeekFrom;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, LOCATION, RANGE};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::sync::CancellationToken;

use crate::auth::TokenSource;
use crate::drive::{check_id, error_from_response, DriveApi, DriveFile, API_TIMEOUT, FILE_FIELDS};
use crate::error::{AppError, Result};

/// Phải là bội số của 256 KiB theo yêu cầu của Google (8 MiB = 32 x 256 KiB).
pub const CHUNK_SIZE: u64 = 8 * 1024 * 1024;
const PIECE: usize = 64 * 1024;
const MAX_CONSECUTIVE_FAILURES: u32 = 8;
const MAX_SESSION_RESTARTS: u32 = 2;

/// Callback tiến độ: nhận số byte đã gửi *tính từ đầu file*.
pub type ProgressFn = Arc<dyn Fn(u64) + Send + Sync>;

pub struct UploadRequest<'a> {
    pub path: &'a Path,
    pub name: String,
    pub parent_id: &'a str,
    pub mime: String,
}

enum Outcome {
    Complete(DriveFile),
    /// Offset kế tiếp mà Google đang chờ.
    Next(u64),
}

enum ChunkError {
    Retry(AppError),
    SessionGone,
    Fatal(AppError),
    Cancelled,
}

/// "bytes=0-42" -> 43. Không có header nghĩa là Google chưa nhận byte nào.
pub fn parse_range_next(header: Option<&str>) -> u64 {
    header
        .and_then(|h| h.trim().strip_prefix("bytes="))
        .and_then(|r| r.rsplit('-').next())
        .and_then(|n| n.trim().parse::<u64>().ok())
        .map(|last| last + 1)
        .unwrap_or(0)
}

/// Body dạng stream để báo tiến độ mượt (từng 64 KiB) nhưng vẫn có `Content-Length` rõ ràng.
fn body_with_progress(data: Vec<u8>, base: u64, progress: ProgressFn) -> reqwest::Body {
    let bytes = Bytes::from(data);
    let mut pos = 0usize;
    let it = std::iter::from_fn(move || {
        if pos >= bytes.len() {
            return None;
        }
        let end = (pos + PIECE).min(bytes.len());
        let piece = bytes.slice(pos..end);
        pos = end;
        progress(base + pos as u64);
        Some(Ok::<Bytes, std::io::Error>(piece))
    });
    reqwest::Body::wrap_stream(futures_util::stream::iter(it))
}

impl DriveApi {
    pub(crate) async fn sleep_cancellable(&self, d: Duration, cancel: &CancellationToken) -> Result<()> {
        tokio::select! {
            _ = tokio::time::sleep(d) => Ok(()),
            _ = cancel.cancelled() => Err(AppError::Cancelled),
        }
    }

    pub async fn upload_file(
        &self,
        ts: &TokenSource,
        req: &UploadRequest<'_>,
        progress: ProgressFn,
        cancel: &CancellationToken,
    ) -> Result<DriveFile> {
        let meta = tokio::fs::metadata(req.path).await?;
        if !meta.is_file() {
            return Err(AppError::msg("Đây không phải file thông thường."));
        }
        let size = meta.len();
        progress(0);
        if size == 0 {
            // Resumable không dùng được cho 0 byte: tạo file rỗng chỉ bằng metadata.
            return self.create_empty_file(ts, &req.name, req.parent_id, &req.mime).await;
        }

        let mut file = tokio::fs::File::open(req.path).await?;
        let mut session = self.initiate_session(ts, req, size).await?;
        let mut offset = 0u64;
        let mut failures = 0u32;
        let mut restarts = 0u32;

        loop {
            if cancel.is_cancelled() {
                return Err(AppError::Cancelled);
            }
            let len = CHUNK_SIZE.min(size - offset);
            let mut buf = vec![0u8; len as usize];
            file.seek(SeekFrom::Start(offset)).await?;
            file.read_exact(&mut buf).await.map_err(|e| {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    AppError::msg("File đã bị thay đổi (ngắn đi) trong lúc đang tải lên.")
                } else {
                    e.into()
                }
            })?;

            let mut restart = false;
            match self.put_chunk(ts, &session, offset, buf, size, &progress, cancel).await {
                Ok(Outcome::Complete(f)) => {
                    progress(size);
                    return Ok(f);
                }
                Ok(Outcome::Next(next)) => {
                    if next <= offset {
                        // Google không nhận thêm byte nào: tránh vòng lặp vô hạn.
                        failures += 1;
                        if failures > MAX_CONSECUTIVE_FAILURES {
                            return Err(AppError::msg("Google không nhận thêm dữ liệu cho phiên tải lên này."));
                        }
                        self.sleep_cancellable(self.backoff(failures - 1, None), cancel).await?;
                    } else {
                        failures = 0;
                    }
                    offset = next.min(size);
                    progress(offset);
                }
                Err(ChunkError::Cancelled) => return Err(AppError::Cancelled),
                Err(ChunkError::Fatal(e)) => return Err(e),
                Err(ChunkError::SessionGone) => restart = true,
                Err(ChunkError::Retry(first_err)) => {
                    // Hỏi Google đã nhận tới đâu rồi mới gửi tiếp, thay vì đoán.
                    let mut last = first_err;
                    loop {
                        failures += 1;
                        if failures > MAX_CONSECUTIVE_FAILURES {
                            return Err(last);
                        }
                        self.sleep_cancellable(self.backoff(failures - 1, None), cancel).await?;
                        match self.query_status(ts, &session, size, cancel).await {
                            Ok(Outcome::Complete(f)) => {
                                progress(size);
                                return Ok(f);
                            }
                            Ok(Outcome::Next(next)) => {
                                offset = next.min(size);
                                progress(offset);
                                break;
                            }
                            Err(ChunkError::SessionGone) => {
                                restart = true;
                                break;
                            }
                            Err(ChunkError::Cancelled) => return Err(AppError::Cancelled),
                            Err(ChunkError::Fatal(e)) => return Err(e),
                            Err(ChunkError::Retry(e)) => last = e,
                        }
                    }
                }
            }

            if restart {
                restarts += 1;
                if restarts > MAX_SESSION_RESTARTS {
                    return Err(AppError::msg("Phiên tải lên liên tục bị Google đóng. Hãy thử lại sau."));
                }
                session = self.initiate_session(ts, req, size).await?;
                offset = 0;
                failures = 0;
                progress(0);
            }
        }
    }

    async fn initiate_session(&self, ts: &TokenSource, req: &UploadRequest<'_>, size: u64) -> Result<String> {
        let url = format!("{}/upload/drive/v3/files", self.upload_base);
        let mut body = json!({ "name": req.name, "mimeType": req.mime });
        if !req.parent_id.is_empty() && req.parent_id != "root" {
            body["parents"] = json!([check_id(req.parent_id)?]);
        }
        let size_s = size.to_string();
        let resp = self
            .execute(&self.upload_http, ts, true, |c, tok| {
                c.post(&url)
                    .query(&[("uploadType", "resumable"), ("supportsAllDrives", "true"), ("fields", FILE_FIELDS)])
                    .bearer_auth(tok)
                    .header("X-Upload-Content-Type", req.mime.as_str())
                    .header("X-Upload-Content-Length", size_s.as_str())
                    .json(&body)
                    .timeout(API_TIMEOUT)
            })
            .await?;
        resp.headers()
            .get(LOCATION)
            .and_then(|v| v.to_str().ok())
            .map(String::from)
            .ok_or_else(|| AppError::msg("Google không trả về địa chỉ phiên tải lên (thiếu header Location)."))
    }

    #[allow(clippy::too_many_arguments)]
    async fn put_chunk(
        &self,
        ts: &TokenSource,
        session: &str,
        offset: u64,
        data: Vec<u8>,
        total: u64,
        progress: &ProgressFn,
        cancel: &CancellationToken,
    ) -> std::result::Result<Outcome, ChunkError> {
        let len = data.len() as u64;
        let token = ts.token().await.map_err(token_error)?;
        let body = body_with_progress(data, offset, progress.clone());
        let fut = self
            .upload_http
            .put(session)
            .bearer_auth(&token)
            .header(CONTENT_LENGTH, len)
            .header(CONTENT_RANGE, format!("bytes {}-{}/{}", offset, offset + len - 1, total))
            .body(body)
            .send();
        // Chấp nhận tốc độ chậm nhất ~32 KiB/s trước khi coi là treo.
        let limit = Duration::from_secs(120).max(Duration::from_secs(len / (32 * 1024)));
        let resp = tokio::select! {
            r = tokio::time::timeout(limit, fut) => match r {
                Ok(Ok(resp)) => resp,
                Ok(Err(e)) => return Err(ChunkError::Retry(e.into())),
                Err(_) => return Err(ChunkError::Retry(AppError::Network("hết thời gian chờ khi gửi dữ liệu".into()))),
            },
            _ = cancel.cancelled() => return Err(ChunkError::Cancelled),
        };
        self.classify(ts, resp).await
    }

    async fn query_status(
        &self,
        ts: &TokenSource,
        session: &str,
        total: u64,
        cancel: &CancellationToken,
    ) -> std::result::Result<Outcome, ChunkError> {
        let token = ts.token().await.map_err(token_error)?;
        let fut = self
            .upload_http
            .put(session)
            .bearer_auth(&token)
            .header(CONTENT_LENGTH, 0)
            .header(CONTENT_RANGE, format!("bytes */{total}"))
            .body(Vec::<u8>::new())
            .timeout(API_TIMEOUT)
            .send();
        let resp = tokio::select! {
            r = fut => r.map_err(|e| ChunkError::Retry(e.into()))?,
            _ = cancel.cancelled() => return Err(ChunkError::Cancelled),
        };
        self.classify(ts, resp).await
    }

    async fn classify(&self, ts: &TokenSource, resp: reqwest::Response) -> std::result::Result<Outcome, ChunkError> {
        match resp.status().as_u16() {
            200 | 201 => {
                let f: DriveFile = resp.json().await.map_err(|e| ChunkError::Retry(e.into()))?;
                Ok(Outcome::Complete(f))
            }
            308 => Ok(Outcome::Next(parse_range_next(resp.headers().get(RANGE).and_then(|v| v.to_str().ok())))),
            404 | 410 => Err(ChunkError::SessionGone),
            401 => {
                ts.invalidate().await;
                Err(ChunkError::Retry(AppError::Drive {
                    status: 401,
                    reason: "authError".into(),
                    message: "Token bị từ chối, đang làm mới".into(),
                }))
            }
            _ => {
                let e = error_from_response(resp).await;
                if e.is_transient() {
                    Err(ChunkError::Retry(e))
                } else {
                    Err(ChunkError::Fatal(e))
                }
            }
        }
    }
}

fn token_error(e: AppError) -> ChunkError {
    if e.is_transient() {
        ChunkError::Retry(e)
    } else {
        ChunkError::Fatal(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{parse_content_range, spawn, temp_dir, Resp};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Mutex;

    fn api(base: &str) -> DriveApi {
        let mut a = DriveApi::with_bases(base, base).unwrap();
        a.retry_base = Duration::from_millis(5);
        a
    }

    fn pattern(n: u64) -> Vec<u8> {
        (0..n).map(|i| (i % 251) as u8).collect()
    }

    fn progress_log() -> (ProgressFn, Arc<Mutex<Vec<u64>>>) {
        let seen: Arc<Mutex<Vec<u64>>> = Arc::default();
        let s2 = seen.clone();
        (Arc::new(move |n| s2.lock().unwrap().push(n)), seen)
    }

    #[test]
    fn parses_range_header() {
        assert_eq!(parse_range_next(Some("bytes=0-42")), 43);
        assert_eq!(parse_range_next(Some("bytes=0-8388607")), 8_388_608);
        assert_eq!(parse_range_next(None), 0);
        assert_eq!(parse_range_next(Some("rác")), 0);
    }

    #[test]
    fn chunk_size_is_multiple_of_256k() {
        assert_eq!(CHUNK_SIZE % (256 * 1024), 0);
    }

    #[tokio::test]
    async fn uploads_in_chunks_with_smooth_progress() {
        let size = CHUNK_SIZE * 2 + 12_345;
        let data = pattern(size);
        let dir = temp_dir("up1");
        let path = dir.join("big.bin");
        std::fs::write(&path, &data).unwrap();

        let store: Arc<Mutex<Vec<u8>>> = Arc::default();
        let st = store.clone();
        let mock = spawn(move |req, base| match (req.method.as_str(), req.path()) {
            ("POST", "/upload/drive/v3/files") => {
                Resp::new(200).header("Location", &format!("{base}/session/1"))
            }
            ("PUT", "/session/1") => {
                let (a, b, total) = parse_content_range(req.header("content-range").unwrap()).unwrap();
                let mut s = st.lock().unwrap();
                if a as usize != s.len() {
                    return Resp::json(400, r#"{"error":{"message":"chunk không nối tiếp"}}"#);
                }
                s.extend_from_slice(&req.body);
                if b + 1 == total {
                    Resp::json(200, r#"{"id":"F1","name":"big.bin","mimeType":"application/octet-stream","size":"1"}"#)
                } else {
                    Resp::new(308).header("Range", &format!("bytes=0-{b}"))
                }
            }
            _ => Resp::new(404),
        })
        .await;

        let api = api(&mock.base);
        let ts = TokenSource::static_token("tok");
        let (pf, seen) = progress_log();
        let f = api
            .upload_file(
                &ts,
                &UploadRequest { path: &path, name: "big.bin".into(), parent_id: "PARENT1", mime: "application/octet-stream".into() },
                pf,
                &CancellationToken::new(),
            )
            .await
            .unwrap();

        assert_eq!(f.id, "F1");
        assert_eq!(*store.lock().unwrap(), data, "dữ liệu phía server phải khớp từng byte");

        let reqs = mock.requests();
        let init = &reqs[0];
        assert_eq!(init.header("x-upload-content-length").unwrap(), size.to_string());
        assert_eq!(init.header("x-upload-content-type").unwrap(), "application/octet-stream");
        assert_eq!(init.json()["parents"][0], "PARENT1");
        assert!(init.target.contains("uploadType=resumable") && init.target.contains("supportsAllDrives=true"));
        assert_eq!(init.header("authorization").unwrap(), "Bearer tok");

        let puts: Vec<_> = reqs.iter().filter(|r| r.method == "PUT").collect();
        assert_eq!(puts.len(), 3, "2 chunk đầy + 1 chunk cuối");
        for p in &puts {
            assert!(!p.chunked, "phải gửi Content-Length rõ ràng, không dùng chunked");
            assert_eq!(p.header("content-length").unwrap(), p.body.len().to_string());
        }
        let ranges: Vec<_> = puts.iter().map(|p| p.header("content-range").unwrap().to_string()).collect();
        assert_eq!(ranges[0], format!("bytes 0-{}/{size}", CHUNK_SIZE - 1));
        assert_eq!(ranges[2], format!("bytes {}-{}/{size}", CHUNK_SIZE * 2, size - 1));

        let seen = seen.lock().unwrap();
        assert_eq!(*seen.last().unwrap(), size);
        assert!(seen.len() > 20, "tiến độ phải mượt, không chỉ nhảy theo chunk (có {})", seen.len());
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn resumes_from_what_google_actually_stored() {
        let size = CHUNK_SIZE + 3 * 1024 * 1024; // 11 MiB -> 2 chunk
        let data = pattern(size);
        let dir = temp_dir("up2");
        let path = dir.join("f.bin");
        std::fs::write(&path, &data).unwrap();

        let store: Arc<Mutex<Vec<u8>>> = Arc::default();
        let fail_once = Arc::new(AtomicBool::new(true));
        let (st, fo) = (store.clone(), fail_once.clone());
        let mock = spawn(move |req, base| match (req.method.as_str(), req.path()) {
            ("POST", _) => Resp::new(200).header("Location", &format!("{base}/session/1")),
            ("PUT", "/session/1") => {
                let cr = req.header("content-range").unwrap();
                let mut s = st.lock().unwrap();
                if cr.starts_with("bytes */") {
                    // truy vấn trạng thái
                    return if s.is_empty() {
                        Resp::new(308)
                    } else {
                        Resp::new(308).header("Range", &format!("bytes=0-{}", s.len() - 1))
                    };
                }
                let (a, b, total) = parse_content_range(cr).unwrap();
                if a == CHUNK_SIZE && fo.swap(false, Ordering::SeqCst) {
                    // chunk thứ 2 bị lỗi 503, nhưng Google đã giữ 1 MiB đầu của nó
                    s.extend_from_slice(&req.body[..1024 * 1024]);
                    return Resp::json(503, r#"{"error":{"code":503,"message":"backend"}}"#);
                }
                assert_eq!(a as usize, s.len(), "phải gửi tiếp đúng chỗ Google đang chờ");
                s.extend_from_slice(&req.body);
                if b + 1 == total {
                    Resp::json(200, r#"{"id":"F2"}"#)
                } else {
                    Resp::new(308).header("Range", &format!("bytes=0-{b}"))
                }
            }
            _ => Resp::new(404),
        })
        .await;

        let api = api(&mock.base);
        let ts = TokenSource::static_token("tok");
        let (pf, _) = progress_log();
        let f = api
            .upload_file(
                &ts,
                &UploadRequest { path: &path, name: "f.bin".into(), parent_id: "root", mime: "application/octet-stream".into() },
                pf,
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(f.id, "F2");
        assert_eq!(*store.lock().unwrap(), data);
        assert!(!fail_once.load(Ordering::SeqCst), "lỗi 503 phải đã xảy ra");

        let reqs = mock.requests();
        assert!(reqs.iter().any(|r| r.header("content-range") == Some(&format!("bytes */{size}"))), "phải hỏi trạng thái phiên");
        // parent = root => không gửi `parents`
        assert!(reqs[0].json().get("parents").is_none());
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn restarts_when_session_expired() {
        let data = pattern(300_000);
        let dir = temp_dir("up3");
        let path = dir.join("s.bin");
        std::fs::write(&path, &data).unwrap();
        let sessions = Arc::new(AtomicUsize::new(0));
        let s2 = sessions.clone();
        let mock = spawn(move |req, base| match (req.method.as_str(), req.path()) {
            ("POST", _) => {
                let n = s2.fetch_add(1, Ordering::SeqCst) + 1;
                Resp::new(200).header("Location", &format!("{base}/session/{n}"))
            }
            ("PUT", "/session/1") => Resp::json(404, r#"{"error":{"message":"Not Found"}}"#),
            ("PUT", "/session/2") => Resp::json(200, r#"{"id":"F3"}"#),
            _ => Resp::new(404),
        })
        .await;
        let api = api(&mock.base);
        let (pf, _) = progress_log();
        let f = api
            .upload_file(
                &TokenSource::static_token("t"),
                &UploadRequest { path: &path, name: "s.bin".into(), parent_id: "root", mime: "application/octet-stream".into() },
                pf,
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(f.id, "F3");
        assert_eq!(sessions.load(Ordering::SeqCst), 2);
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn zero_byte_file_is_created_from_metadata() {
        let dir = temp_dir("up4");
        let path = dir.join("empty.txt");
        std::fs::write(&path, b"").unwrap();
        let mock = spawn(|req, _| match (req.method.as_str(), req.path()) {
            ("POST", "/drive/v3/files") => Resp::json(200, r#"{"id":"E1","name":"empty.txt","mimeType":"text/plain","size":"0"}"#),
            _ => Resp::new(404),
        })
        .await;
        let (pf, _) = progress_log();
        let f = api(&mock.base)
            .upload_file(
                &TokenSource::static_token("t"),
                &UploadRequest { path: &path, name: "empty.txt".into(), parent_id: "PAR", mime: "text/plain".into() },
                pf,
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(f.id, "E1");
        let reqs = mock.requests();
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].json()["parents"][0], "PAR");
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn quota_error_is_fatal_and_not_retried() {
        let dir = temp_dir("up5");
        let path = dir.join("x.bin");
        std::fs::write(&path, pattern(1000)).unwrap();
        let mock = spawn(|_, _| {
            Resp::json(
                403,
                r#"{"error":{"code":403,"message":"Service Accounts do not have storage quota.","errors":[{"reason":"storageQuotaExceeded","message":"Service Accounts do not have storage quota."}]}}"#,
            )
        })
        .await;
        let (pf, _) = progress_log();
        let err = api(&mock.base)
            .upload_file(
                &TokenSource::static_token("t"),
                &UploadRequest { path: &path, name: "x.bin".into(), parent_id: "root", mime: "application/octet-stream".into() },
                pf,
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code(), "drive:storageQuotaExceeded");
        assert!(err.hint().unwrap().contains("Shared Drive"));
        assert_eq!(mock.requests().len(), 1, "lỗi quota không được thử lại");
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn cancellation_stops_before_sending_data() {
        let dir = temp_dir("up6");
        let path = dir.join("c.bin");
        std::fs::write(&path, pattern(5000)).unwrap();
        let mock = spawn(|req, base| match req.method.as_str() {
            "POST" => Resp::new(200).header("Location", &format!("{base}/session/1")),
            _ => Resp::new(404),
        })
        .await;
        let cancel = CancellationToken::new();
        cancel.cancel();
        let (pf, _) = progress_log();
        let err = api(&mock.base)
            .upload_file(
                &TokenSource::static_token("t"),
                &UploadRequest { path: &path, name: "c.bin".into(), parent_id: "root", mime: "application/octet-stream".into() },
                pf,
                &cancel,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Cancelled));
        assert!(mock.requests().iter().all(|r| r.method != "PUT"));
        std::fs::remove_dir_all(dir).ok();
    }
}
