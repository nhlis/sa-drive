<script lang="ts">
  import { onMount } from "svelte";
  import { api, pickJsonFiles, toApiError } from "../api";
  import Button from "../components/Button.svelte";
  import Icon from "../components/Icon.svelte";
  import Modal from "../components/Modal.svelte";
  import TextField from "../components/TextField.svelte";
  import type { ImportResult } from "../types";

  interface Props {
    /** File kéo thả vào cửa sổ, nhập ngay khi mở */
    initialPaths?: string[];
    onclose: () => void;
    /** Gọi khi bấm "Xong", kèm id account đầu tiên đã thêm */
    ondone: (firstId: string | null) => void;
  }

  let { initialPaths = [], onclose, ondone }: Props = $props();

  type Tab = "files" | "paste";
  let tab = $state<Tab>("files");
  let json = $state("");
  let label = $state("");
  let busy = $state(false);
  let results = $state<ImportResult[]>([]);
  let error = $state<{ message: string; hint: string | null } | null>(null);
  let firstId = $state<string | null>(null);

  const okCount = $derived(results.filter((r) => r.ok).length);

  const baseName = (p: string) => p.split(/[\\/]/).pop() || p;

  async function importPaths(paths: string[]) {
    busy = true;
    error = null;
    try {
      const r = await api.accounts.addFromFiles(paths);
      results = [...results, ...r];
      firstId ??= r.find((x) => x.ok)?.outcome?.account.id ?? null;
    } catch (e) {
      error = toApiError(e);
    } finally {
      busy = false;
    }
  }

  async function chooseFiles() {
    const paths = await pickJsonFiles();
    if (paths && paths.length > 0) await importPaths(paths);
  }

  async function addPasted(e: Event) {
    e.preventDefault();
    if (!json.trim()) return;
    busy = true;
    error = null;
    try {
      const outcome = await api.accounts.add(json, label.trim() || undefined);
      results = [...results, { path: "Nội dung đã dán", ok: true, outcome, error: null }];
      firstId ??= outcome.account.id;
      // Xoá khoá khỏi ô nhập sau khi lưu
      json = "";
      label = "";
    } catch (err) {
      error = toApiError(err);
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    if (initialPaths.length > 0) void importPaths(initialPaths);
  });

  function finish() {
    ondone(firstId);
    onclose();
  }
</script>

<Modal title="Thêm service account" onclose={okCount > 0 ? finish : onclose} {busy} width={560}>
  <div class="md-tabs" role="tablist" aria-label="Cách thêm">
    <button class="md-tab" role="tab" aria-selected={tab === "files"} onclick={() => (tab = "files")}><span class="md-tab-label">Từ file JSON</span></button>
    <button class="md-tab" role="tab" aria-selected={tab === "paste"} onclick={() => (tab = "paste")}><span class="md-tab-label">Dán nội dung JSON</span></button>
  </div>

  {#if tab === "files"}
    <div class="md-upload" role="tabpanel">
      <span class="md-upload-icon"><Icon name="upload" size={24} /></span>
      <div>
        <div class="md-upload-title">Chọn file khoá JSON</div>
        <div class="md-supporting">Chọn được nhiều file, hoặc kéo thả vào cửa sổ.</div>
      </div>
      <Button variant="filled" icon="upload" onclick={chooseFiles} disabled={busy}>{busy ? "Đang nhập…" : "Chọn file JSON"}</Button>
    </div>
  {:else}
    <div role="tabpanel">
      <form id="paste-form" class="md-form" onsubmit={addPasted}>
        <TextField label="Nội dung file JSON" rows={7} mono bind:value={json} disabled={busy} placeholder={'{ "type": "service_account", … }'} />
        <TextField label="Tên hiển thị (tuỳ chọn)" bind:value={label} disabled={busy} />
      </form>
    </div>
  {/if}

  {#if error}
    <div class="md-note error" role="alert">
      <span>
        {error.message}
        {#if error.hint}<span class="md-note-hint">{error.hint}</span>{/if}
      </span>
    </div>
  {/if}

  {#if results.length > 0}
    <ul class="md-list" aria-live="polite">
      {#each results as r, i (i)}
        <li class="md-list-item md-result" class:bad={!r.ok}>
          <Icon name={r.ok ? "check" : "alert"} size={20} />
          <div class="md-list-text">
            <span class="md-list-headline md-result-name">{baseName(r.path)}</span>
            {#if r.ok && r.outcome}
              <span class="md-list-support md-result-msg">
                {r.outcome.status === "added" ? "Đã thêm" : "Đã cập nhật khoá của"}
                <strong>{r.outcome.account.label}</strong>
                <span class="mono">{r.outcome.account.clientEmail}</span>
              </span>
            {:else}
              <span class="md-list-support md-result-msg">{r.error}</span>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}

  <p class="md-secure"><Icon name="lock" size={16} />Khoá được mã hoá trên máy này và chỉ gửi tới Google để lấy token.</p>

  {#snippet footer()}
    {#if okCount > 0}
      <Button variant={tab === "files" ? "filled" : "text"} onclick={finish} disabled={busy}>Xong</Button>
    {:else}
      <Button onclick={onclose} disabled={busy}>Huỷ</Button>
    {/if}
    {#if tab === "paste"}
      <Button variant="filled" type="submit" form="paste-form" disabled={busy || !json.trim()}>{busy ? "Đang nhập…" : "Thêm account"}</Button>
    {/if}
  {/snippet}
</Modal>
