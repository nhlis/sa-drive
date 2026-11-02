//! Xác thực service account: ký JWT (RS256) và đổi lấy access token (OAuth2 jwt-bearer).

use std::fmt;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

pub const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive";

// Giá trị chuẩn của các trường mà kho không lưu (xem `ServiceAccountKey::to_key_file`).
const AUTH_URI: &str = "https://accounts.google.com/o/oauth2/auth";
const CERTS_URL: &str = "https://www.googleapis.com/oauth2/v1/certs";
const ROBOT_METADATA_URL: &str = "https://www.googleapis.com/robot/v1/metadata/x509/";

#[derive(Clone, Serialize, Deserialize)]
pub struct ServiceAccountKey {
    #[serde(rename = "type")]
    pub key_type: String,
    pub project_id: String,
    pub private_key_id: String,
    pub private_key: String,
    pub client_email: String,
    #[serde(default)]
    pub client_id: String,
}

// Không bao giờ in private_key ra log/debug.
impl fmt::Debug for ServiceAccountKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceAccountKey")
            .field("client_email", &self.client_email)
            .field("project_id", &self.project_id)
            .field("private_key_id", &self.private_key_id)
            .field("private_key", &"<redacted>")
            .finish()
    }
}

impl ServiceAccountKey {
    /// Đọc và kiểm tra file khoá JSON tải từ Cloud Console.
    pub fn parse(json: &str) -> Result<Self> {
        let raw: serde_json::Value = serde_json::from_str(json.trim_start_matches('\u{feff}'))
            .map_err(|e| AppError::msg(format!("Nội dung không phải JSON hợp lệ: {e}")))?;
        let obj = raw
            .as_object()
            .ok_or_else(|| AppError::msg("Nội dung JSON phải là một object."))?;

        if obj.get("type").and_then(|v| v.as_str()) != Some("service_account") {
            let looks_oauth = obj.contains_key("installed") || obj.contains_key("web");
            return Err(AppError::msg(if looks_oauth {
                "Đây là file OAuth client (client_secret_*.json), không phải khoá service account. Hãy tạo khoá ở IAM & Admin → Service Accounts → Keys → Add key → JSON."
            } else {
                "Thiếu \"type\": \"service_account\". Hãy chọn file khoá JSON của service account."
            }));
        }
        // Chỉ chấp nhận endpoint token chuẩn của Google: chặn file khoá "độc" bắt app gửi
        // yêu cầu đến máy chủ khác.
        if let Some(uri) = obj.get("token_uri").and_then(|v| v.as_str()) {
            if uri != TOKEN_URL {
                return Err(AppError::msg(format!(
                    "token_uri không được hỗ trợ ({uri}). Chỉ chấp nhận {TOKEN_URL}."
                )));
            }
        }

        let key: ServiceAccountKey = serde_json::from_value(raw)
            .map_err(|e| AppError::msg(format!("Khoá thiếu trường bắt buộc: {e}")))?;
        if !key.client_email.contains('@') {
            return Err(AppError::msg("client_email không hợp lệ."));
        }
        // Kiểm tra khoá ký được ngay lúc nhập, thay vì chờ đến lần gọi API đầu tiên.
        EncodingKey::from_rsa_pem(key.private_key.as_bytes()).map_err(|e| {
            AppError::msg(format!(
                "private_key không đọc được ({e}). File có thể bị sửa hoặc cắt mất."
            ))
        })?;
        Ok(key)
    }

    /// Dựng file khoá JSON theo định dạng chuẩn mà Cloud Console cấp, để gcloud, rclone, các thư viện
    /// Google và chính ứng dụng này (Thêm service account) đọc lại được.
    ///
    /// Kho chỉ giữ 6 trường nên đây KHÔNG phải bản sao từng byte của file đã nhập: các trường còn lại
    /// (`auth_uri`, `token_uri`, ...) được điền giá trị chuẩn. `token_uri` luôn là `TOKEN_URL`,
    /// đúng thứ mà `parse` chấp nhận.
    pub fn to_key_file(&self) -> Result<String> {
        #[derive(Serialize)]
        struct KeyFile<'a> {
            #[serde(rename = "type")]
            key_type: &'a str,
            project_id: &'a str,
            private_key_id: &'a str,
            private_key: &'a str,
            client_email: &'a str,
            #[serde(skip_serializing_if = "is_blank")]
            client_id: &'a str,
            auth_uri: &'a str,
            token_uri: &'a str,
            auth_provider_x509_cert_url: &'a str,
            client_x509_cert_url: String,
            universe_domain: &'a str,
        }
        fn is_blank(s: &&str) -> bool {
            s.is_empty()
        }

        let file = KeyFile {
            key_type: "service_account",
            project_id: &self.project_id,
            private_key_id: &self.private_key_id,
            private_key: &self.private_key,
            client_email: &self.client_email,
            client_id: &self.client_id,
            auth_uri: AUTH_URI,
            token_uri: TOKEN_URL,
            auth_provider_x509_cert_url: CERTS_URL,
            client_x509_cert_url: format!("{ROBOT_METADATA_URL}{}", percent_encode(&self.client_email)),
            universe_domain: "googleapis.com",
        };
        let mut text = serde_json::to_string_pretty(&file)?;
        text.push('\n');
        Ok(text)
    }

    /// Tên file gợi ý trong hộp thoại lưu: `<phần trước @>-<8 ký tự đầu của key id>.json`.
    pub fn suggested_file_name(&self) -> String {
        let local = self.client_email.split('@').next().unwrap_or("service-account");
        let kid: String = self.private_key_id.chars().filter(char::is_ascii_alphanumeric).take(8).collect();
        let name = if kid.is_empty() { format!("{local}.json") } else { format!("{local}-{kid}.json") };
        crate::fsutil::sanitize_filename(&name)
    }
}

/// Mã hoá phần trăm theo RFC 3986 (giữ nguyên ký tự unreserved); đủ cho một email đặt trong đường dẫn URL.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[derive(Serialize)]
struct Claims<'a> {
    iss: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    sub: Option<&'a str>,
    scope: &'a str,
    aud: &'a str,
    iat: u64,
    exp: u64,
}

/// Tạo JWT assertion. `now` truyền vào để test được.
pub fn build_assertion(key: &ServiceAccountKey, subject: Option<&str>, now: u64) -> Result<String> {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(key.private_key_id.clone());
    let claims = Claims {
        iss: &key.client_email,
        sub: subject,
        scope: DRIVE_SCOPE,
        aud: TOKEN_URL,
        // Lùi iat 30 giây để chịu được đồng hồ máy nhanh nhẹ; exp < iat + 3600.
        iat: now.saturating_sub(30),
        exp: now + 3300,
    };
    let enc = EncodingKey::from_rsa_pem(key.private_key.as_bytes())
        .map_err(|e| AppError::msg(format!("private_key không hợp lệ: {e}")))?;
    jsonwebtoken::encode(&header, &claims, &enc)
        .map_err(|e| AppError::msg(format!("Không ký được JWT: {e}")))
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: u64,
}

#[derive(Deserialize)]
struct TokenError {
    #[serde(default)]
    error: String,
    #[serde(default)]
    error_description: String,
}

struct Cached {
    token: String,
    expires_at: Instant,
}

enum Kind {
    ServiceAccount {
        key: ServiceAccountKey,
        subject: Option<String>,
    },
    #[cfg(test)]
    Static(String),
}

/// Nguồn access token cho một account. Có cache; chỉ một request làm mới token tại một thời điểm.
pub struct TokenSource {
    kind: Kind,
    http: reqwest::Client,
    token_url: String,
    cache: tokio::sync::Mutex<Option<Cached>>,
}

impl TokenSource {
    pub fn new(key: ServiceAccountKey, subject: Option<String>, http: reqwest::Client) -> Self {
        let subject = subject.filter(|s| !s.trim().is_empty());
        Self {
            kind: Kind::ServiceAccount { key, subject },
            http,
            token_url: TOKEN_URL.to_string(),
            cache: tokio::sync::Mutex::new(None),
        }
    }

    #[cfg(test)]
    pub fn static_token(token: &str) -> Self {
        Self {
            kind: Kind::Static(token.to_string()),
            http: reqwest::Client::new(),
            token_url: String::new(),
            cache: tokio::sync::Mutex::new(None),
        }
    }

    #[cfg(test)]
    pub fn with_token_url(mut self, url: &str) -> Self {
        self.token_url = url.to_string();
        self
    }

    pub async fn token(&self) -> Result<String> {
        #[cfg(test)]
        if let Kind::Static(t) = &self.kind {
            return Ok(t.clone());
        }
        let mut guard = self.cache.lock().await;
        if let Some(c) = guard.as_ref() {
            if c.expires_at > Instant::now() + Duration::from_secs(60) {
                return Ok(c.token.clone());
            }
        }
        let fresh = self.fetch().await?;
        let token = fresh.token.clone();
        *guard = Some(fresh);
        Ok(token)
    }

    /// Bỏ token đang cache (khi Google trả 401).
    pub async fn invalidate(&self) {
        *self.cache.lock().await = None;
    }

    async fn fetch(&self) -> Result<Cached> {
        let (key, subject) = match &self.kind {
            Kind::ServiceAccount { key, subject } => (key, subject),
            #[cfg(test)]
            Kind::Static(_) => unreachable!(),
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| AppError::msg(format!("Đồng hồ hệ thống không hợp lệ: {e}")))?
            .as_secs();
        let assertion = build_assertion(key, subject.as_deref(), now)?;

        let resp = self
            .http
            .post(&self.token_url)
            .timeout(Duration::from_secs(30))
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", assertion.as_str()),
            ])
            .send()
            .await?;

        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            let e: TokenError = serde_json::from_str(&body).unwrap_or(TokenError {
                error: format!("http_{}", status.as_u16()),
                error_description: body.chars().take(300).collect(),
            });
            return Err(AppError::Auth {
                error: e.error,
                description: e.error_description,
            });
        }
        let t: TokenResponse = serde_json::from_str(&body)?;
        Ok(Cached {
            token: t.access_token,
            expires_at: Instant::now() + Duration::from_secs(t.expires_in.max(120)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    /// Sinh khoá RSA bằng openssl CLI (nếu máy có) để test ký JWT mà không phải commit khoá mẫu.
    fn gen_rsa() -> Option<(String, String)> {
        let dir = std::env::temp_dir().join(format!("sadrive-rsa-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).ok()?;
        let priv_path = dir.join("k.pem");
        let ok = std::process::Command::new("openssl")
            .args(["genpkey", "-algorithm", "RSA", "-pkeyopt", "rsa_keygen_bits:2048", "-out"])
            .arg(&priv_path)
            .output()
            .ok()?
            .status
            .success();
        if !ok {
            return None;
        }
        let pubkey = std::process::Command::new("openssl")
            .args(["pkey", "-in"])
            .arg(&priv_path)
            .args(["-pubout"])
            .output()
            .ok()?;
        let priv_pem = std::fs::read_to_string(&priv_path).ok()?;
        let pub_pem = String::from_utf8(pubkey.stdout).ok()?;
        std::fs::remove_dir_all(dir).ok();
        Some((priv_pem, pub_pem))
    }

    pub fn sample_key_json(private_key: &str) -> String {
        serde_json::json!({
            "type": "service_account",
            "project_id": "demo-project",
            "private_key_id": "abc123",
            "private_key": private_key,
            "client_email": "bot@demo-project.iam.gserviceaccount.com",
            "client_id": "1234567890",
            "token_uri": TOKEN_URL
        })
        .to_string()
    }

    #[test]
    fn jwt_is_signed_and_verifiable() {
        let Some((priv_pem, pub_pem)) = gen_rsa() else {
            eprintln!("bỏ qua: không có openssl");
            return;
        };
        let key = ServiceAccountKey::parse(&sample_key_json(&priv_pem)).unwrap();
        let jwt = build_assertion(&key, Some("user@example.com"), 1_800_000_000).unwrap();

        let mut v = jsonwebtoken::Validation::new(Algorithm::RS256);
        v.set_audience(&[TOKEN_URL]);
        v.validate_exp = false;
        let data = jsonwebtoken::decode::<serde_json::Value>(
            &jwt,
            &jsonwebtoken::DecodingKey::from_rsa_pem(pub_pem.as_bytes()).unwrap(),
            &v,
        )
        .expect("chữ ký phải hợp lệ");
        let c = data.claims;
        assert_eq!(c["iss"], "bot@demo-project.iam.gserviceaccount.com");
        assert_eq!(c["sub"], "user@example.com");
        assert_eq!(c["scope"], DRIVE_SCOPE);
        assert!(c["exp"].as_u64().unwrap() - c["iat"].as_u64().unwrap() <= 3600);
        assert_eq!(data.header.kid.as_deref(), Some("abc123"));

        // không có subject -> không có claim sub
        let jwt2 = build_assertion(&key, None, 1_800_000_000).unwrap();
        let payload = jwt2.split('.').nth(1).unwrap();
        let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        assert!(v.get("sub").is_none());
    }

    #[test]
    fn rejects_non_service_account_files() {
        let oauth = r#"{"installed":{"client_id":"x","client_secret":"y"}}"#;
        let e = ServiceAccountKey::parse(oauth).unwrap_err().to_string();
        assert!(e.contains("OAuth client"), "{e}");
        assert!(ServiceAccountKey::parse("không phải json").is_err());
        assert!(ServiceAccountKey::parse("[]").is_err());
    }

    #[test]
    fn exported_key_file_is_standard_and_can_be_imported_back() {
        let Some((priv_pem, _)) = gen_rsa() else {
            eprintln!("bỏ qua: không có openssl");
            return;
        };
        let key = ServiceAccountKey::parse(&sample_key_json(&priv_pem)).unwrap();
        let text = key.to_key_file().unwrap();

        assert!(text.starts_with("{\n  \"type\": \"service_account\","), "{text}");
        assert!(text.ends_with("}\n"));
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        for k in [
            "type",
            "project_id",
            "private_key_id",
            "private_key",
            "client_email",
            "client_id",
            "auth_uri",
            "token_uri",
            "auth_provider_x509_cert_url",
            "client_x509_cert_url",
            "universe_domain",
        ] {
            assert!(v.get(k).and_then(|x| x.as_str()).is_some_and(|s| !s.is_empty()), "thiếu trường {k}");
        }
        assert_eq!(v["token_uri"], TOKEN_URL);
        assert_eq!(
            v["client_x509_cert_url"],
            "https://www.googleapis.com/robot/v1/metadata/x509/bot%40demo-project.iam.gserviceaccount.com"
        );

        // Đọc lại bằng chính bộ phân tích của ứng dụng: ra đúng khoá cũ và vẫn ký được JWT.
        let back = ServiceAccountKey::parse(&text).unwrap();
        assert_eq!(back.private_key, key.private_key);
        assert_eq!(back.private_key_id, key.private_key_id);
        assert_eq!(back.client_email, key.client_email);
        assert_eq!(back.project_id, key.project_id);
        assert_eq!(back.client_id, key.client_id);
        build_assertion(&back, None, 1_800_000_000).unwrap();
    }

    #[test]
    fn exported_key_file_edge_cases() {
        // Không cần openssl: to_key_file không kiểm tra khoá.
        let key = ServiceAccountKey {
            key_type: "service_account".into(),
            project_id: "p".into(),
            private_key_id: String::new(),
            private_key: "PEM\nXYZ\n".into(),
            client_email: "a+b@p.iam.gserviceaccount.com".into(),
            client_id: String::new(),
        };
        let v: serde_json::Value = serde_json::from_str(&key.to_key_file().unwrap()).unwrap();
        assert!(v.get("client_id").is_none(), "client_id rỗng thì bỏ hẳn trường");
        assert_eq!(v["private_key"], "PEM\nXYZ\n", "xuống dòng trong PEM phải đi qua JSON nguyên vẹn");
        assert!(v["client_x509_cert_url"].as_str().unwrap().ends_with("/a%2Bb%40p.iam.gserviceaccount.com"));
        assert_eq!(key.suggested_file_name(), "a+b.json");

        let mut k2 = key.clone();
        k2.private_key_id = "0123456789abcdef".into();
        k2.client_email = "../../evil@p.iam.gserviceaccount.com".into();
        let n = k2.suggested_file_name();
        assert!(!n.contains('/') && !n.contains('\\'), "tên file gợi ý không được chứa dấu phân cách: {n}");
        assert!(n.ends_with("-01234567.json"), "{n}");
    }

    #[test]
    fn rejects_foreign_token_uri_and_bad_pem() {
        let j = r#"{"type":"service_account","project_id":"p","private_key_id":"k","private_key":"x","client_email":"a@b.c","token_uri":"https://evil.example/token"}"#;
        assert!(ServiceAccountKey::parse(j).unwrap_err().to_string().contains("token_uri"));
        let j2 = r#"{"type":"service_account","project_id":"p","private_key_id":"k","private_key":"không phải PEM","client_email":"a@b.c"}"#;
        assert!(ServiceAccountKey::parse(j2).unwrap_err().to_string().contains("private_key"));
    }

    #[tokio::test]
    async fn exchanges_jwt_for_token_caches_it_and_maps_errors() {
        use crate::testutil::{spawn, Resp};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let Some((priv_pem, pub_pem)) = gen_rsa() else {
            eprintln!("bỏ qua: không có openssl");
            return;
        };
        let key = ServiceAccountKey::parse(&sample_key_json(&priv_pem)).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let c2 = calls.clone();
        let mock = spawn(move |req, _| {
            let n = c2.fetch_add(1, Ordering::SeqCst);
            assert_eq!(req.method, "POST");
            assert_eq!(req.header("content-type"), Some("application/x-www-form-urlencoded"));
            let body = String::from_utf8(req.body.clone()).unwrap();
            assert!(body.contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer"), "{body}");
            let jwt = body.split('&').find_map(|p| p.strip_prefix("assertion=")).expect("thiếu assertion");
            // chữ ký phải kiểm chứng được bằng khoá công khai
            let mut v = jsonwebtoken::Validation::new(Algorithm::RS256);
            v.set_audience(&[TOKEN_URL]);
            let data = jsonwebtoken::decode::<serde_json::Value>(
                jwt,
                &jsonwebtoken::DecodingKey::from_rsa_pem(pub_pem.as_bytes()).unwrap(),
                &v,
            )
            .expect("JWT gửi đi phải hợp lệ");
            assert_eq!(data.claims["sub"], "boss@corp.example");
            if n == 0 {
                Resp::json(200, r#"{"access_token":"ya29.TOKEN","expires_in":3599,"token_type":"Bearer"}"#)
            } else {
                Resp::json(400, r#"{"error":"invalid_grant","error_description":"Invalid JWT Signature."}"#)
            }
        })
        .await;

        let client = crate::drive::DriveApi::new().unwrap().token_client();
        let ts = TokenSource::new(key, Some("boss@corp.example".into()), client).with_token_url(&mock.base);
        assert_eq!(ts.token().await.unwrap(), "ya29.TOKEN");
        assert_eq!(ts.token().await.unwrap(), "ya29.TOKEN");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "token còn hạn phải được cache, không gọi lại");

        ts.invalidate().await;
        let err = ts.token().await.unwrap_err();
        assert_eq!(err.code(), "auth:invalid_grant");
        assert!(err.hint().unwrap().contains("Khoá JSON"), "{:?}", err.hint());
    }

    #[test]
    fn debug_never_prints_private_key() {
        let Some((priv_pem, _)) = gen_rsa() else { return };
        let key = ServiceAccountKey::parse(&sample_key_json(&priv_pem)).unwrap();
        let dbg = format!("{key:?}");
        assert!(!dbg.contains("BEGIN PRIVATE KEY"));
        assert!(dbg.contains("redacted"));
    }
}
