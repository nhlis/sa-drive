<script lang="ts">
  import { formatSpeed, plural } from "../format";
  import { transfers } from "../transfers.svelte";
  import type { AccountInfo, TransferInfo, TransferState } from "../types";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import TransferList from "./TransferList.svelte";

  const RANK: Record<TransferState, number> = { running: 0, queued: 1, error: 2, cancelled: 3, done: 4 };

  function order(a: TransferInfo, b: TransferInfo): number {
    const r = RANK[a.state] - RANK[b.state];
    if (r !== 0) return r;
    // Hàng chờ theo thứ tự sẽ chạy; còn lại mới nhất lên trước.
    return a.state === "running" || a.state === "queued" ? a.createdAt - b.createdAt : b.createdAt - a.createdAt;
  }

  let { accounts }: { accounts: AccountInfo[] } = $props();

  /** Khoảng cách giữa thẻ và thông báo ngay phía trên, bằng khoảng cách giữa các thông báo (.toasts). */
  const GAP = 8;

  let open = $state(false);
  let prevCount = 0;
  let boxH = $state(0);

  // Mở panel khi có truyền tải mới được thêm vào.
  $effect(() => {
    const n = transfers.items.length;
    if (n > prevCount) open = true;
    prevCount = n;
  });

  // Thông báo xếp ngay trên thẻ này nên CSS cần biết thẻ đang cao bao nhiêu (mở, thu gọn hay không còn).
  $effect(() => {
    const px = transfers.items.length > 0 ? boxH + GAP : 0;
    document.documentElement.style.setProperty("--xfer-offset", `${px}px`);
  });
  $effect(() => () => document.documentElement.style.removeProperty("--xfer-offset"));

  const sorted = $derived(transfers.items.slice().sort(order));
  const stats = $derived.by(() => {
    const s = { running: 0, queued: 0, error: 0, done: 0, cancelled: 0, total: 0, doneBytes: 0, speed: 0 };
    for (const t of transfers.items) {
      s[t.state]++;
      if (t.state === "running" || t.state === "queued") {
        s.total += t.total;
        s.doneBytes += t.done;
      }
      if (t.state === "running") s.speed += transfers.speedOf(t.id);
    }
    return s;
  });
  const active = $derived(stats.running + stats.queued);
  const finished = $derived(stats.done + stats.error + stats.cancelled);
  const pct = $derived(stats.total > 0 ? Math.min(100, (stats.doneBytes / stats.total) * 100) : 0);
  const byId = $derived(new Map(accounts.map((a) => [a.id, a] as const)));
  const title = $derived.by(() => {
    if (active > 0) return `Đang truyền ${plural(active, "mục")}`;
    if (stats.error > 0) return `${plural(stats.error, "mục")} bị lỗi`;
    if (stats.done > 0) return `Đã xong ${plural(stats.done, "mục")}`;
    return `Đã huỷ ${plural(stats.cancelled, "mục")}`;
  });
</script>

{#if transfers.items.length > 0}
  <section class="xfer" class:open aria-label="Truyền tải" bind:offsetHeight={boxH}>
    <div class="xfer-head">
      <button class="xfer-toggle" onclick={() => (open = !open)} aria-expanded={open}>
        <span class="xfer-title">{title}</span>
        {#if stats.error > 0 && active > 0}<span class="xfer-flag">{stats.error} lỗi</span>{/if}
        {#if stats.running > 0 && stats.speed > 0}<span class="xfer-speed num">{formatSpeed(stats.speed)}</span>{/if}
        <Icon name={open ? "chevronDown" : "chevronUp"} size={18} />
      </button>
      {#if active > 0}
        <div class="xfer-total" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(pct)} aria-label="Tiến độ tổng">
          <span style:width="{pct}%"></span>
        </div>
      {/if}
    </div>
    {#if open}
      <div class="xfer-tools">
        {#if active > 0}
          <Button size="dense" onclick={transfers.cancelAll}>Huỷ tất cả</Button>
        {/if}
        {#if finished > 0}
          <Button size="dense" onclick={() => void transfers.clearFinished()}>Xoá mục đã xong</Button>
        {/if}
      </div>
      <TransferList items={sorted} {byId} />
    {/if}
  </section>
{/if}
