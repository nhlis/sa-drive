<script lang="ts">
  import { onMount } from "svelte";
  import type { MenuItem } from "../menu";
  import Icon from "./Icon.svelte";

  let { x, y, items, onclose }: { x: number; y: number; items: MenuItem[]; onclose: () => void } = $props();

  let el: HTMLDivElement;
  let pos = $state<{ left: number; top: number } | null>(null);

  // Giữ menu trong khung nhìn: đo kích thước thật rồi kẹp vào mép cửa sổ.
  // Dùng offsetWidth/offsetHeight (kích thước bố cục), không dùng getBoundingClientRect: hiệu ứng mở menu bắt đầu ở scale(0.92)
  // nên getBoundingClientRect lúc vừa gắn vào trang nhỏ hơn thật 8% và menu sát mép phải/dưới bị tràn ra ngoài.
  $effect(() => {
    void items.length;
    const width = el.offsetWidth;
    const height = el.offsetHeight;
    pos = {
      left: Math.max(8, Math.min(x, window.innerWidth - width - 8)),
      top: Math.max(8, Math.min(y, window.innerHeight - height - 8)),
    };
  });

  onMount(() => {
    el.querySelector<HTMLElement>("button:not([disabled])")?.focus();
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
      return;
    }
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const btns = Array.from(el.querySelectorAll<HTMLElement>("button:not([disabled])"));
    if (btns.length === 0) return;
    const i = btns.indexOf(document.activeElement as HTMLElement);
    const next = e.key === "ArrowDown" ? (i + 1) % btns.length : (i - 1 + btns.length) % btns.length;
    btns[next]?.focus();
  }
</script>

<svelte:window onblur={onclose} onresize={onclose} />

<div
  class="menu-scrim"
  role="presentation"
  onmousedown={onclose}
  oncontextmenu={(e) => {
    e.preventDefault();
    onclose();
  }}
>
  <div
    bind:this={el}
    class="menu"
    role="menu"
    tabindex="-1"
    style:left="{pos?.left ?? x}px"
    style:top="{pos?.top ?? y}px"
    onmousedown={(e) => e.stopPropagation()}
    {onkeydown}
  >
    {#each items as it, i (i)}
      {#if it.separatorBefore}<div class="menu-sep" role="separator"></div>{/if}
      <button
        role="menuitem"
        class="menu-item"
        class:danger={it.danger}
        disabled={it.disabled}
        onclick={() => {
          onclose();
          it.run();
        }}
      >
        <span class="menu-ico">{#if it.icon}<Icon name={it.icon} size={15} />{/if}</span>
        {it.label}
      </button>
    {/each}
  </div>
</div>
