<script lang="ts">
  import AccountRow from "../components/AccountRow.svelte";
  import Button from "../components/Button.svelte";
  import Icon from "../components/Icon.svelte";
  import Modal from "../components/Modal.svelte";
  import { isFolder, plural } from "../format";
  import type { AccountInfo, DriveFile } from "../types";
  import { errorText } from "../util";

  type Mode = "trash" | "delete";

  interface Props {
    account: AccountInfo;
    files: DriveFile[];
    /** Đang xem thùng rác: chỉ còn xoá vĩnh viễn */
    inTrash: boolean;
    onconfirm: (mode: Mode) => Promise<void>;
    onclose: () => void;
  }

  let { account, files, inTrash, onconfirm, onclose }: Props = $props();

  const n = $derived(files.length);
  const canTrash = $derived(!inTrash && files.every((f) => f.capabilities?.canTrash !== false));
  const canDelete = $derived(files.every((f) => f.capabilities?.canDelete !== false));
  const hasFolder = $derived(files.some(isFolder));
  const shown = $derived(files.slice(0, 5));

  // Mặc định thùng rác nếu được phép
  // svelte-ignore state_referenced_locally
  let mode = $state<Mode>(inTrash || !files.every((f) => f.capabilities?.canTrash !== false) ? "delete" : "trash");
  let busy = $state(false);
  let error = $state<string | null>(null);

  const allowed = $derived(mode === "trash" ? canTrash : canDelete);

  async function confirm() {
    busy = true;
    error = null;
    try {
      await onconfirm(mode);
      onclose();
    } catch (e) {
      error = errorText(e);
      busy = false;
    }
  }
</script>

<Modal title={mode === "trash" ? "Chuyển vào thùng rác" : "Xoá vĩnh viễn"} {onclose} {busy} width={500}>
  <AccountRow {account} />

  <ul class="md-list md-files">
    {#each shown as f (f.id)}
      <li class="md-list-item"><span class="nm" title={f.name}>{f.name}</span></li>
    {/each}
    {#if n > shown.length}<li class="md-list-item more">và {plural(n - shown.length, "mục")} khác</li>{/if}
  </ul>

  {#if !inTrash}
    <fieldset class="md-list" disabled={busy}>
      <legend class="sr-only">Cách xoá</legend>
      <label class="md-list-item selectable" class:selected={mode === "trash"} class:disabled={!canTrash}>
        <input class="md-radio-input" type="radio" name="rm-mode" value="trash" bind:group={mode} disabled={!canTrash} />
        <span class="md-radio" aria-hidden="true"></span>
        <span class="md-list-text">
          <span class="md-list-headline">Chuyển vào thùng rác</span>
          <span class="md-list-support">Khôi phục được, nhưng vẫn chiếm dung lượng cho tới khi dọn thùng rác.</span>
        </span>
      </label>
      <label class="md-list-item selectable" class:selected={mode === "delete"} class:disabled={!canDelete}>
        <input class="md-radio-input" type="radio" name="rm-mode" value="delete" bind:group={mode} disabled={!canDelete} />
        <span class="md-radio" aria-hidden="true"></span>
        <span class="md-list-text">
          <span class="md-list-headline">Xoá vĩnh viễn</span>
          <span class="md-list-support">Không thể khôi phục.</span>
        </span>
      </label>
    </fieldset>
  {:else}
    <p class="md-note error"><Icon name="alert" size={20} />Xoá vĩnh viễn, không thể khôi phục.</p>
  {/if}

  {#if mode === "delete" && hasFolder}
    <p class="md-note error"><Icon name="alert" size={20} />Thư mục bị xoá cùng toàn bộ nội dung bên trong.</p>
  {/if}
  {#if !allowed}
    <p class="md-note error" role="alert">Service account không đủ quyền để {mode === "trash" ? "chuyển các mục này vào thùng rác" : "xoá vĩnh viễn các mục này"}.</p>
  {/if}
  {#if error}<p class="md-note error" role="alert">{error}</p>{/if}

  {#snippet footer()}
    <Button data-autofocus onclick={onclose} disabled={busy}>Huỷ</Button>
    <Button variant="filled" error onclick={confirm} disabled={busy || !allowed}>
      {#if busy}
        Đang xử lý…
      {:else if mode === "trash"}
        Chuyển {n > 1 ? plural(n, "mục") : "mục này"} vào thùng rác
      {:else}
        Xoá vĩnh viễn {n > 1 ? plural(n, "mục") : "mục này"}
      {/if}
    </Button>
  {/snippet}
</Modal>
