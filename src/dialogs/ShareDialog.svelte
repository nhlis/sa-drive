<script lang="ts">
  import { onMount } from "svelte";
  import { api, toApiError } from "../api";
  import Button from "../components/Button.svelte";
  import Icon from "../components/Icon.svelte";
  import IconButton from "../components/IconButton.svelte";
  import Modal from "../components/Modal.svelte";
  import SelectField from "../components/SelectField.svelte";
  import TextField from "../components/TextField.svelte";
  import { copyText } from "../format";
  import { toasts } from "../toasts.svelte";
  import type { AccountInfo, DriveFile, Permission } from "../types";
  import { EMAIL_RE } from "../util";

  let { account, file, onclose }: { account: AccountInfo; file: DriveFile; onclose: () => void } = $props();

  const ROLE_LABEL: Record<string, string> = {
    owner: "Chủ sở hữu",
    organizer: "Người quản lý",
    fileOrganizer: "Người sắp xếp nội dung",
    writer: "Người chỉnh sửa",
    commenter: "Người nhận xét",
    reader: "Người xem",
  };

  let perms = $state<Permission[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);

  let email = $state("");
  let role = $state("reader");
  let notify = $state(false);
  let adding = $state(false);
  let addError = $state<string | null>(null);
  // id quyền đang gỡ, hoặc "link"
  let busyId = $state<string | null>(null);

  const anyone = $derived(perms.find((p) => p.type === "anyone"));
  const people = $derived(perms.filter((p) => p.type !== "anyone" && !p.deleted));

  async function load() {
    loading = true;
    loadError = null;
    try {
      perms = await api.drive.permissions(account.id, file.id);
    } catch (e) {
      loadError = toApiError(e).message;
    } finally {
      loading = false;
    }
  }
  onMount(load);

  function who(p: Permission): string {
    if (p.type === "domain") return `Mọi người trong ${p.domain ?? "domain"}`;
    return p.displayName || p.emailAddress || "Không rõ";
  }

  function inherited(p: Permission): boolean {
    return !!p.permissionDetails?.some((d) => d.inherited);
  }

  async function add(e: Event) {
    e.preventDefault();
    const v = email.trim();
    if (!EMAIL_RE.test(v)) {
      addError = "Nhập một địa chỉ email hợp lệ.";
      return;
    }
    adding = true;
    addError = null;
    try {
      const p = await api.drive.shareUser(account.id, file.id, v, role, notify);
      perms = [...perms.filter((x) => x.id !== p.id), p];
      email = "";
      toasts.ok(`Đã chia sẻ cho ${v}`);
    } catch (err) {
      const a = toApiError(err);
      addError = a.hint ? `${a.message} ${a.hint}` : a.message;
    } finally {
      adding = false;
    }
  }

  async function remove(p: Permission) {
    busyId = p.id;
    try {
      await api.drive.unshare(account.id, file.id, p.id);
      perms = perms.filter((x) => x.id !== p.id);
    } catch (err) {
      toasts.err(err, "Không gỡ được quyền");
    } finally {
      busyId = null;
    }
  }

  async function enableLink() {
    busyId = "link";
    try {
      const p = await api.drive.shareAnyone(account.id, file.id);
      perms = [...perms.filter((x) => x.id !== p.id), p];
    } catch (err) {
      toasts.err(err, "Không bật được liên kết");
    } finally {
      busyId = null;
    }
  }

  async function copyLink() {
    if (file.webViewLink && (await copyText(file.webViewLink))) toasts.ok("Đã sao chép liên kết");
    else toasts.info("Không sao chép được. Hãy chọn và sao chép thủ công.");
  }
</script>

<Modal title="Chia sẻ “{file.name}”" {onclose} width={560}>
  <form class="md-form" onsubmit={add}>
    <div class="md-share-row">
      <div class="grow"><TextField label="Email người nhận" type="email" bind:value={email} disabled={adding} /></div>
      <div class="role">
        <SelectField label="Vai trò" bind:value={role} disabled={adding}>
          <option value="reader">Người xem</option>
          <option value="commenter">Người nhận xét</option>
          <option value="writer">Người chỉnh sửa</option>
        </SelectField>
      </div>
      <Button variant="filled" type="submit" disabled={adding || !email.trim()}>{adding ? "Đang chia sẻ…" : "Chia sẻ"}</Button>
    </div>
    <label class="md-check" class:disabled={adding}>
      <input type="checkbox" bind:checked={notify} disabled={adding} />
      <span class="md-check-box"><Icon name="check" size={14} /></span>
      Gửi email thông báo cho người nhận
    </label>
    {#if addError}<p class="md-note error" role="alert">{addError}</p>{/if}
  </form>

  <section aria-live="polite">
    <p class="md-label">Những người có quyền truy cập</p>
    {#if loading}
      <div class="md-list"><div class="skeleton-rows"><span class="skeleton"></span><span class="skeleton"></span></div></div>
    {:else if loadError}
      <p class="md-note error" role="alert">{loadError}</p>
      <Button onclick={load}>Thử lại</Button>
    {:else}
      <ul class="md-list">
        {#each people as p (p.id)}
          <li class="md-list-item">
            <Icon name={p.type === "domain" ? "globe" : "users"} size={20} />
            <div class="md-list-text">
              <span class="md-list-headline">{who(p)}</span>
              {#if p.emailAddress && p.displayName}<span class="md-list-support md-perm-mail">{p.emailAddress}</span>{/if}
            </div>
            <span class="md-perm-role">{ROLE_LABEL[p.role] ?? p.role}</span>
            {#if p.role !== "owner"}
              <IconButton
                small
                icon="close"
                label="Gỡ quyền của {who(p)}"
                title={inherited(p) ? "Quyền kế thừa từ thư mục cha, gỡ ở thư mục đó" : "Gỡ quyền"}
                onclick={() => remove(p)}
                disabled={busyId === p.id || inherited(p)}
              />
            {:else}
              <span class="md-icon-spacer"></span>
            {/if}
          </li>
        {/each}
        {#if people.length === 0}<li class="md-list-item"><span class="md-supporting">Chưa chia sẻ cho ai.</span></li>{/if}
      </ul>

      <p class="md-label md-label-gap">Liên kết</p>
      <ul class="md-list">
        {#if anyone}
          <li class="md-list-item">
            <Icon name="globe" size={20} />
            <div class="md-list-text">
              <span class="md-list-headline">Bất kỳ ai có liên kết</span>
              <span class="md-list-support">Xem được nội dung, không cần đăng nhập</span>
            </div>
            <span class="md-perm-role">{ROLE_LABEL[anyone.role] ?? anyone.role}</span>
            <IconButton small icon="close" label="Tắt liên kết công khai" onclick={() => remove(anyone)} disabled={busyId === anyone.id} />
          </li>
        {:else}
          <li class="md-list-item">
            <Icon name="lock" size={20} />
            <div class="md-list-text">
              <span class="md-list-headline">Chỉ người được thêm ở trên</span>
            </div>
            <Button variant="outlined" onclick={enableLink} disabled={busyId === "link"}>Cho ai có liên kết xem</Button>
          </li>
        {/if}
      </ul>

      {#if file.webViewLink}
        <div class="md-share-link">
          <TextField label="Liên kết tới file" readonly mono value={file.webViewLink} onfocus={(e) => (e.currentTarget as HTMLInputElement).select()} />
          <Button variant="outlined" icon="copy" onclick={copyLink}>Sao chép</Button>
        </div>
      {/if}
    {/if}
  </section>

  {#snippet footer()}
    <Button variant="filled" onclick={onclose}>Xong</Button>
  {/snippet}
</Modal>
