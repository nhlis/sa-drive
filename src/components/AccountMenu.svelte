<script lang="ts">
  import { tick } from "svelte";
  import { copyText } from "../format";
  import { toasts } from "../toasts.svelte";
  import type { AccountInfo } from "../types";
  import Avatar from "./Avatar.svelte";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import IconButton from "./IconButton.svelte";

  interface Props {
    account: AccountInfo;
    accounts: AccountInfo[];
    onselect: (id: string) => void;
    onadd: () => void;
    onsettings: () => void;
  }

  let { account, accounts, onselect, onadd, onsettings }: Props = $props();

  let open = $state(false);
  let trigger: HTMLButtonElement;
  let panel: HTMLDivElement | undefined = $state();

  const others = $derived(accounts.filter((a) => a.id !== account.id));
  // Phần tên miền luôn xuống dòng nguyên khối (nếu đủ chỗ) thay vì bị ngắt giữa chừng.
  const emailParts = $derived(account.clientEmail.split("@"));
  const tip = $derived(account.impersonate ? `${account.label} · giả danh ${account.impersonate}` : `${account.label} · ${account.clientEmail}`);

  async function toggle() {
    if (open) return close();
    open = true;
    await tick();
    panel?.querySelector<HTMLElement>("[data-autofocus]")?.focus();
  }

  function close(refocus = true) {
    open = false;
    if (refocus) trigger.focus();
  }

  /** Đóng bảng rồi mới chạy việc được chọn, để hộp thoại mở ra sau đó không bị bảng này đè lên. */
  function run(fn: () => void) {
    close(false);
    fn();
  }

  async function copyEmail() {
    if (await copyText(account.clientEmail)) toasts.ok("Đã sao chép email của service account");
    else toasts.info(account.clientEmail);
  }

  function onwindowkey(e: KeyboardEvent) {
    if (open && e.key === "Escape") {
      e.preventDefault();
      close();
    }
  }

  // Giữ Tab quanh trong bảng khi nó đang mở.
  function trap(e: KeyboardEvent) {
    if (e.key !== "Tab" || !panel) return;
    const items = Array.from(panel.querySelectorAll<HTMLElement>("button:not([disabled])"));
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

<svelte:window onkeydown={onwindowkey} />

<button bind:this={trigger} class="acct-btn" aria-haspopup="dialog" aria-expanded={open} title={tip} onclick={toggle}>
  <Avatar {account} size={32} />
  <span class="acct-btn-label">{account.label}</span>
  {#if account.impersonate}<span class="acct-btn-flag"><Icon name="users" size={14} /></span>{/if}
  <Icon name="chevronDown" size={16} class="acct-btn-caret" />
</button>

{#if open}
  <div class="pop-scrim" role="presentation" onmousedown={() => close(false)}></div>
  <div bind:this={panel} class="acctpanel" role="dialog" aria-label="Service account" tabindex="-1" onkeydown={trap}>
    <div class="ap-top">
      <IconButton small icon="close" label="Đóng" title="Đóng (Esc)" onclick={() => close()} />
    </div>

    <section class="ap-card ap-current" aria-label="Service account đang dùng">
      <div class="ap-id">
        <Avatar {account} size={56} />
        <div class="ap-who">
          <div class="ap-name">{account.label}</div>
          <div class="ap-email" title={account.clientEmail}>{emailParts[0]}@<span class="ap-email-domain">{emailParts.slice(1).join("@")}</span></div>
          {#if account.impersonate}
            <span class="ap-chip" title="Domain-wide delegation: mọi thao tác chạy dưới danh nghĩa người dùng này, file thuộc về họ">
              <Icon name="users" size={13} />
              Giả danh <span class="ap-chip-mail">{account.impersonate}</span>
            </span>
          {/if}
        </div>
      </div>
      <div class="ap-actions">
        <Button variant="outlined" size="compact" icon="copy" data-autofocus onclick={copyEmail} title="Dùng email này khi chia sẻ thư mục hoặc Shared Drive cho service account">Sao chép email</Button>
        <Button variant="outlined" size="compact" icon="settings" onclick={() => run(onsettings)}>Cài đặt</Button>
      </div>
    </section>

    <section class="ap-card ap-list" aria-label="Chuyển sang service account khác">
      <ul>
        {#each others as a (a.id)}
          <li>
            <button class="ap-row" onclick={() => run(() => onselect(a.id))} title={a.impersonate ? `${a.clientEmail} · giả danh ${a.impersonate}` : a.clientEmail}>
              <Avatar account={a} size={36} />
              <span class="ap-row-text">
                <span class="ap-row-label">
                  <span class="ap-row-name">{a.label}</span>
                  {#if a.impersonate}<Icon name="users" size={13} />{/if}
                </span>
                <span class="ap-row-sub">{a.clientEmail}</span>
              </span>
            </button>
          </li>
        {/each}
        <li>
          <button class="ap-row ap-add" onclick={() => run(onadd)}>
            <span class="ap-plus" aria-hidden="true"><Icon name="plus" size={18} /></span>
            <span class="ap-row-label">Thêm service account</span>
          </button>
        </li>
      </ul>
    </section>
  </div>
{/if}
