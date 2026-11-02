//! Mock HTTP server tối giản (không thêm dependency) để test client Drive mà không cần mạng.
//! Mỗi kết nối chỉ phục vụ một request rồi đóng (`Connection: close`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone, Debug)]
pub struct Req {
    pub method: String,
    /// Đường dẫn kèm query.
    pub target: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    /// Client có dùng `Transfer-Encoding: chunked` hay không.
    pub chunked: bool,
}

impl Req {
    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or("")
    }
    pub fn query(&self) -> &str {
        self.target.split_once('?').map(|x| x.1).unwrap_or("")
    }
    pub fn header(&self, k: &str) -> Option<&str> {
        self.headers.get(&k.to_ascii_lowercase()).map(|s| s.as_str())
    }
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or(serde_json::Value::Null)
    }
}

pub struct Resp {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Ghi đè `Content-Length` (khai báo dài hơn thực tế để mô phỏng đứt kết nối giữa chừng).
    pub declared_len: Option<usize>,
}

impl Resp {
    pub fn new(status: u16) -> Self {
        Resp { status, headers: vec![], body: vec![], declared_len: None }
    }
    pub fn json(status: u16, body: &str) -> Self {
        Resp::new(status).header("Content-Type", "application/json").body(body.as_bytes().to_vec())
    }
    pub fn header(mut self, k: &str, v: &str) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }
    pub fn body(mut self, b: Vec<u8>) -> Self {
        self.body = b;
        self
    }
    pub fn truncated(mut self, declared: usize) -> Self {
        self.declared_len = Some(declared);
        self
    }
}

pub struct Mock {
    pub base: String,
    pub log: Arc<Mutex<Vec<Req>>>,
}

impl Mock {
    pub fn requests(&self) -> Vec<Req> {
        self.log.lock().unwrap().clone()
    }
}

pub async fn spawn<H>(handler: H) -> Mock
where
    H: Fn(&Req, &str) -> Resp + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let log: Arc<Mutex<Vec<Req>>> = Arc::new(Mutex::new(Vec::new()));
    let handler = Arc::new(handler);
    let (b2, l2) = (base.clone(), log.clone());
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { break };
            let (h, b, l) = (handler.clone(), b2.clone(), l2.clone());
            tokio::spawn(async move {
                if let Some(req) = read_request(&mut sock).await {
                    l.lock().unwrap().push(req.clone());
                    let resp = h(&req, &b);
                    let _ = write_response(&mut sock, resp).await;
                }
                let _ = sock.shutdown().await;
            });
        }
    });
    Mock { base, log }
}

struct Reader<'a> {
    sock: &'a mut TcpStream,
    buf: Vec<u8>,
}

impl Reader<'_> {
    async fn fill(&mut self) -> bool {
        let mut tmp = [0u8; 64 * 1024];
        match self.sock.read(&mut tmp).await {
            Ok(0) | Err(_) => false,
            Ok(n) => {
                self.buf.extend_from_slice(&tmp[..n]);
                true
            }
        }
    }

    async fn read_line(&mut self) -> Option<String> {
        loop {
            if let Some(i) = self.buf.windows(2).position(|w| w == b"\r\n") {
                let line: Vec<u8> = self.buf.drain(..i + 2).collect();
                return Some(String::from_utf8_lossy(&line[..i]).to_string());
            }
            if !self.fill().await {
                return None;
            }
        }
    }

    async fn read_n(&mut self, n: usize) -> Option<Vec<u8>> {
        while self.buf.len() < n {
            if !self.fill().await {
                return None;
            }
        }
        Some(self.buf.drain(..n).collect())
    }
}

async fn read_request(sock: &mut TcpStream) -> Option<Req> {
    let mut r = Reader { sock, buf: Vec::new() };
    let start = r.read_line().await?;
    let mut parts = start.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let mut headers = HashMap::new();
    loop {
        let line = r.read_line().await?;
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    let chunked = headers.get("transfer-encoding").map(|v| v.to_ascii_lowercase().contains("chunked")).unwrap_or(false);
    let body = if chunked {
        let mut out = Vec::new();
        loop {
            let size_line = r.read_line().await?;
            let size = usize::from_str_radix(size_line.split(';').next()?.trim(), 16).ok()?;
            if size == 0 {
                // trailer (nếu có) rồi dòng trống kết thúc
                loop {
                    if r.read_line().await?.is_empty() {
                        break;
                    }
                }
                break;
            }
            out.extend(r.read_n(size).await?);
            r.read_n(2).await?; // CRLF sau mỗi chunk
        }
        out
    } else if let Some(len) = headers.get("content-length").and_then(|v| v.parse::<usize>().ok()) {
        r.read_n(len).await?
    } else {
        Vec::new()
    };
    Some(Req { method, target, headers, body, chunked })
}

async fn write_response(sock: &mut TcpStream, resp: Resp) -> std::io::Result<()> {
    let reason = match resp.status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        206 => "Partial Content",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        416 => "Range Not Satisfiable",
        _ => "Status",
    };
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nConnection: close\r\nContent-Length: {}\r\n",
        resp.status,
        reason,
        resp.declared_len.unwrap_or(resp.body.len())
    );
    for (k, v) in &resp.headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    sock.write_all(head.as_bytes()).await?;
    sock.write_all(&resp.body).await?;
    sock.flush().await
}

/// Lấy một tham số trong query string (giải mã `+` và `%XX`).
pub fn query_param(req: &Req, key: &str) -> Option<String> {
    for pair in req.query().split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return Some(percent_decode(v));
            }
        }
    }
    None
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                let hex = std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Thư mục tạm duy nhất cho mỗi test.
pub fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("sadrive-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Parse `bytes a-b/total` -> (a, b, total).
pub fn parse_content_range(v: &str) -> Option<(u64, u64, u64)> {
    let rest = v.strip_prefix("bytes ")?;
    let (range, total) = rest.split_once('/')?;
    let (a, b) = range.split_once('-')?;
    Some((a.parse().ok()?, b.parse().ok()?, total.parse().ok()?))
}
