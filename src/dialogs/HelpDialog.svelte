<script lang="ts">
  import { onMount } from "svelte";
  import Button from "../components/Button.svelte";
  import Modal from "../components/Modal.svelte";
  import type { HelpSection } from "../types";

  let { section, onclose }: { section?: HelpSection; onclose: () => void } = $props();

  onMount(() => {
    if (section) document.getElementById(`help-${section}`)?.scrollIntoView({ block: "start" });
  });
</script>

<Modal title="Hướng dẫn và giới hạn" {onclose} width={560}>
  <div class="prose">
    <section id="help-quota">
      <h3>Vì sao tải lên bị Google từ chối?</h3>
      <p>
        File do service account (SA) tạo ra luôn thuộc sở hữu của chính SA đó và tốn dung lượng My Drive của SA. Từ khoảng 15/04/2025, SA mới
        tạo không còn được cấp dung lượng này (<code>storageQuota.limit = 0</code>). Vì vậy mọi lần tải lên My Drive của SA, hoặc vào thư mục
        do người khác chia sẻ, đều bị Google trả về lỗi <code>storageQuotaExceeded</code>. SA tạo trước thời điểm đó vẫn giữ 15 GB. Dải màu ở
        đầu cửa sổ hiện hạn mức thật mà Google báo cho từng SA.
      </p>
      <p>Có hai cách để tải lên hoạt động với SA không có dung lượng:</p>
      <ul>
        <li>
          <strong>Shared Drive.</strong> Thêm email của SA vào Shared Drive với vai trò Content manager trở lên. File thuộc về Shared Drive nên
          không tốn dung lượng của SA. Tạo Shared Drive cần Google Workspace.
        </li>
        <li>
          <strong>Domain-wide delegation.</strong> SA giả danh một người dùng trong domain Workspace; file thuộc về người đó và dùng dung lượng
          của họ (xem mục bên dưới).
        </li>
      </ul>
      <p>Xem, tải xuống, đổi tên, di chuyển và xoá file mà người khác chia sẻ cho SA vẫn dùng bình thường.</p>
      <p class="callout">
        Tài khoản Gmail cá nhân không tạo được Shared Drive và không dùng được delegation. Với trường hợp đó service account không phải công cụ
        phù hợp để lưu file; cần đăng nhập OAuth bằng chính tài khoản Google, và ứng dụng này chưa hỗ trợ.
      </p>
    </section>

    <section id="help-setup">
      <h3>Chuẩn bị service account</h3>
      <ol>
        <li>Trong Google Cloud Console, bật Google Drive API cho project (APIs &amp; Services → Library).</li>
        <li>IAM &amp; Admin → Service Accounts → Create service account.</li>
        <li>Mở service account vừa tạo → Keys → Add key → Create new key → JSON, rồi tải file về.</li>
        <li>Trong ứng dụng, bấm Thêm service account và chọn file JSON đó.</li>
        <li>
          Chia sẻ thư mục, hoặc thêm SA vào Shared Drive, bằng đúng email của SA (dạng <code>tên@project.iam.gserviceaccount.com</code>). SA
          không tự thấy file trong Drive cá nhân của bạn.
        </li>
      </ol>
    </section>

    <section id="help-dwd">
      <h3>Domain-wide delegation (Google Workspace)</h3>
      <ol>
        <li>Trong Cloud Console, mở service account và sao chép Client ID (dãy số).</li>
        <li>Admin console → Security → Access and data control → API controls → Manage domain-wide delegation → Add new.</li>
        <li>Dán Client ID, thêm scope <code>https://www.googleapis.com/auth/drive</code>, rồi bấm Authorize.</li>
        <li>Trong ứng dụng, mở Cài đặt của SA và điền email người dùng cần giả danh.</li>
      </ol>
      <p class="callout">
        Cần quyền super admin. Mọi thao tác sẽ chạy với toàn quyền Drive của người dùng được giả danh, nên hãy dùng một tài khoản chuyên dụng thay
        vì tài khoản cá nhân.
      </p>
    </section>

    <section id="help-security">
      <h3>Khoá được lưu như thế nào</h3>
      <ul>
        <li>
          Khoá JSON được mã hoá bằng AES-256-GCM trong file <code>vault.bin</code>. Master key ngẫu nhiên nằm trong keychain của hệ điều hành
          (Credential Manager trên Windows, Keychain trên macOS, Secret Service trên Linux).
        </li>
        <li>
          Nếu keychain không dùng được, master key phải nằm trong file <code>master.key</code> cạnh <code>vault.bin</code> (quyền 0600). Cách này
          yếu hơn vì ai đọc được thư mục dữ liệu là đọc được cả hai; ứng dụng sẽ cảnh báo ở thanh bên.
        </li>
        <li>Ứng dụng chỉ gửi khoá tới <code>oauth2.googleapis.com</code> để lấy token và chỉ gọi <code>www.googleapis.com</code>.</li>
        <li>
          Cài đặt account có mục <strong>Xuất file khoá JSON</strong>: ghi khoá ra file JSON văn bản thường (quyền 0600 trên Linux và macOS) để dùng ở
          project khác hoặc nhập lại vào đây. Đây là lúc khoá rời khỏi kho mã hoá, nên hãy lưu ở nơi tin cậy. File được dựng lại theo định dạng chuẩn
          của Google nên không giống từng byte với file gốc.
        </li>
        <li>
          Xoá account khỏi ứng dụng chỉ xoá khoá trên máy này. Khoá vẫn còn hiệu lực trên Google; nếu nghi ngờ bị lộ, hãy xoá khoá trong Cloud
          Console.
        </li>
      </ul>
    </section>
  </div>

  {#snippet footer()}
    <Button onclick={onclose}>Đóng</Button>
  {/snippet}
</Modal>
