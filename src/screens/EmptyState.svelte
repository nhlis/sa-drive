<script lang="ts">
  import Button from "../components/Button.svelte";
  import Icon from "../components/Icon.svelte";
  import type { HelpSection } from "../types";

  let { onadd, onhelp }: { onadd: () => void; onhelp: (section?: HelpSection) => void } = $props();
</script>

<main class="welcome">
  <div class="welcome-card">
    <h1>Thêm service account đầu tiên</h1>
    <p>
      Nhập file khoá JSON của service account để duyệt, tải lên, tải xuống và xoá file trên Google Drive mà SA đó truy cập được. Bạn có thể thêm
      nhiều account; mỗi account có một màu riêng để không nhầm khi thao tác.
    </p>
    <div class="welcome-actions">
      <Button variant="filled" icon="plus" onclick={onadd}>Chọn file khoá JSON</Button>
      <Button variant="outlined" onclick={() => onhelp("setup")}>Cách lấy file khoá</Button>
    </div>
    <div class="welcome-note" role="note">
      <Icon name="alert" size={20} />
      <p>
        Service account tạo từ khoảng 15/04/2025 không có dung lượng My Drive riêng, nên chỉ tải lên được vào Shared Drive hoặc khi dùng
        domain-wide delegation. Xem, tải xuống và xoá file được chia sẻ cho SA vẫn hoạt động bình thường.
        <button class="link" onclick={() => onhelp("quota")}>Chi tiết</button>
      </p>
    </div>
  </div>
</main>
