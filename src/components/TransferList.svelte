<script lang="ts">
  import type { AccountInfo, TransferInfo } from "../types";
  import { endIndex, startIndex } from "../virtual";
  import TransferRow from "./TransferRow.svelte";

  const ROW_H = 62;
  const MAX_H = 248;

  let { items, byId }: { items: TransferInfo[]; byId: Map<string, AccountInfo> } = $props();

  let scroller: HTMLDivElement;
  let scrollTop = $state(0);
  let viewH = $state(MAX_H);

  const height = $derived(Math.min(MAX_H, items.length * ROW_H));
  const start = $derived(startIndex(scrollTop, ROW_H, 4));
  const end = $derived(endIndex(items.length, scrollTop, viewH, ROW_H, 0, 4));
  const rows = $derived(items.slice(start, end));
</script>

<div bind:this={scroller} bind:clientHeight={viewH} class="xfer-list" style:height="{height}px" role="list" onscroll={() => (scrollTop = scroller.scrollTop)}>
  <div class="xfer-body" style:height="{items.length * ROW_H}px">
    {#each rows as t, k (t.id)}
      <TransferRow {t} y={(start + k) * ROW_H} account={byId.get(t.accountId)} />
    {/each}
  </div>
</div>
