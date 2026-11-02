//! Tiện ích tên file / đường dẫn cục bộ. Tên file lấy từ Drive là dữ liệu không đáng tin:
//! Drive cho phép tên chứa `/`, `..`, ký tự cấm của Windows… nên luôn phải làm sạch
//! trước khi ghi xuống đĩa (chống path traversal).

use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

const MAX_NAME_BYTES: usize = 180;

pub fn sanitize_filename(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();

    // Windows không cho phép tên kết thúc bằng dấu chấm hoặc khoảng trắng.
    s = s.trim().trim_end_matches(['.', ' ']).to_string();
    if s.is_empty() || s == "." || s == ".." {
        s = "_".into();
    }

    // Tên thiết bị dành riêng của Windows (CON, NUL, COM1…), kể cả khi có đuôi file.
    let stem = s.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0');
    if reserved {
        s.insert(0, '_');
    }

    // Giới hạn độ dài (byte) nhưng giữ phần mở rộng.
    if s.len() > MAX_NAME_BYTES {
        let (stem, ext) = match s.rfind('.') {
            Some(i) if s.len() - i <= 16 => (s[..i].to_string(), s[i..].to_string()),
            _ => (s.clone(), String::new()),
        };
        let mut keep = String::new();
        for c in stem.chars() {
            if keep.len() + c.len_utf8() + ext.len() > MAX_NAME_BYTES {
                break;
            }
            keep.push(c);
        }
        s = format!("{keep}{ext}");
    }
    s
}

fn candidate(dir: &Path, name: &str, n: u32) -> PathBuf {
    if n == 0 {
        return dir.join(name);
    }
    match name.rfind('.') {
        Some(i) if i > 0 => dir.join(format!("{} ({}){}", &name[..i], n, &name[i..])),
        _ => dir.join(format!("{name} ({n})")),
    }
}

/// Tạo (đặt chỗ) một file rỗng với tên chưa tồn tại: `a.txt`, `a (1).txt`, …
/// `create_new` đảm bảo hai tác vụ song song không chọn trùng cùng một tên.
pub fn reserve_unique_file(dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    for n in 0..10_000u32 {
        let p = candidate(dir, name, n);
        match OpenOptions::new().write(true).create_new(true).open(&p) {
            Ok(_) => return Ok(p),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "không tìm được tên file chưa dùng",
    ))
}

/// Tương tự cho thư mục.
pub fn reserve_unique_dir(parent: &Path, name: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(parent)?;
    for n in 0..10_000u32 {
        let p = candidate(parent, name, n);
        match std::fs::create_dir(&p) {
            Ok(()) => return Ok(p),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "không tìm được tên thư mục chưa dùng",
    ))
}

/// Ghi `data` ra `path` sao cho chỉ chủ sở hữu đọc/ghi được (0600 trên Unix), kể cả khi đè lên một
/// file đã có đang ở quyền rộng hơn (ví dụ 0644). Dùng cho thứ nhạy cảm như khoá service account.
///
/// Ghi vào file tạm cùng thư mục (tạo mới với quyền 0600 ngay từ đầu) rồi `rename` đè lên đích:
/// không để lại file nửa chừng khi lỗi, và không có khoảnh khắc nào khoá nằm trong file quyền rộng.
/// Nếu `path` là symlink thì chính symlink bị thay, nội dung không bị ghi xuyên qua nó.
/// Trên Windows quyền do ACL của thư mục đích quyết định.
pub fn write_private_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    if path.file_name().is_none() {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "đường dẫn không có tên file"));
    }
    // Tên file tạm cố định độ dài, không dựa vào tên đích (tên đích dài gần giới hạn 255 byte sẽ làm hỏng).
    let tmp = dir.join(format!(".sadrive-write-{}.tmp", uuid::Uuid::new_v4().simple()));

    let result = (|| {
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn mode_of(p: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p).unwrap().permissions().mode() & 0o777
    }

    #[test]
    #[cfg(unix)]
    fn private_write_is_0600_even_when_overwriting_a_wider_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("sadrive-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let target = dir.join("key.json");
        std::fs::write(&target, "cũ").unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o644)).unwrap();
        write_private_atomic(&target, b"bi mat").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"bi mat");
        assert_eq!(mode_of(&target), 0o600, "đè lên file 0644 vẫn phải ra 0600");

        let fresh = dir.join("moi.json");
        write_private_atomic(&fresh, b"x").unwrap();
        assert_eq!(mode_of(&fresh), 0o600);

        let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        names.sort();
        assert_eq!(names, ["key.json", "moi.json"], "không được để sót file tạm");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn private_write_handles_long_names_and_fails_cleanly() {
        let dir = std::env::temp_dir().join(format!("sadrive-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let long = dir.join(format!("{}.json", "a".repeat(200)));
        write_private_atomic(&long, b"ok").unwrap();
        assert_eq!(std::fs::read(&long).unwrap(), b"ok");

        // thư mục đích không tồn tại: trả lỗi, không panic
        assert!(write_private_atomic(&dir.join("khong-co").join("k.json"), b"x").is_err());
        // đích là một thư mục: trả lỗi và không để lại file tạm
        let sub = dir.join("thu-muc");
        std::fs::create_dir(&sub).unwrap();
        assert!(write_private_atomic(&sub, b"x").is_err());
        let leftovers = std::fs::read_dir(&dir).unwrap().filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().starts_with(".sadrive-write-")).count();
        assert_eq!(leftovers, 0);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn blocks_path_traversal() {
        assert_eq!(sanitize_filename("../../etc/passwd"), ".._.._etc_passwd");
        assert_eq!(sanitize_filename(".."), "_");
        assert_eq!(sanitize_filename("."), "_");
        assert_eq!(sanitize_filename(""), "_");
        assert!(!sanitize_filename("a/b\\c").contains(['/', '\\']));
    }

    #[test]
    fn handles_windows_rules() {
        assert_eq!(sanitize_filename("CON"), "_CON");
        assert_eq!(sanitize_filename("nul.txt"), "_nul.txt");
        assert_eq!(sanitize_filename("COM1.log"), "_COM1.log");
        assert_eq!(sanitize_filename("COM0"), "COM0");
        assert_eq!(sanitize_filename("report. "), "report");
        assert_eq!(sanitize_filename("a<b>:c|d?.txt"), "a_b__c_d_.txt");
    }

    #[test]
    fn keeps_vietnamese_and_extension_when_truncating() {
        assert_eq!(sanitize_filename("Báo cáo tháng 10.xlsx"), "Báo cáo tháng 10.xlsx");
        let long = format!("{}.pdf", "đ".repeat(300));
        let out = sanitize_filename(&long);
        assert!(out.len() <= MAX_NAME_BYTES);
        assert!(out.ends_with(".pdf"));
        assert!(out.is_char_boundary(out.len()));
    }

    #[test]
    fn reserves_unique_names() {
        let dir = std::env::temp_dir().join(format!("sadrive-test-{}", uuid::Uuid::new_v4()));
        let a = reserve_unique_file(&dir, "a.txt").unwrap();
        let b = reserve_unique_file(&dir, "a.txt").unwrap();
        let c = reserve_unique_file(&dir, "noext").unwrap();
        let d = reserve_unique_file(&dir, "noext").unwrap();
        assert_eq!(a.file_name().unwrap(), "a.txt");
        assert_eq!(b.file_name().unwrap(), "a (1).txt");
        assert_eq!(c.file_name().unwrap(), "noext");
        assert_eq!(d.file_name().unwrap(), "noext (1)");
        let f1 = reserve_unique_dir(&dir, "Thư mục").unwrap();
        let f2 = reserve_unique_dir(&dir, "Thư mục").unwrap();
        assert_ne!(f1, f2);
        std::fs::remove_dir_all(dir).ok();
    }
}
