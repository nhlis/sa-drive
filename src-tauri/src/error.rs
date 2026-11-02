//! Kiểu lỗi chung của ứng dụng. Được serialize thành `{ code, message, hint }`
//! để giao diện hiển thị thông báo rõ ràng kèm gợi ý cách khắc phục.

use serde::{Serialize, Serializer};

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Msg(String),
    #[error("Lỗi đọc/ghi file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Lỗi mạng: {0}")]
    Network(String),
    #[error("Dữ liệu JSON không hợp lệ: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Google Drive báo lỗi {status}: {message}")]
    Drive {
        status: u16,
        reason: String,
        message: String,
    },
    #[error("Không lấy được access token ({error}): {description}")]
    Auth { error: String, description: String },
    #[error("Kho khoá bảo mật: {0}")]
    Vault(String),
    #[error("Không giải mã được kho khoá: {0}")]
    VaultCorrupt(String),
    #[error("Đã huỷ")]
    Cancelled,
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        // without_url: tránh lộ URL kèm tham số trong thông báo lỗi
        AppError::Network(e.without_url().to_string())
    }
}

impl AppError {
    pub fn msg(s: impl Into<String>) -> Self {
        AppError::Msg(s.into())
    }

    /// Mã ngắn để giao diện phân nhánh xử lý.
    pub fn code(&self) -> String {
        match self {
            AppError::Msg(_) => "error".into(),
            AppError::Io(_) => "io".into(),
            AppError::Network(_) => "network".into(),
            AppError::Json(_) => "json".into(),
            AppError::Drive { reason, .. } => format!("drive:{reason}"),
            AppError::Auth { error, .. } => format!("auth:{error}"),
            AppError::Vault(_) => "vault".into(),
            AppError::VaultCorrupt(_) => "vault_corrupt".into(),
            AppError::Cancelled => "cancelled".into(),
        }
    }

    /// Lỗi tạm thời, đáng thử lại.
    pub fn is_transient(&self) -> bool {
        match self {
            AppError::Network(_) => true,
            AppError::Drive { status, reason, .. } => {
                matches!(*status, 408 | 429 | 500 | 502 | 503 | 504) || is_rate_limit_reason(reason)
            }
            _ => false,
        }
    }

    pub fn hint(&self) -> Option<String> {
        match self {
            AppError::Drive {
                status,
                reason,
                message,
            } => drive_hint(*status, reason, message),
            AppError::Auth { error, description } => auth_hint(error, description),
            AppError::Network(_) => Some(
                "Kiểm tra kết nối Internet hoặc proxy/tường lửa. Ứng dụng cần truy cập oauth2.googleapis.com và www.googleapis.com.".into(),
            ),
            AppError::VaultCorrupt(_) => Some(
                "Master key trong keychain của hệ điều hành đã mất hoặc không khớp với kho khoá. Bạn có thể đặt lại kho (file cũ được giữ lại dưới dạng .bak) rồi nhập lại các file JSON.".into(),
            ),
            _ => None,
        }
    }

    /// Thông báo đầy đủ (message + gợi ý) dùng cho dòng trạng thái của transfer.
    pub fn display_full(&self) -> String {
        match self.hint() {
            Some(h) => format!("{self}. {h}"),
            None => self.to_string(),
        }
    }
}

pub fn is_rate_limit_reason(reason: &str) -> bool {
    matches!(
        reason,
        "userRateLimitExceeded" | "rateLimitExceeded" | "sharingRateLimitExceeded" | "backendError"
    )
}

fn drive_hint(status: u16, reason: &str, message: &str) -> Option<String> {
    let lower = message.to_lowercase();
    // Google dùng nhiều tên khác nhau cho cùng một tình huống "chưa bật Drive API".
    if matches!(reason, "accessNotConfigured" | "SERVICE_DISABLED")
        || lower.contains("has not been used in project")
        || lower.contains("api has not been used")
    {
        return Some("Google Drive API chưa được bật trong project của service account này. Mở Cloud Console → APIs & Services → Library → Google Drive API → Enable, rồi đợi vài phút.".into());
    }
    let s = match reason {
        "storageQuotaExceeded" => "Service account này không có dung lượng lưu trữ riêng (hoặc đã đầy). SA tạo sau 15/04/2025 không sở hữu được file: hãy tải lên trong một Shared Drive mà SA là thành viên, hoặc dùng domain-wide delegation. Với SA cũ, hãy xoá bớt file do SA sở hữu.",
        "notFound" => "Không tìm thấy mục này, hoặc service account chưa được chia sẻ quyền truy cập. Hãy chia sẻ thư mục / thêm SA vào Shared Drive bằng đúng email của SA.",
        "forbidden" | "insufficientFilePermissions" | "insufficientPermissions" | "appNotAuthorizedToFile" => "Service account không đủ quyền với mục này (cần vai trò Editor / Content manager để tải lên, sửa hoặc xoá).",
        "fileNotDownloadable" => "File này không tải trực tiếp được (ví dụ Google Docs/Sheets chỉ xuất được sang định dạng khác).",
        "exportSizeLimitExceeded" => "File quá lớn để xuất (Google giới hạn 10 MB cho việc xuất Docs/Sheets/Slides).",
        "cannotDownloadAbusiveFile" => "Google đã đánh dấu file này là có hại nên không cho tải xuống.",
        "teamDriveFileLimitExceeded" | "numChildrenInNonRootLimitExceeded" | "numItemsLimitExceeded" => "Shared Drive hoặc thư mục đã đạt giới hạn số mục của Google.",
        "domainPolicy" => "Chính sách quản trị Workspace của tổ chức đang chặn thao tác này (ví dụ chia sẻ ra ngoài domain).",
        "invalidSharingRequest" | "sharingRateLimitExceeded" => "Google từ chối yêu cầu chia sẻ này (email không hợp lệ, không có tài khoản Google, hoặc bị giới hạn tần suất).",
        "userRateLimitExceeded" | "rateLimitExceeded" | "dailyLimitExceeded" => "Vượt giới hạn tần suất / hạn mức của Google. Thử lại sau ít phút; hạn mức tải lên là 750 GB mỗi ngày cho mỗi người dùng.",
        _ => {
            return match status {
                401 => Some("Token bị từ chối. Thử lại; nếu vẫn lỗi, hãy kiểm tra khoá JSON còn hiệu lực trong Cloud Console.".into()),
                _ => None,
            }
        }
    };
    Some(s.into())
}

fn auth_hint(error: &str, description: &str) -> Option<String> {
    let d = description.to_lowercase();
    let s = match error {
        "invalid_grant" if d.contains("signature") => "Khoá JSON không còn hiệu lực (đã bị xoá hoặc vô hiệu hoá trong Cloud Console). Hãy tạo khoá mới và nhập lại.",
        "invalid_grant" if d.contains("short-lived") || d.contains("timeframe") || d.contains("iat") || d.contains("exp") => "Đồng hồ của máy lệch so với giờ thực. Hãy bật đồng bộ thời gian tự động rồi thử lại.",
        "invalid_grant" if d.contains("email") || d.contains("user") => "Email giả danh (domain-wide delegation) không hợp lệ hoặc không thuộc domain Workspace đã cấp quyền cho SA.",
        "invalid_grant" => "Google từ chối khoá này. Kiểm tra service account còn tồn tại và khoá chưa bị xoá; nếu dùng domain-wide delegation thì kiểm tra email giả danh.",
        "unauthorized_client" => "Client ID của SA chưa được cấp domain-wide delegation cho scope https://www.googleapis.com/auth/drive. Vào Admin console → Security → API controls → Domain-wide delegation để thêm.",
        "invalid_scope" => "Scope bị từ chối. Với domain-wide delegation, phải cấp đúng scope https://www.googleapis.com/auth/drive trong Admin console.",
        "access_denied" | "disabled_client" => "Service account đã bị vô hiệu hoá hoặc không được phép lấy token.",
        _ => return None,
    };
    Some(s.into())
}

#[derive(Serialize)]
struct Payload {
    code: String,
    message: String,
    hint: Option<String>,
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        Payload {
            code: self.code(),
            message: self.to_string(),
            hint: self.hint(),
        }
        .serialize(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_error_has_vietnamese_hint() {
        let e = AppError::Drive {
            status: 403,
            reason: "storageQuotaExceeded".into(),
            message: "Service Accounts do not have storage quota.".into(),
        };
        assert_eq!(e.code(), "drive:storageQuotaExceeded");
        assert!(e.hint().unwrap().contains("Shared Drive"));
        assert!(!e.is_transient());
    }

    #[test]
    fn disabled_api_is_detected_by_message() {
        let e = AppError::Drive {
            status: 403,
            reason: "forbidden".into(),
            message: "Google Drive API has not been used in project 123 before or it is disabled.".into(),
        };
        assert!(e.hint().unwrap().contains("Enable"));
    }

    #[test]
    fn rate_limits_and_5xx_are_transient() {
        let rl = AppError::Drive { status: 403, reason: "userRateLimitExceeded".into(), message: String::new() };
        let be = AppError::Drive { status: 503, reason: "backendError".into(), message: String::new() };
        let nf = AppError::Drive { status: 404, reason: "notFound".into(), message: String::new() };
        assert!(rl.is_transient() && be.is_transient() && !nf.is_transient());
    }

    #[test]
    fn serializes_to_payload() {
        let v = serde_json::to_value(AppError::Cancelled).unwrap();
        assert_eq!(v["code"], "cancelled");
        assert_eq!(v["message"], "Đã huỷ");
    }

    #[test]
    fn clock_skew_hint() {
        let e = AppError::Auth {
            error: "invalid_grant".into(),
            description: "Invalid JWT: Token must be a short-lived token (60 minutes) and in a reasonable timeframe.".into(),
        };
        // Hint bắt đầu bằng "Đồng hồ" (viết hoa) nên so sánh không phân biệt hoa/thường.
        assert!(e.hint().unwrap().to_lowercase().contains("đồng hồ"));
    }

    #[test]
    fn revoked_key_is_not_mistaken_for_clock_skew() {
        let e = AppError::Auth {
            error: "invalid_grant".into(),
            description: "Invalid JWT Signature.".into(),
        };
        let h = e.hint().unwrap().to_lowercase();
        assert!(h.contains("khoá json") && !h.contains("đồng hồ"));
    }
}
