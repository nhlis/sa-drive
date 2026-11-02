<script lang="ts">
  import { tick } from "svelte";
  import { api } from "../api";
  import { PALETTE, accountColor, accountInk } from "../colors";
  import Button from "../components/Button.svelte";
  import Icon from "../components/Icon.svelte";
  import IconButton from "../components/IconButton.svelte";
  import Modal from "../components/Modal.svelte";
  import TextField from "../components/TextField.svelte";
  import { copyText, formatDate } from "../format";
  import { theme } from "../theme.svelte";
  import { toasts } from "../toasts.svelte";
  import type { AccountInfo } from "../types";
  import { EMAIL_RE, errorText } from "../util";

  interface Props {
    account: AccountInfo;
    onclose: () => void;
    onchange: () => Promise<void>;
    onremoved: (id: string) => void;
  }

  let { account, onclose, onchange, onremoved }: Props = $props();

  // svelte-ignore state_referenced_locally
  let label = $state(account.label);
  // svelte-ignore state_referenced_locally
  let color = $state(account.color);
  // svelte-ignore state_referenced_locally
  let impersonate = $state(account.impersonate ?? "");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let confirmRemove = $state(false);
  let confirmExport = $state(false);
  let keyZone = $state<HTMLElement>();
  let dangerZone = $state<HTMLElement>();

  // Cuộn tới khối xác nhận và đưa focus vào nút an toàn
  function reveal(node: HTMLElement) {
    node.scrollIntoView({ block: "nearest" });
    node.querySelector<HTMLElement>("[data-autofocus]")?.focus({ preventScroll: true });
  }

  // Nút vừa bấm đã bị gỡ: trả focus về khu vực đó
  async function refocus(zone: HTMLElement | undefined) {
    await tick();
    zone?.querySelector<HTMLElement>("[data-autofocus], .md-btn:not(:disabled)")?.focus();
  }

  async function save(e: Event) {
    e.preventDefault();
    const imp = impersonate.trim();
    if (!label.trim()) {
      error = "Tên hiển thị không được để trống.";
      return;
    }
    if (imp && !EMAIL_RE.test(imp)) {
      error = "Email giả danh không hợp lệ.";
      return;
    }
    const patch: { label?: string; color?: number; impersonate?: string } = {};
    if (label.trim() !== account.label) patch.label = label.trim();
    if (color !== account.color) patch.color = color;
    if (imp !== (account.impersonate ?? "")) patch.impersonate = imp; // chuỗi rỗng = xoá
    if (Object.keys(patch).length === 0) {
      onclose();
      return;
    }
    busy = true;
    error = null;
    try {
      await api.accounts.update(account.id, patch);
      await onchange();
      toasts.ok("Đã lưu cài đặt");
      onclose();
    } catch (err) {
      error = errorText(err);
      busy = false;
    }
  }

  async function remove() {
    busy = true;
    error = null;
    try {
      await api.accounts.remove(account.id);
      onremoved(account.id);
      onclose();
    } catch (err) {
      error = errorText(err);
      busy = false;
    }
  }

  async function exportKey() {
    busy = true;
    error = null;
    try {
      const r = await api.accounts.exportKey(account.id);
      if (r) {
        confirmExport = false;
        toasts.ok(`Đã lưu file khoá tại ${r.path}`);
      }
      // null: người dùng đóng hộp thoại lưu, giữ nguyên khối xác nhận
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
      void refocus(keyZone);
    }
  }

  async function copy(text: string) {
    if (await copyText(text)) toasts.ok("Đã sao chép");
    else toasts.info("Không sao chép được. Hãy chọn và sao chép thủ công.");
  }
</script>

<Modal title="Cài đặt account" {onclose} {busy} width={520}>
  <form id="settings-form" class="md-form" onsubmit={save}>
    <TextField label="Tên hiển thị" bind:value={label} disabled={busy} />

    <div>
      <p class="md-label" id="color-label">Màu nhận diện</p>
      <div class="md-swatches" role="radiogroup" aria-labelledby="color-label">
        {#each PALETTE as c, i (c.name)}
          <button
            type="button"
            class="md-swatch"
            role="radio"
            aria-checked={color === i}
            aria-label={c.name}
            title={c.name}
            style:--c={accountColor(i, theme.dark)}
            style:--on-c={accountInk(i, theme.dark)}
            onclick={() => (color = i)}
            disabled={busy}
          >
            {#if color === i}<Icon name="check" size={16} />{/if}
          </button>
        {/each}
      </div>
    </div>

    <TextField
      label="Giả danh người dùng (domain-wide delegation)"
      type="email"
      bind:value={impersonate}
      placeholder="Để trống nếu không dùng"
      support="Chỉ dùng với Google Workspace đã cấp delegation cho service account."
      disabled={busy}
    />
  </form>

  <dl class="md-info">
    <div>
      <dt>Email</dt>
      <dd class="mono">
        <span>{account.clientEmail}</span>
        <IconButton small icon="copy" label="Sao chép email" onclick={() => copy(account.clientEmail)} />
      </dd>
    </div>
    <div>
      <dt>Project</dt>
      <dd class="mono">{account.projectId}</dd>
    </div>
    <div>
      <dt>Key ID</dt>
      <dd class="mono">{account.privateKeyId}</dd>
    </div>
    <div>
      <dt>Thêm lúc</dt>
      <dd>{formatDate(new Date(account.addedAt).toISOString())}</dd>
    </div>
  </dl>

  <ul class="md-list">
    <li class="md-list-item md-setting key-zone" bind:this={keyZone}>
      <div class="md-setting-main">
        <Icon name="download" size={20} />
        <div class="md-list-text">
          <span class="md-list-headline">Xuất file khoá JSON</span>
          <span class="md-list-support">Dùng ở project khác hoặc nhập lại.</span>
        </div>
        {#if !confirmExport}
          <Button variant="outlined" onclick={() => (confirmExport = true)} disabled={busy}>Xuất…</Button>
        {/if}
      </div>
      {#if confirmExport}
        <div class="md-confirm" use:reveal>
          <p class="md-note warning">
            <Icon name="alert" size={20} />
            File chứa private key dạng văn bản thường. Hãy lưu ở nơi tin cậy, đừng gửi qua chat hay commit lên git.
          </p>
          <div class="md-actions">
            <Button
              data-autofocus
              onclick={() => {
                confirmExport = false;
                void refocus(keyZone);
              }}
              disabled={busy}>Không xuất</Button
            >
            <Button variant="filled" onclick={exportKey} disabled={busy}>{busy ? "Đang chờ hộp thoại lưu…" : "Chọn nơi lưu…"}</Button>
          </div>
        </div>
      {/if}
    </li>

    <li class="md-list-item md-setting danger-zone" bind:this={dangerZone}>
      <div class="md-setting-main">
        <Icon name="trash" size={20} />
        <div class="md-list-text">
          <span class="md-list-headline">Xoá account khỏi ứng dụng</span>
          <span class="md-list-support">Chỉ xoá khoá trên máy này.</span>
        </div>
        {#if !confirmRemove}
          <Button error onclick={() => (confirmRemove = true)} disabled={busy}>Xoá…</Button>
        {/if}
      </div>
      {#if confirmRemove}
        <div class="md-confirm" use:reveal>
          <p class="md-note error">
            <Icon name="alert" size={20} />
            Khoá vẫn còn hiệu lực trên Google Cloud. Muốn giữ bản sao thì xuất file khoá trước khi xoá.
          </p>
          <div class="md-actions">
            <Button
              data-autofocus
              onclick={() => {
                confirmRemove = false;
                void refocus(dangerZone);
              }}
              disabled={busy}>Giữ lại</Button
            >
            <Button variant="filled" error onclick={remove} disabled={busy}>{busy ? "Đang xoá…" : "Xoá khỏi ứng dụng"}</Button>
          </div>
        </div>
      {/if}
    </li>
  </ul>

  {#if error}<p class="md-note error" role="alert" use:reveal>{error}</p>{/if}

  {#snippet footer()}
    <Button onclick={onclose} disabled={busy}>Huỷ</Button>
    <Button variant="filled" type="submit" form="settings-form" disabled={busy}>{busy ? "Đang lưu…" : "Lưu"}</Button>
  {/snippet}
</Modal>
