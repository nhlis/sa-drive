<script lang="ts">
  import type { Snippet } from "svelte";
  import { formatBytes, formatDate, isFolder, ownerLabel } from "../format";
  import type { DriveFile, Sort, SortKey, TableCommand } from "../types";
  import { SHORTCUT_MIME } from "../types";
  import { endIndex, revealRow, startIndex } from "../virtual";
  import FileIcon from "./FileIcon.svelte";
  import Icon from "./Icon.svelte";

  const ROW_H = 40;
  const HEAD_H = 34;

  interface Props {
    files: DriveFile[];
    selected: ReadonlySet<string>;
    active: string | null;
    sort: Sort;
    /** Nội dung hiển thị khi không có dòng nào (đang tải, lỗi, thư mục trống…) */
    placeholder: Snippet;
    onselect: (ids: Set<string>) => void;
    onactive: (id: string | null) => void;
    onsort: (s: Sort) => void;
    onopen: (f: DriveFile) => void;
    oncontext: (f: DriveFile | null, x: number, y: number) => void;
    oncommand: (c: TableCommand) => void;
  }

  let { files, selected, active, sort, placeholder, onselect, onactive, onsort, onopen, oncontext, oncommand }: Props = $props();

  let scroller: HTMLDivElement;
  let scrollTop = $state(0);
  let viewH = $state(600);
  /** Mốc để chọn dải bằng Shift (không cần phản ứng nên không dùng $state) */
  let anchor: string | null = null;

  const index = $derived(new Map(files.map((f, i) => [f.id, i] as const)));
  const start = $derived(startIndex(scrollTop, ROW_H));
  const end = $derived(endIndex(files.length, scrollTop, viewH, ROW_H, HEAD_H));
  const rows = $derived(files.slice(start, end));

  function range(a: number, b: number): Set<string> {
    return new Set(files.slice(Math.min(a, b), Math.max(a, b) + 1).map((f) => f.id));
  }

  function toggle(f: DriveFile) {
    const n = new Set(selected);
    if (n.has(f.id)) n.delete(f.id);
    else n.add(f.id);
    anchor = f.id;
    onactive(f.id);
    onselect(n);
  }

  function click(f: DriveFile, e: MouseEvent) {
    const i = index.get(f.id) ?? 0;
    onactive(f.id);
    if (e.shiftKey && anchor != null && index.has(anchor)) {
      onselect(range(index.get(anchor)!, i));
    } else if (e.ctrlKey || e.metaKey) {
      toggle(f);
    } else {
      anchor = f.id;
      onselect(new Set([f.id]));
    }
  }

  // Icon và tên: nhấp đúp để mở, nhấp đơn không đổi vùng chọn. Ctrl/Shift + bấm vẫn chọn như phần còn lại của dòng.
  function hotClick(f: DriveFile, e: MouseEvent) {
    if (e.shiftKey || e.ctrlKey || e.metaKey) return;
    e.stopPropagation();
    onactive(f.id);
  }

  function move(to: number, extend: boolean) {
    if (files.length === 0) return;
    const i = Math.max(0, Math.min(files.length - 1, to));
    const f = files[i];
    onactive(f.id);
    if (extend) {
      if (anchor == null || !index.has(anchor)) anchor = f.id;
      onselect(range(index.get(anchor)!, i));
    } else {
      anchor = f.id;
      onselect(new Set([f.id]));
    }
    revealRow(scroller, i, ROW_H, HEAD_H);
  }

  function onkeydown(e: KeyboardEvent) {
    // Để các nút sắp xếp trên tiêu đề tự xử lý Enter/Space.
    if ((e.target as HTMLElement).closest(".th button")) return;
    const cur = active != null ? (index.get(active) ?? -1) : -1;
    const page = Math.max(1, Math.floor(scroller.clientHeight / ROW_H) - 2);
    const mod = e.ctrlKey || e.metaKey;
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        move(cur < 0 ? 0 : cur + 1, e.shiftKey);
        break;
      case "ArrowUp":
        e.preventDefault();
        move(cur < 0 ? 0 : cur - 1, e.shiftKey);
        break;
      case "PageDown":
        e.preventDefault();
        move(cur + page, e.shiftKey);
        break;
      case "PageUp":
        e.preventDefault();
        move(cur - page, e.shiftKey);
        break;
      case "Home":
        e.preventDefault();
        move(0, e.shiftKey);
        break;
      case "End":
        e.preventDefault();
        move(files.length - 1, e.shiftKey);
        break;
      case " ":
        e.preventDefault();
        if (cur >= 0) toggle(files[cur]);
        break;
      case "Enter":
        if (cur >= 0) {
          e.preventDefault();
          onopen(files[cur]);
        }
        break;
      case "a":
      case "A":
        if (mod) {
          e.preventDefault();
          onselect(new Set(files.map((f) => f.id)));
        }
        break;
      case "Escape":
        if (selected.size > 0) onselect(new Set());
        break;
      case "Delete":
        if (selected.size > 0) {
          e.preventDefault();
          oncommand("delete");
        }
        break;
      case "F2":
        e.preventDefault();
        oncommand("rename");
        break;
      case "Backspace":
        e.preventDefault();
        oncommand("up");
        break;
      case "ArrowLeft":
        if (e.altKey) {
          e.preventDefault();
          oncommand("up");
        }
        break;
    }
  }

  function sortBy(key: SortKey) {
    onsort({ key, dir: sort.key === key && sort.dir === "asc" ? "desc" : "asc" });
  }

  function ariaSort(key: SortKey): "ascending" | "descending" | "none" {
    return sort.key === key ? (sort.dir === "asc" ? "ascending" : "descending") : "none";
  }
</script>

{#snippet sortHead(key: SortKey, label: string, cls: string)}
  <div role="columnheader" class={cls} aria-sort={ariaSort(key)}>
    <button class="th-btn" onclick={() => sortBy(key)}>
      {label}
      {#if sort.key === key}<Icon name={sort.dir === "asc" ? "arrowUp" : "arrowDown"} size={16} />{/if}
    </button>
  </div>
{/snippet}

<div
  bind:this={scroller}
  bind:clientHeight={viewH}
  class="table"
  class:idle={active == null}
  role="grid"
  aria-label="Danh sách file"
  aria-multiselectable="true"
  aria-rowcount={files.length + 1}
  aria-activedescendant={active ? `row-${active}` : undefined}
  tabindex="0"
  onscroll={() => (scrollTop = scroller.scrollTop)}
  {onkeydown}
  oncontextmenu={(e) => {
    e.preventDefault();
    oncontext(null, e.clientX, e.clientY);
  }}
  onmousedown={(e) => {
    // Bấm vào vùng trống thì bỏ chọn.
    if (e.target === e.currentTarget && selected.size > 0) onselect(new Set());
  }}
>
  <div class="th" role="row">
    {@render sortHead("name", "Tên", "c-name")}
    <div role="columnheader" class="c-owner"><span class="th-static">Chủ sở hữu</span></div>
    {@render sortHead("modified", "Sửa đổi lần cuối", "c-mod")}
    {@render sortHead("size", "Kích thước", "c-size")}
  </div>

  {#if files.length === 0}
    <div class="table-empty">{@render placeholder()}</div>
  {:else}
    <div class="tbody" style:height="{files.length * ROW_H}px">
      {#each rows as f, k (f.id)}
        {@const i = start + k}
        {@const sel = selected.has(f.id)}
        <!-- Tiêu điểm nằm ở lưới (tabindex=0 + aria-activedescendant) và bàn phím cũng xử lý ở đó; dòng chỉ nhận chuột. -->
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions, a11y_interactive_supports_focus -->
        <div
          id="row-{f.id}"
          role="row"
          aria-rowindex={i + 2}
          aria-selected={sel}
          class="tr"
          class:sel
          class:act={active === f.id}
          class:trashed={f.trashed}
          style:transform="translateY({i * ROW_H}px)"
          onclick={(e) => click(f, e)}
          oncontextmenu={(e) => {
            e.preventDefault();
            e.stopPropagation();
            oncontext(f, e.clientX, e.clientY);
          }}
        >
          <div role="gridcell" class="c-name">
            <!-- Bàn phím mở bằng Enter ở cấp lưới, nên vùng này chỉ cần chuột. -->
            <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
            <span class="hot" onclick={(e) => hotClick(f, e)} ondblclick={() => onopen(f)}>
              <FileIcon mime={f.mimeType} name={f.name} />
              <span class="nm" title={f.name}>{f.name}</span>
            </span>
            {#if f.shared}<Icon name="users" size={13} class="badge" />{/if}
            {#if f.mimeType === SHORTCUT_MIME}<span class="tag">lối tắt</span>{/if}
          </div>
          <div role="gridcell" class="c-owner" title={f.owners[0]?.emailAddress ?? undefined}>{ownerLabel(f)}</div>
          <div role="gridcell" class="c-mod num">{formatDate(f.modifiedTime)}</div>
          <div role="gridcell" class="c-size num">{isFolder(f) ? "—" : formatBytes(f.size)}</div>
        </div>
      {/each}
    </div>
  {/if}
</div>
