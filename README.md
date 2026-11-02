# SA Drive

Ứng dụng desktop (Tauri 2 + Svelte 5 + Rust) để quản lý **nhiều Google service account (SA)** và duyệt, tải lên, tải xuống, đổi tên, di chuyển, chia sẻ, xoá file của từng SA như một "Drive mini".

Bố cục theo Google Drive: nút **Mới** và các mục điều hướng ở thanh bên trái; account đang dùng nằm ở góc trên bên phải, bấm vào để xem danh sách account và chuyển nhanh. Giao diện theo **Material Design 3** (xem phần "Giao diện").

---

## Đọc trước: service account không phải Drive cá nhân

Đây là điểm quyết định ứng dụng có dùng được cho trường hợp của bạn hay không.

- File do SA tạo ra thuộc sở hữu của chính SA và tốn dung lượng **My Drive của SA**.
- Từ khoảng 15/04/2025, SA mới tạo có `storageQuota.limit = 0`: không sở hữu được file nào. Tải lên My Drive của SA, hoặc vào thư mục người khác chia sẻ cho SA, đều bị Google trả về `storageQuotaExceeded`. SA tạo trước đó vẫn giữ 15 GB.
- Thanh dung lượng ở góc dưới thanh bên hiện **hạn mức thật mà Google báo** cho từng SA, nên bạn tự kiểm chứng được ngay sau khi thêm account.

> Nguồn: thông báo của Google được trích lại trong [diễn đàn rclone](https://forum.rclone.org/t/google-drive-service-account-changes-and-rclone/50136) và [cộng đồng Dataiku](https://community.dataiku.com/discussion/44746/change-to-google-drive-will-this-impact-the-dataiku-google-drive-plugins): "Starting April 15, 2025 for new Service Accounts", SA mới "will not be able to own any Drive items", SA tạo trước mốc đó giữ dung lượng. Google liệt kê ba lối thay thế: Shared Drive, OAuth thay mặt một người dùng, domain-wide delegation. Tôi chưa tìm được trang tài liệu chính thức của Google nêu điều này, và thông báo tự mâu thuẫn nhẹ về việc mốc là "từ" hay "sau" 15/04/2025 — vì vậy mới ghi "khoảng".

Muốn tải lên được với SA không có dung lượng, chỉ có hai đường:

| Cách | Cần gì | File thuộc về |
| --- | --- | --- |
| **Shared Drive** | Google Workspace (để tạo Shared Drive); thêm email SA vào drive với vai trò Content manager trở lên | Shared Drive, không tốn dung lượng SA |
| **Domain-wide delegation** | Google Workspace + quyền super admin | Người dùng được SA giả danh |

**Tài khoản Gmail cá nhân không dùng được cả hai.** Nếu bạn định quản lý Drive của các tài khoản Gmail cá nhân, service account là công cụ sai; cần đăng nhập OAuth bằng chính tài khoản đó, và **ứng dụng này chưa hỗ trợ OAuth**.

Những việc vẫn hoạt động bình thường kể cả khi SA không có dung lượng: xem, tải xuống, đổi tên, di chuyển và xoá file mà người khác chia sẻ cho SA (tuỳ vai trò được cấp).

---

## Tính năng

- Nhiều account; mỗi account có màu riêng (hiện ở ảnh đại diện, nút Mới, mục đang chọn) để khó nhầm khi xoá. Hộp thoại xoá luôn nêu lại tên và email của account.
- Điều hướng: Drive của SA, Tất cả file của SA, Được chia sẻ với SA, từng Shared Drive, Thùng rác. Tìm theo tên.
- Tải lên file hoặc cả thư mục (nút Mới hoặc kéo thả vào cửa sổ). Tải theo từng khối 8 MiB, tự thử lại có backoff, tiếp tục từ đúng vị trí Google đã nhận khi đứt kết nối.
- Tải xuống có tiếp tục bằng HTTP Range. Google Docs, Sheets, Slides được xuất sang `docx`, `xlsx`, `pptx`; Drawings sang `png`. Trùng tên trên máy thì thêm hậu tố `(1)`, `(2)`, …
- Thư mục mới, đổi tên, di chuyển, chia sẻ (mời theo email hoặc bất kỳ ai có liên kết), thùng rác, khôi phục, xoá vĩnh viễn, dọn thùng rác.
- **Xuất file khoá JSON** (account → Cài đặt → Xuất file khoá JSON → Xuất…) để dùng ở project khác hoặc nhập lại vào ứng dụng. Xem phần Bảo mật.
- Thẻ truyền tải nổi ở góc dưới bên phải: tiến độ, tốc độ, huỷ, thử lại, xoá mục đã xong. Thông báo cũng hiện ở đúng chỗ đó, và nằm ngay phía trên thẻ khi thẻ đang hiện.
- Giao diện sáng/tối theo hệ thống; danh sách file được ảo hoá (chỉ dựng các dòng đang nhìn thấy). Toàn bộ giao diện theo Material Design 3, tông màu lấy từ account đang chọn; thông báo là snackbar. Chọn dòng chỉ đổi màu nền, không có ô tích (giống Drive web).

### Phím tắt

| Phím | Tác dụng |
| --- | --- |
| `Ctrl/Cmd + F` | Vào ô tìm kiếm |
| `F5` hoặc `Ctrl/Cmd + R` | Tải lại |
| `↑` `↓` `PageUp` `PageDown` `Home` `End` | Di chuyển; thêm `Shift` để chọn dải |
| `Ctrl/Cmd + bấm` | Thêm / bớt một dòng khỏi vùng chọn |
| `Shift + bấm` | Chọn cả dải từ dòng bấm gần nhất |
| `Space` | Chọn / bỏ chọn mục đang trỏ |
| `Enter` | Mở thư mục, hoặc **tải xuống** nếu là file |
| `Ctrl/Cmd + A` | Chọn tất cả |
| `Esc` | Bỏ chọn |
| `F2` | Đổi tên |
| `Delete` | Xoá (luôn hỏi xác nhận) |
| `Backspace` hoặc `Alt + ←` | Lên thư mục cha |

---

## Chuẩn bị service account

1. Google Cloud Console → **APIs & Services → Library** → bật **Google Drive API** cho project.
2. **IAM & Admin → Service Accounts → Create service account**.
3. Mở SA vừa tạo → **Keys → Add key → Create new key → JSON**, tải file về.
4. Trong ứng dụng: bấm account ở góc trên bên phải → **Thêm service account** → chọn một hoặc nhiều file JSON (mỗi file tối đa 64 KB), hoặc dán nội dung.
5. Chia sẻ thư mục, hoặc thêm SA vào Shared Drive, bằng đúng email của SA (`tên@project.iam.gserviceaccount.com`). SA **không tự thấy** file trong Drive cá nhân của bạn.

### Domain-wide delegation (Google Workspace)

1. Cloud Console → mở SA → sao chép **Client ID** (dãy số).
2. Admin console → Security → Access and data control → API controls → **Manage domain-wide delegation → Add new**.
3. Dán Client ID, thêm scope `https://www.googleapis.com/auth/drive`, bấm Authorize.
4. Trong ứng dụng: account → Cài đặt → điền email người dùng cần giả danh.

Mọi thao tác khi đó chạy với **toàn quyền Drive của người được giả danh**. Hãy dùng một tài khoản chuyên dụng, không dùng tài khoản cá nhân.

---

## Chạy và build

### Yêu cầu

- Node.js 20.19+ hoặc 22.12+ (đã thử với 22.22) và npm.
- Rust stable **≥ 1.90** (Tauri 2.12 đòi tối thiểu bản này; đã thử với 1.97).
- **Windows**: Microsoft C++ Build Tools và WebView2 (có sẵn trên Windows 10/11 bản mới). TLS dùng `rustls` + `ring` nên **không cần CMake hay NASM**.
- **macOS**: Xcode Command Line Tools.
- **Linux** (Debian/Ubuntu):

  ```bash
  sudo apt install build-essential pkg-config curl wget file \
    libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev libdbus-1-dev \
    libayatana-appindicator3-dev librsvg2-dev
  ```

  Muốn lưu master key vào keychain trên Linux cần chạy một Secret Service (GNOME Keyring hoặc KWallet). Nếu không có, ứng dụng rơi về file `master.key` và cảnh báo ở thanh bên (xem phần Bảo mật).

### Lệnh

```bash
npm install

npm run tauri dev      # chạy ứng dụng thật, tự nạp lại giao diện
npm run tauri build    # đóng gói bản cài đặt (msi/nsis, dmg, deb/rpm/AppImage)

npm run check          # svelte-check + TypeScript
npm run dev            # chỉ giao diện, mở http://localhost:1420 trong trình duyệt, dùng backend GIẢ
cd src-tauri && cargo test --lib   # 50 test của backend Rust
```

`npm run dev` mở bằng trình duyệt thường sẽ nạp `src/mock.ts` (dữ liệu giả, chỉ có ở chế độ dev, không lọt vào bản build). Dùng để chỉnh giao diện; không kết nối Google.

Đổi file trong `src-tauri/icons/` mà `npm run tauri dev` vẫn hiện icon cũ là chuyện bình thường: Tauri đọc icon lúc biên dịch nhưng Cargo không theo dõi các file đó, nên chạy `touch src-tauri/src/lib.rs` (nơi gọi `generate_context!`) rồi chạy lại. Riêng `icon.icns` của macOS phải dựng từ `source-macos-1024.png` (thân icon 80% khung), không phải từ `source-1024.png`; Dock còn nhớ icon cũ một lúc, mở lại ứng dụng hoặc chạy `killall Dock` nếu cần.

---

## Bảo mật

- Khoá JSON được mã hoá **AES-256-GCM** trong `vault.bin` (quyền `0600` trên Unix). Master key ngẫu nhiên nằm trong keychain của hệ điều hành: Credential Manager (Windows), Keychain (macOS), Secret Service (Linux).
- Nếu keychain không dùng được, master key rơi về file `master.key` cạnh `vault.bin` (quyền `0600`). Cách này **yếu hơn**: ai đọc được thư mục dữ liệu là đọc được cả hai. Ứng dụng cảnh báo ở thanh bên khi đang ở chế độ này.
- Thư mục dữ liệu theo quy ước của Tauri với identifier `dev.sadrive.app`: `~/.local/share/dev.sadrive.app` (Linux), `~/Library/Application Support/dev.sadrive.app` (macOS), `%APPDATA%\dev.sadrive.app` (Windows).
- Ứng dụng chỉ gửi khoá tới `oauth2.googleapis.com` để lấy token và chỉ gọi `www.googleapis.com`. Không có telemetry. CSP của webview chặt: `default-src 'self'` và `script-src 'self'`.
- **Xoá account khỏi ứng dụng chỉ xoá khoá trên máy này.** Khoá vẫn còn hiệu lực trên Google. Nếu nghi bị lộ, hãy xoá khoá trong Cloud Console.
- **Xuất khoá ghi private key ra file văn bản thường**, nên đây là chỗ duy nhất khoá rời khỏi kho mã hoá, và chỉ khi bạn bấm. Hộp thoại lưu do phía Rust mở bằng hộp thoại gốc của hệ điều hành; giao diện không chọn được đường dẫn và không bao giờ nhận được nội dung khoá. File ghi với quyền `0600` trên Linux/macOS (kể cả khi đè lên file cũ quyền rộng hơn); trên Windows quyền do thư mục đích quyết định, nên hãy lưu trong thư mục người dùng. Mỗi bản sao là thêm một chỗ có thể bị lộ: nếu project khác cần khoá riêng, tạo khoá mới trong Cloud Console sẽ thu hồi được từng cái.
- File xuất ra được **dựng lại theo định dạng chuẩn của Google**, không phải bản sao từng byte của file bạn đã nhập, vì kho chỉ giữ `type`, `project_id`, `private_key_id`, `private_key`, `client_email`, `client_id` (sáu trường này giống file gốc từng ký tự). Các trường còn lại (`auth_uri`, `token_uri`, `auth_provider_x509_cert_url`, `client_x509_cert_url`, `universe_domain`) lấy giá trị chuẩn. File đọc được bởi thư viện `google-auth`, gcloud, rclone và nhập lại được vào ứng dụng này. Tên hiển thị, màu và email giả danh là cài đặt riêng của ứng dụng nên không nằm trong file.
- Quyền của cửa sổ chỉ gồm `core:default`, `dialog:default`, `opener:default`; không cấp quyền đọc/ghi file tuỳ ý cho phía giao diện. Việc ghi file khoá xuất ra do lệnh Rust `account_export_key` làm, không phải do giao diện.

---

## Hành vi cần biết

- **Nhấp đúp vào icon hoặc tên (hay `Enter` khi dòng đang được trỏ) sẽ mở thư mục, hoặc tải file xuống ngay**, không có xem trước. Hai chỗ đó hiện con trỏ pointer và nhấp đơn vào đó **không chọn** dòng; muốn chọn thì bấm vào phần còn lại của dòng (chỗ trống cạnh tên, các cột khác). `Ctrl`/`Cmd`/`Shift` + bấm vào tên vẫn chọn như thường. Thư mục tải về được hỏi lần đầu rồi nhớ lại; muốn chọn thư mục khác thì nhấn chuột phải → **Tải xuống vào…**.
- **Không có ô tích: dòng được chọn chỉ đổi màu nền**, như Drive web. Chọn một dòng thì bấm vào dòng; thêm hoặc bớt từng dòng bằng `Ctrl`/`Cmd` + bấm; chọn cả dải bằng `Shift` + bấm; `Space` chọn hoặc bỏ dòng đang trỏ; `Ctrl/Cmd + A` chọn tất cả; `Esc` hoặc bấm vào vùng trống dưới bảng để bỏ chọn. Cái giá phải trả: chỉ dùng chuột thì không chọn được nhiều dòng rời nhau, và không còn nút "chọn tất cả" ở tiêu đề cột. Nền dòng được chọn chỉ hơn nền thường 1,3:1 (sáng) hoặc 1,8:1 (tối), thấp hơn mức 3:1 mà WCAG đòi cho dấu hiệu trạng thái, nên người kém thị lực khó nhận ra hơn so với khi có ô tích. Bù lại có thanh "Đã chọn N mục" (bằng chữ), `aria-selected` trên từng dòng cho trình đọc màn hình và viền focus 3px quanh dòng đang trỏ khi dùng bàn phím.
- Khi có mục được chọn, thanh thao tác (Tải xuống, Đổi tên, …) thay chỗ tiêu đề trang thay vì chèn thêm một hàng, nên bảng không bị đẩy xuống. Cửa sổ dưới khoảng 1040 px thì thanh này chỉ còn icon (rê chuột để xem tên).
- Thông báo nằm ở góc dưới bên phải, cùng mép phải với thẻ truyền tải, bề rộng ôm theo chữ (từ 344 đến 480 px); thu gọn hay mở thẻ thì thông báo đi theo, thẻ biến mất thì thông báo về lại đúng góc. Thông báo nằm **trên** cả hộp thoại, nên khi một hộp thoại cao đang mở (Cài đặt, Chia sẻ, và hầu hết hộp thoại ở cửa sổ nhỏ) nó có thể che nút ở chân hộp thoại cho tới khi tự tắt (4,5 giây, lỗi thì 14 giây); bấm × để tắt ngay. Vì snackbar của Material 3 không có biến thể màu, thông báo thành công, thông tin và lỗi trông giống nhau; chỉ lỗi dùng vai trò `alert` và ở lại 14 giây. Khác chuẩn một chỗ: Material 3 chỉ hiện một snackbar mỗi lần, ở đây tối đa 4 cái xếp chồng để lỗi không phải chờ sau thông báo thường.
- Menu chuột phải mặc định của webview (Reload, Inspect…) đã tắt. Còn lại ở ô nhập chữ, để vẫn dán khoá JSON bằng chuột phải được; menu tự vẽ trong bảng file không đổi. Khi chạy `npm run tauri dev`, giữ `Shift` + chuột phải để mở lại menu gốc (Inspect Element); bản build không có lối này. Muốn tắt cả ở ô nhập chữ thì trong `src/main.ts` đổi `if (!isTextField(e.target)) e.preventDefault();` thành `e.preventDefault();`.
- Tải lên **không kiểm tra trùng tên**: Drive cho phép nhiều file cùng tên, nên tải hai lần sẽ ra hai bản.
- Khi tải lên thư mục, **symlink và file đặc biệt bị bỏ qua** (ứng dụng báo số mục đã bỏ qua).
- Màn hình **Thùng rác chỉ liệt kê file của My Drive của SA**, không gồm Shared Drive: file đã bỏ vào thùng rác trong Shared Drive phải khôi phục bằng giao diện Google Drive. Di chuyển chỉ thực hiện trong cùng một drive (giới hạn của Drive API).
- Hạn mức tải lên của Google là 750 GB mỗi ngày cho mỗi người dùng.
- Xoá vĩnh viễn không thể hoàn tác; hộp thoại xoá mặc định chọn "Chuyển vào thùng rác".

---

## Giao diện: Material Design 3

Token của Material Design 3 là nguồn duy nhất cho màu, chữ, bo góc, độ nổi và chuyển động; `styles.css` và `m3.css` chỉ tham chiếu token, không viết cứng màu hay độ bo góc (trừ vài chỗ nêu bên dưới).

- `src/tokens.css`: màu (`--md-sys-color-*`; màu mở rộng `--md-extended-color-*` cho thành công, cảnh báo và loại file), 15 kiểu chữ (`--md-sys-typescale-*`), hình dạng (`--md-sys-shape-corner-*`), độ nổi level 0–5, chuyển động (easing, duration), mức state layer (rê chuột 8%, focus/nhấn 10%) và vòng focus 3px. Có đủ bộ token của M3, nên một số vai trò chưa chỗ nào dùng.
- Màu theo account: `colors.ts` tính các vai trò `primary`, `secondary`, `*-container`, `inverse-primary` từ màu của account (OKLCH, tự chỉnh để đủ tương phản) rồi ghi lên `<html>`; các vai trò trung tính và `error` cố định trong `tokens.css`. Sáng/tối theo `prefers-color-scheme`.
- Thành phần: nút filled / tonal / outlined / text và nút icon, extended FAB (nút Mới), thẻ viền, menu, hộp thoại, snackbar, tab, ô nhập viền và select, checkbox, radio, danh sách, vòng tiến trình, thanh tiến độ. State layer làm bằng lớp phủ `currentColor` (không dùng `color-mix`, vì webview cũ không có).
- Biểu tượng: Material Symbols (Outlined, weight 400), nhúng dạng path trong `src/icons.ts`. Giấy phép Apache-2.0, bản sao và ghi chú nguồn ở `third_party/material-symbols/`.

Những chỗ **cố ý khác chuẩn** (nói rõ để khỏi nhầm là sót):

| Chỗ | Chuẩn M3 | Ở đây | Lý do |
| --- | --- | --- | --- |
| Phông chữ | Roboto / Roboto Flex | phông hệ thống (Segoe UI, SF, Noto Sans…) | không kèm file font; muốn Roboto thì thêm `@font-face` và đổi `--md-ref-typeface-plain` |
| Mật độ | mục nav 56px, thanh tìm 56px, dòng danh sách 56px, menu 48px | nav 36px, thanh tìm 46px, dòng bảng 40px, menu 36px, nút dense 32px | giữ bố cục gọn như trước, không thiết kế lại |
| Màu trung tính | sinh từ màu hạt giống | cố định trong `tokens.css`; chỉ primary, secondary và container đổi theo account | nền xám giống nhau ở mọi account, không ngả màu |
| Vai trò màu | có tertiary | không dùng tertiary | không có chỗ nào cần |
| Vạch giữa các dòng bảng | `outline-variant` | tông `surface-container-high` | `outline-variant` lặp trên mọi dòng làm bảng thành lưới ô |
| Tooltip | tooltip của M3 | thuộc tính `title` của trình duyệt | không dựng thêm thành phần |
| Chuyển động | có hiệu ứng vào và ra | chỉ có hiệu ứng vào (menu, hộp thoại, thẻ nổi) | |
| Snackbar | một cái mỗi lần | tối đa 4 cái xếp chồng | để lỗi không phải chờ sau thông báo thường |
| Chữ mã (khoá, email, `code`) | không có kiểu monospace | cỡ cố định 12–14px | M3 không định nghĩa kiểu này |

Hệ quả của việc theo thang chữ M3: chữ nội dung là 14/20 thay vì 13px như trước, nên một số khối cao hơn vài px và cột ngày phải rộng thêm (150px). Cột ngày, kích thước và chủ sở hữu có độ rộng cố định; với phông hệ thống rộng hơn phông thử nghiệm (ví dụ Segoe UI ở cỡ chữ hiển thị lớn), chữ có thể bị cắt bằng dấu "…".

---

## Cấu trúc dự án

```
src/                     giao diện Svelte 5 (runes) + TypeScript
  App.svelte             trạng thái account, theme, kéo thả
  Workspace.svelte       danh sách file, thao tác, hộp thoại
  components/            TopBar, Sidebar, AccountMenu, FileTable, TransferPanel, ...; Modal, Button, TextField (M3)
  dialogs/               thêm account, cài đặt, xoá, chia sẻ, di chuyển, hướng dẫn, ...
  tokens.css             token Material Design 3: màu, chữ, hình dạng, độ nổi, chuyển động, state layer
  styles.css             giao diện chính (thanh trên, thanh bên, bảng file, thẻ truyền tải, menu), chỉ dùng token
  m3.css                 thành phần M3 dùng chung: nút, ô nhập, hộp thoại, danh sách, snackbar
  icons.ts               biểu tượng Material Symbols (đường vẽ SVG); nguồn và giấy phép ở third_party/
  colors.ts              màu account và các vai trò màu M3 tính từ đó (OKLCH, đủ tương phản)
  api.ts                 lớp gọi lệnh Tauri (+ mock khi chạy trong trình duyệt ở dev)
src-tauri/src/
  vault.rs               mã hoá AES-256-GCM, keychain
  auth.rs                JWT bearer (RS256), token cache, domain-wide delegation
  drive.rs               Drive API v3, retry/backoff, lỗi kèm gợi ý tiếng Việt
  upload.rs download.rs  truyền tải có tiếp tục
  transfer.rs            hàng đợi, huỷ, thử lại, sự kiện tiến độ
  commands.rs            28 lệnh IPC
third_party/
  material-symbols/      giấy phép Apache-2.0 và ghi chú nguồn của bộ biểu tượng
src-tauri/icons/
  source-macos-1024.png  nguồn của icon.icns: thân icon 824/1024 px theo lưới macOS, nếu để tràn khung thì Dock hiện to hơn app khác
  source-1024.png        nguồn của các file png và ico (Windows, Linux)
```

---

## Trạng thái kiểm thử

Đã làm:

- 50 test Rust, chạy với **máy chủ HTTP giả** trong tiến trình: đổi token, phân trang, tải lên theo khối và tiếp tục khi đứt, tải xuống có Range, lỗi quota, lệch đồng hồ, vault mã hoá, quyền file, xuất khoá rồi nhập lại.
- `svelte-check` không có lỗi hay cảnh báo; build production thành công.
- 376 kiểm tra hành vi giao diện bằng Playwright với backend giả (chọn dòng không ô tích bằng chuột và bàn phím, nhấp đúp vào icon/tên, phím tắt, xoá, tải lên, tìm kiếm, chuyển account, thùng rác, cài đặt và xuất khoá, vị trí và kiểu snackbar, menu chuột phải nằm trọn trong cửa sổ, tắt menu chuột phải kể cả trên bản build production; nhiều cỡ cửa sổ, sáng và tối), cộng kiểm tra tương phản trên DOM thật: hơn 18.000 phép đo chữ, icon của nút và cặp màu token ở 20 trạng thái (trang chủ, đang chọn, menu, thẻ truyền tải, snackbar, từng hộp thoại, màn hình chào…) × 8 màu account × sáng/tối; chữ ≥ 4,5:1, icon của nút và viền ≥ 3:1. Các script này chạy ngoài dự án và **không kèm theo** trong gói.
- Chạy thật binary Tauri trong môi trường ảo: thêm account bằng cách dán JSON, vault không chứa văn bản thường, cảnh báo khoá yếu, hiển thị lỗi mạng thật; hộp thoại Cài đặt và Thêm account hiển thị đúng trên WebKitGTK ở chế độ sáng và tối; xuất khoá qua hộp thoại lưu GTK thật, file ra quyền `0600`, thư viện `google-auth` 2.62 đọc được và ký được JWT assertion (chưa đổi lấy token thật vì môi trường không tới được Google).

**Chưa làm được:** gọi **Google API thật** (môi trường phát triển không tới được `googleapis.com`), keychain thật của hệ điều hành, webview trên Windows và macOS (nên chưa thấy được phông chữ và cách vẽ thật ở đó). Mã gọi Google được viết theo tài liệu Drive API v3 và kiểm tra với máy chủ giả, nên lần chạy đầu với account thật có thể lộ ra khác biệt mà test không bắt được. Nếu gặp lỗi, thông báo trong ứng dụng kèm gợi ý; hãy gửi lại nguyên văn lỗi.

---

## Khắc phục sự cố

| Triệu chứng | Nguyên nhân thường gặp |
| --- | --- |
| `storageQuotaExceeded` khi tải lên | SA không có dung lượng. Xem phần "Đọc trước" ở trên: dùng Shared Drive hoặc domain-wide delegation. |
| `accessNotConfigured` / `SERVICE_DISABLED` | Chưa bật Google Drive API cho project của SA. |
| `invalid_grant` kèm "signature" | Khoá JSON đã bị xoá hoặc vô hiệu hoá trong Cloud Console; tạo khoá mới. |
| `invalid_grant` kèm "iat" / "exp" / "short-lived" | Đồng hồ máy lệch giờ thực; bật đồng bộ thời gian tự động. |
| `invalid_grant` kèm "email" / "user" | Email giả danh sai hoặc không thuộc domain đã cấp delegation. |
| Danh sách trống dù SA đã tạo | SA không thấy Drive cá nhân của bạn; hãy chia sẻ thư mục cho email của SA. |
| `userRateLimitExceeded` | Vượt tần suất; thử lại sau vài phút. |
| Thanh bên cảnh báo "khoá yếu" (Linux) | Không có Secret Service; cài và mở GNOME Keyring hoặc KWallet, rồi thêm lại account. |
