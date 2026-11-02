<script lang="ts">
  import { onMount, type Snippet } from "svelte";

  interface Props {
    title: string;
    onclose: () => void;
    width?: number;
    /** Đang xử lý: khoá việc đóng bằng Esc hoặc bấm nền */
    busy?: boolean;
    children: Snippet;
    footer?: Snippet;
  }

  let { title, onclose, width = 480, busy = false, children, footer }: Props = $props();

  const titleId = $props.id();
  let dialog: HTMLDivElement;
  let body: HTMLDivElement;
  let content: HTMLDivElement;
  let scrolled = $state(false);
  let more = $state(false);

  const FOCUSABLE =
    'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

  // Vạch phân cách (M3) chỉ hiện khi nội dung cuộn được.
  function measure() {
    scrolled = body.scrollTop > 0;
    more = body.scrollTop + body.clientHeight < body.scrollHeight - 1;
  }

  onMount(() => {
    const previous = document.activeElement as HTMLElement | null;
    const target =
      dialog.querySelector<HTMLElement>("[data-autofocus]") ??
      dialog.querySelector<HTMLElement>("input, textarea, select") ??
      dialog.querySelector<HTMLElement>("button");
    target?.focus();

    const ro = new ResizeObserver(measure);
    ro.observe(body);
    ro.observe(content);
    return () => {
      ro.disconnect();
      previous?.focus?.();
    };
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && !busy) {
      e.stopPropagation();
      onclose();
      return;
    }
    if (e.key !== "Tab") return;
    const items = Array.from(dialog.querySelectorAll<HTMLElement>(FOCUSABLE));
    if (items.length === 0) return;
    const first = items[0];
    const last = items[items.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }
</script>

<div
  class="scrim"
  role="presentation"
  onmousedown={(e) => {
    if (e.target === e.currentTarget && !busy) onclose();
  }}
>
  <div
    bind:this={dialog}
    class="modal"
    role="dialog"
    aria-modal="true"
    aria-labelledby={titleId}
    tabindex="-1"
    style:--w="{width}px"
    {onkeydown}
  >
    <header class="modal-head" class:divided={scrolled}>
      <h2 id={titleId}>{title}</h2>
    </header>
    <div bind:this={body} class="modal-body" onscroll={measure}>
      <div bind:this={content} class="modal-content">{@render children()}</div>
    </div>
    {#if footer}
      <footer class="modal-foot" class:divided={more}>{@render footer()}</footer>
    {/if}
  </div>
</div>
