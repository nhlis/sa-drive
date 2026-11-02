<script lang="ts">
  import { toasts } from "../toasts.svelte";
  import IconButton from "./IconButton.svelte";
</script>

<!-- Snackbar của Material 3: không có biến thể màu theo loại, nên ok/info/err chỉ khác vai trò đọc màn hình. -->
<div class="toasts" role="region" aria-label="Thông báo" aria-live="polite">
  {#each toasts.items as t (t.id)}
    <div class="md-snackbar" role={t.kind === "err" ? "alert" : "status"}>
      <div class="md-snackbar-text">
        <div>{t.text}</div>
        {#if t.hint}<div>{t.hint}</div>{/if}
      </div>
      <IconButton icon="close" label="Đóng thông báo" onclick={() => toasts.dismiss(t.id)} />
    </div>
  {/each}
</div>
