import { mount } from "svelte";
import App from "./App.svelte";
import "./tokens.css";
import "./styles.css";
import "./m3.css";

// Ô nhập chữ: giữ menu gốc của hệ điều hành (Cắt, Sao chép, Dán…) để vẫn dán được khoá JSON bằng chuột phải.
const TEXT_INPUT = /^(text|search|url|tel|email|password|number)?$/i;

function isTextField(t: EventTarget | null): boolean {
  if (!(t instanceof HTMLElement) || t.closest(":disabled")) return false;
  if (t.isContentEditable || t instanceof HTMLTextAreaElement) return true;
  return t instanceof HTMLInputElement && TEXT_INPUT.test(t.type);
}

// Tắt menu chuột phải mặc định của webview (Reload, Inspect…) cho cả ứng dụng.
// Menu tự vẽ trong bảng file (FileTable) vẫn hoạt động vì nó tự gọi preventDefault.
// Khi chạy dev, giữ Shift + chuột phải để mở menu gốc (Inspect Element).
document.addEventListener(
  "contextmenu",
  (e) => {
    if (import.meta.env.DEV && e.shiftKey) return;
    if (!isTextField(e.target)) e.preventDefault();
  },
  { capture: true },
);

mount(App, { target: document.getElementById("root")! });
