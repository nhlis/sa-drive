//! Tải xuống dạng stream, tự tiếp tục (HTTP Range) khi đứt kết nối giữa chừng.
//! Google Docs/Sheets/Slides/Drawings không tải thẳng được nên dùng `files.export`.

use std::io::SeekFrom;
use std::path::Path;

use futures_util::StreamExt;
use reqwest::header::RANGE;
use reqwest::StatusCode;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

use crate::auth::TokenSource;
use crate::drive::{check_id, export_target, is_google_native, DriveApi};
use crate::error::{AppError, Result};
use crate::upload::ProgressFn;

const MAX_STREAM_RETRIES: u32 = 6;

impl DriveApi {
    /// Ghi nội dung file vào `part` (ghi đè nếu đã có). Trả về số byte đã ghi.
    pub async fn download_to(
        &self,
        ts: &TokenSource,
        file_id: &str,
        mime: &str,
        expected: Option<u64>,
        part: &Path,
        progress: ProgressFn,
        cancel: &CancellationToken,
    ) -> Result<u64> {
        let id = check_id(file_id)?;
        let export = export_target(mime);
        if export.is_none() && is_google_native(mime) {
            return Err(AppError::msg("Loại file Google này không xuất ra được (ví dụ Forms, Sites, Maps)."));
        }
        let (url, query): (String, Vec<(&str, String)>) = match export {
            Some((emime, _)) => (
                format!("{}/drive/v3/files/{id}/export", self.api_base),
                vec![("mimeType", emime.to_string())],
            ),
            None => (
                format!("{}/drive/v3/files/{id}", self.api_base),
                vec![("alt", "media".into()), ("supportsAllDrives", "true".into())],
            ),
        };

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(part)
            .await?;
        let mut written = 0u64;
        let mut retries = 0u32;
        progress(0);

        loop {
            if cancel.is_cancelled() {
                return Err(AppError::Cancelled);
            }
            let from = written;
            let sent = self.execute(&self.http, ts, false, |c, tok| {
                let mut rb = c.get(&url).query(&query).bearer_auth(tok);
                if from > 0 {
                    rb = rb.header(RANGE, format!("bytes={from}-"));
                }
                rb
            });
            let resp = tokio::select! {
                r = sent => r,
                _ = cancel.cancelled() => return Err(AppError::Cancelled),
            };
            let resp = match resp {
                Ok(r) => r,
                // đã tải đủ từ lần trước mà Google báo "Range không thoả"
                Err(AppError::Drive { status: 416, .. }) if expected == Some(written) => break,
                Err(e) => return Err(e),
            };

            if from > 0 && resp.status() == StatusCode::OK {
                // Máy chủ bỏ qua Range và gửi lại từ đầu.
                file.set_len(0).await?;
                file.seek(SeekFrom::Start(0)).await?;
                written = 0;
                progress(0);
            }

            let mut stream = resp.bytes_stream();
            let mut stream_err: Option<AppError> = None;
            loop {
                let next = tokio::select! {
                    n = stream.next() => n,
                    _ = cancel.cancelled() => return Err(AppError::Cancelled),
                };
                match next {
                    Some(Ok(chunk)) => {
                        file.write_all(&chunk).await?;
                        written += chunk.len() as u64;
                        progress(written);
                    }
                    Some(Err(e)) => {
                        stream_err = Some(e.into());
                        break;
                    }
                    None => break,
                }
            }

            match stream_err {
                None => break,
                Some(e) => {
                    retries += 1;
                    if retries > MAX_STREAM_RETRIES {
                        return Err(e);
                    }
                    if export.is_some() {
                        // file xuất không hỗ trợ Range: làm lại từ đầu
                        file.set_len(0).await?;
                        file.seek(SeekFrom::Start(0)).await?;
                        written = 0;
                    }
                    self.sleep_cancellable(self.backoff(retries - 1, None), cancel).await?;
                }
            }
        }

        file.flush().await?;
        drop(file);
        if export.is_none() {
            if let Some(exp) = expected {
                if written != exp {
                    return Err(AppError::msg(format!(
                        "File tải về bị thiếu dữ liệu ({written} / {exp} byte). Hãy thử lại."
                    )));
                }
            }
        }
        progress(written);
        Ok(written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{spawn, temp_dir, Resp};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    fn api(base: &str) -> DriveApi {
        let mut a = DriveApi::with_bases(base, base).unwrap();
        a.retry_base = Duration::from_millis(5);
        a
    }

    fn pattern(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i % 253) as u8).collect()
    }

    #[tokio::test]
    async fn downloads_and_resumes_after_connection_cut() {
        let data = pattern(2_000_000);
        let d2 = data.clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let c2 = calls.clone();
        let mock = spawn(move |req, _| {
            assert!(req.path().ends_with("/drive/v3/files/FILE1"));
            assert!(req.query().contains("alt=media"));
            c2.fetch_add(1, Ordering::SeqCst);
            match req.header("range") {
                None => {
                    // khai báo đủ độ dài nhưng chỉ gửi 40% rồi đóng kết nối
                    let cut = d2.len() * 4 / 10;
                    Resp::new(200).body(d2[..cut].to_vec()).truncated(d2.len())
                }
                Some(r) => {
                    let from: usize = r.strip_prefix("bytes=").unwrap().trim_end_matches('-').parse().unwrap();
                    Resp::new(206)
                        .header("Content-Range", &format!("bytes {from}-{}/{}", d2.len() - 1, d2.len()))
                        .body(d2[from..].to_vec())
                }
            }
        })
        .await;

        let dir = temp_dir("dl1");
        let part = dir.join("out.part");
        let seen: Arc<Mutex<Vec<u64>>> = Arc::default();
        let s2 = seen.clone();
        let n = api(&mock.base)
            .download_to(
                &TokenSource::static_token("t"),
                "FILE1",
                "application/octet-stream",
                Some(data.len() as u64),
                &part,
                Arc::new(move |n| s2.lock().unwrap().push(n)),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(n as usize, data.len());
        assert_eq!(std::fs::read(&part).unwrap(), data);
        assert!(calls.load(Ordering::SeqCst) >= 2, "phải có ít nhất một lần tiếp tục bằng Range");
        assert_eq!(*seen.lock().unwrap().last().unwrap() as usize, data.len());
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn restarts_when_server_ignores_range() {
        let data = pattern(100_000);
        let d2 = data.clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let c2 = calls.clone();
        let mock = spawn(move |_, _| {
            if c2.fetch_add(1, Ordering::SeqCst) == 0 {
                Resp::new(200).body(d2[..30_000].to_vec()).truncated(d2.len())
            } else {
                // bỏ qua Range, trả 200 + toàn bộ file
                Resp::new(200).body(d2.clone())
            }
        })
        .await;
        let dir = temp_dir("dl2");
        let part = dir.join("o.part");
        api(&mock.base)
            .download_to(&TokenSource::static_token("t"), "F2", "application/pdf", Some(data.len() as u64), &part, Arc::new(|_| {}), &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(std::fs::read(&part).unwrap(), data, "không được nối dữ liệu trùng lặp");
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn google_docs_use_export_endpoint() {
        let mock = spawn(|req, _| {
            assert!(req.path().ends_with("/drive/v3/files/DOC1/export"), "{}", req.path());
            assert!(req.query().contains("mimeType=application%2Fvnd.openxmlformats-officedocument.wordprocessingml.document"), "{}", req.query());
            Resp::new(200).body(b"DOCX-BYTES".to_vec())
        })
        .await;
        let dir = temp_dir("dl3");
        let part = dir.join("d.part");
        let n = api(&mock.base)
            .download_to(&TokenSource::static_token("t"), "DOC1", "application/vnd.google-apps.document", None, &part, Arc::new(|_| {}), &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(n, 10);
        assert_eq!(std::fs::read(&part).unwrap(), b"DOCX-BYTES");
        // loại Google không xuất được -> báo lỗi rõ ràng, không gọi mạng
        let err = api(&mock.base)
            .download_to(&TokenSource::static_token("t"), "FORM1", "application/vnd.google-apps.form", None, &part, Arc::new(|_| {}), &CancellationToken::new())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("không xuất"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn detects_short_download_and_reports_drive_errors() {
        // Máy chủ luôn cắt cụt -> sau khi hết lượt thử phải báo lỗi, không im lặng ghi file thiếu.
        let mock = spawn(|_, _| Resp::new(200).body(vec![1; 10]).truncated(100)).await;
        let dir = temp_dir("dl4");
        let part = dir.join("o.part");
        let mut a = api(&mock.base);
        a.retry_base = Duration::from_millis(1);
        let r = a
            .download_to(&TokenSource::static_token("t"), "F3", "text/plain", Some(100), &part, Arc::new(|_| {}), &CancellationToken::new())
            .await;
        assert!(r.is_err());

        let m2 = spawn(|_, _| Resp::json(403, r#"{"error":{"message":"x","errors":[{"reason":"fileNotDownloadable"}]}}"#)).await;
        let e = api(&m2.base)
            .download_to(&TokenSource::static_token("t"), "F4", "text/plain", None, &part, Arc::new(|_| {}), &CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(e.code(), "drive:fileNotDownloadable");
        std::fs::remove_dir_all(dir).ok();
    }
}
