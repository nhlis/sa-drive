<script lang="ts">
  import { untrack } from "svelte";
  import { api, toApiError } from "../api";
  import Button from "../components/Button.svelte";
  import FileIcon from "../components/FileIcon.svelte";
  import Icon from "../components/Icon.svelte";
  import Modal from "../components/Modal.svelte";
  import { plural, sortFiles } from "../format";
  import type { AccountInfo, DriveFile } from "../types";
  import { errorText } from "../util";

  interface Props {
    account: AccountInfo;
    files: DriveFile[];
    /** Shared Drive chứa các mục (null = My Drive); chỉ di chuyển được trong cùng một drive */
    driveId: string | null;
    rootName: string;
    /** Thư mục đang xem, không cho di chuyển vào chính nó */
    fromParent: string | null;
    onsubmit: (newParentId: string) => Promise<void>;
    onclose: () => void;
  }

  let { account, files, driveId, rootName, fromParent, onsubmit, onclose }: Props = $props();

  interface Step {
    id: string;
    name: string;
  }

  // svelte-ignore state_referenced_locally
  let trail = $state<Step[]>([{ id: driveId ?? "root", name: rootName }]);
  let folders = $state.raw<DriveFile[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let gen = 0;

  const current = $derived(trail[trail.length - 1]);
  const moving = $derived(new Set(files.map((f) => f.id)));
  const visible = $derived(folders.filter((f) => !moving.has(f.id)));
  const alreadyThere = $derived((fromParent != null && current.id === fromParent) || files.every((f) => f.parents.includes(current.id)));

  async function loadFolders(parent: string) {
    const my = ++gen;
    loading = true;
    loadError = null;
    try {
      let all: DriveFile[] = [];
      let token: string | null = null;
      do {
        const page = await api.drive.list(account.id, { mode: "children", parentId: parent, driveId, foldersOnly: true, pageToken: token });
        if (my !== gen) return;
        all = all.concat(page.files);
        token = page.nextPageToken;
      } while (token && all.length < 2000);
      folders = sortFiles(all, { key: "name", dir: "asc" });
    } catch (e) {
      if (my === gen) loadError = toApiError(e).message;
    } finally {
      if (my === gen) loading = false;
    }
  }

  $effect(() => {
    const id = current.id;
    untrack(() => void loadFolders(id));
  });

  function enter(f: DriveFile) {
    trail = [...trail, { id: f.id, name: f.name }];
  }

  async function confirm() {
    busy = true;
    error = null;
    try {
      await onsubmit(current.id);
      onclose();
    } catch (e) {
      error = errorText(e);
      busy = false;
    }
  }
</script>

<Modal title={files.length === 1 ? `Di chuyển “${files[0].name}”` : `Di chuyển ${plural(files.length, "mục")}`} {onclose} {busy} width={520}>
  <nav class="crumbs mini" aria-label="Vị trí đích">
    {#each trail as c, i (c.id)}
      {#if i > 0}<Icon name="chevronRight" size={14} class="crumb-sep" />{/if}
      {#if i < trail.length - 1}
        <button class="crumb" onclick={() => (trail = trail.slice(0, i + 1))}>{c.name}</button>
      {:else}
        <span class="crumb current" aria-current="page">{c.name}</span>
      {/if}
    {/each}
  </nav>

  <div class="md-list md-pick" aria-busy={loading}>
    {#if loading}
      <div class="skeleton-rows"><span class="skeleton"></span><span class="skeleton"></span><span class="skeleton"></span></div>
    {:else if loadError}
      <div class="md-pick-empty">
        <p class="md-note error" role="alert">{loadError}</p>
        <Button onclick={() => loadFolders(current.id)}>Thử lại</Button>
      </div>
    {:else if visible.length === 0}
      <p class="md-pick-empty md-supporting">Không có thư mục con. Bạn vẫn có thể di chuyển vào thư mục này.</p>
    {:else}
      <ul>
        {#each visible as f (f.id)}
          <li>
            <button class="md-pick-item" onclick={() => enter(f)} title="Mở “{f.name}”">
              <FileIcon mime={f.mimeType} name={f.name} />
              <span class="nm">{f.name}</span>
              <Icon name="chevronRight" size={16} />
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  {#if alreadyThere}<p class="md-supporting small">Các mục này đã nằm trong thư mục này.</p>{/if}
  {#if error}<p class="md-note error" role="alert">{error}</p>{/if}

  {#snippet footer()}
    <Button onclick={onclose} disabled={busy}>Huỷ</Button>
    <Button variant="filled" onclick={confirm} disabled={busy || alreadyThere}>
      {busy ? "Đang di chuyển…" : `Di chuyển vào “${current.name}”`}
    </Button>
  {/snippet}
</Modal>
