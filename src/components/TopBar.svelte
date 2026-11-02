<script lang="ts">
  import type { AccountInfo } from "../types";
  import AccountMenu from "./AccountMenu.svelte";
  import Icon from "./Icon.svelte";
  import IconButton from "./IconButton.svelte";

  interface Props {
    account: AccountInfo;
    accounts: AccountInfo[];
    query: string;
    placeholder: string;
    searchDisabled: boolean;
    onsearch: () => void;
    onclear: () => void;
    onhelp: () => void;
    onselectaccount: (id: string) => void;
    onaddaccount: () => void;
    onsettings: () => void;
  }

  let { account, accounts, query = $bindable(), placeholder, searchDisabled, onsearch, onclear, onhelp, onselectaccount, onaddaccount, onsettings }: Props = $props();

  let input: HTMLInputElement;

  /** Workspace gọi khi người dùng bấm Ctrl+F. */
  export function focusSearch() {
    input.focus();
    input.select();
  }
</script>

<header class="topbar">
  <div class="brand">
    <svg class="brand-mark" width="28" height="28" viewBox="0 0 22 22" aria-hidden="true">
      <rect x="1.5" y="2.5" width="19" height="17" rx="4.5" fill="var(--md-sys-color-primary)" />
      <path d="M1.5 13h19" stroke="var(--md-sys-color-on-primary)" stroke-opacity=".55" stroke-width="1.4" />
      <circle cx="6" cy="16.2" r="1.1" fill="var(--md-sys-color-on-primary)" />
      <path d="M6.5 7.5h9" stroke="var(--md-sys-color-on-primary)" stroke-width="1.6" stroke-linecap="round" />
    </svg>
    <span class="brand-name">SA Drive</span>
  </div>

  <div class="search" class:disabled={searchDisabled}>
    <Icon name="search" size={20} />
    <input
      bind:this={input}
      bind:value={query}
      class="search-input"
      type="text"
      {placeholder}
      aria-label="Tìm file theo tên"
      disabled={searchDisabled}
      autocomplete="off"
      spellcheck="false"
      onkeydown={(e) => {
        if (e.key === "Enter") onsearch();
        else if (e.key === "Escape") {
          onclear();
          input.blur();
        }
      }}
    />
    {#if query}
      <IconButton small icon="close" label="Xoá từ khoá" title="Xoá từ khoá (Esc)" onclick={onclear} />
    {/if}
  </div>

  <div class="top-end">
    <IconButton icon="help" label="Hướng dẫn và giới hạn" onclick={() => onhelp()} />
    <AccountMenu {account} {accounts} onselect={onselectaccount} onadd={onaddaccount} {onsettings} />
  </div>
</header>
