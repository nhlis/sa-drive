// Lưu tuỳ chọn nhẹ của từng người dùng (tài khoản đang chọn, cách sắp xếp, thư mục tải xuống…).
// localStorage có thể trống hoặc ném lỗi (chế độ riêng tư, bị chặn) nên mọi thao tác đều bọc try/catch.

const PREFIX = "sadrive.";

export function getPref<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(PREFIX + key);
    return raw == null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}

export function setPref(key: string, value: unknown): void {
  try {
    localStorage.setItem(PREFIX + key, JSON.stringify(value));
  } catch {
    /* bỏ qua: chỉ là tiện ích */
  }
}
