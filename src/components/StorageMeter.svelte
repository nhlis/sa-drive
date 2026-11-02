<script lang="ts">
  import { formatBytes } from "../format";
  import type { About } from "../types";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    about: About | null;
    loading: boolean;
    error: { message: string } | null;
    onretry: () => void;
    onhelp: () => void;
  }

  let { about, loading, error, onretry, onhelp }: Props = $props();

  const q = $derived(about?.storageQuota ?? null);
  const pct = $derived(q && q.limit ? Math.min(100, Math.max(0, ((q.usage ?? 0) / q.limit) * 100)) : 0);
</script>

<!-- Dung lượng thật của service account đang chọn (không phải của project Google Cloud). -->
<div class="storage" aria-live="polite">
  {#if loading && !about}
    <span class="skeleton" style:width="100%" style:height="4px"></span>
    <span class="skeleton" style:width="70%"></span>
  {:else if error && !about}
    <div class="storage-err" title={error.message}>
      <Icon name="alert" size={15} />
      <span>Không đọc được dung lượng.</span>
      <button class="link" onclick={onretry}>Thử lại</button>
    </div>
  {:else if q}
    {#if q.limit === 0}
      <div class="meter hazard" role="img" aria-label="Hạn mức lưu trữ bằng 0"></div>
      <p class="storage-text">
        <strong>Không có dung lượng riêng.</strong> Chỉ tải lên được Shared Drive hoặc khi giả danh một người dùng.
      </p>
      <Button variant="outlined" size="compact" onclick={onhelp}>Cách khắc phục</Button>
    {:else if q.limit == null}
      <p class="storage-text">Đã dùng <span class="num">{formatBytes(q.usage)}</span>, không giới hạn.</p>
    {:else}
      <div class="meter" role="meter" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(pct)} aria-label="Dung lượng đã dùng">
        <span style:width="{pct}%"></span>
      </div>
      <p class="storage-text">
        Đã dùng <span class="num">{formatBytes(q.usage)}</span> trong tổng số <span class="num">{formatBytes(q.limit)}</span>
      </p>
    {/if}
  {/if}
</div>
