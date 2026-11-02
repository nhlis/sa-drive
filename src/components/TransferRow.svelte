<script lang="ts">
  import { revealPath } from "../api";
  import { accountColor } from "../colors";
  import { formatBytes, formatEta, formatSpeed } from "../format";
  import { theme } from "../theme.svelte";
  import { transfers } from "../transfers.svelte";
  import type { AccountInfo, TransferInfo, TransferState } from "../types";
  import Icon from "./Icon.svelte";
  import IconButton from "./IconButton.svelte";

  const STATE_LABEL: Record<TransferState, string> = {
    queued: "Đang chờ",
    running: "Đang chạy",
    done: "Xong",
    error: "Lỗi",
    cancelled: "Đã huỷ",
  };

  let { t, y, account }: { t: TransferInfo; y: number; account?: AccountInfo } = $props();

  const pct = $derived(t.total > 0 ? Math.min(100, (t.done / t.total) * 100) : t.state === "done" ? 100 : 0);
  const bps = $derived(t.state === "running" ? transfers.speedOf(t.id) : 0);
  const eta = $derived(bps > 0 ? formatEta((t.total - t.done) / bps) : "");
  const live = $derived(t.state === "running" || t.state === "queued");
  const showMsg = $derived(t.state === "error" || (t.state === "cancelled" && !!t.message));
  const progressText = $derived.by(() => {
    if (t.state === "queued") return "Đang chờ";
    if (t.state === "cancelled") return "Đã huỷ";
    let s = t.total > 0 ? `${formatBytes(t.done)} / ${formatBytes(t.total)}` : formatBytes(t.done);
    if (t.state === "running" && bps > 0) s += ` · ${formatSpeed(bps)}`;
    if (eta) s += ` · còn ${eta}`;
    return s;
  });
</script>

<!-- Chiều cao dòng (62px) đặt trong CSS (.xrow) và phải khớp với ROW_H của TransferList. -->
<div role="listitem" class="xrow {t.state}" style:transform="translateY({y}px)" style:--c={account ? accountColor(account.color, theme.dark) : undefined}>
  <Icon name={t.kind === "upload" ? "arrowUp" : "arrowDown"} size={16} class="xrow-kind" />
  <div class="xrow-main">
    <div class="xrow-top">
      <span class="xrow-name" title={t.name}>{t.name}</span>
      {#if account}
        <span class="xrow-acct" title={account.clientEmail}>{account.label}</span>
      {/if}
    </div>
    {#if showMsg}
      <div class="xrow-msg" title={t.message ?? undefined}>{t.message ?? STATE_LABEL[t.state]}</div>
    {:else}
      <div class="xrow-bar">
        <div class="bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(pct)} aria-label="Tiến độ {t.name}">
          <span style:width="{pct}%"></span>
        </div>
        <span class="xrow-num num">{progressText}</span>
      </div>
    {/if}
  </div>
  <div class="xrow-act">
    {#if live}
      <IconButton small icon="close" label="Huỷ {t.name}" title="Huỷ" onclick={() => transfers.cancel(t.id)} />
    {/if}
    {#if t.state === "error" || t.state === "cancelled"}
      <IconButton small icon="refresh" label="Thử lại {t.name}" title="Thử lại" onclick={() => void transfers.retry(t.id).catch(() => {})} />
    {/if}
    {#if t.state === "done" && t.kind === "download" && t.localPath}
      <IconButton small icon="folder" label="Hiện trong thư mục" onclick={() => void revealPath(t.localPath!).catch(() => {})} />
    {/if}
    {#if t.state === "done"}<Icon name="check" size={16} class="xrow-ok" />{/if}
  </div>
</div>
