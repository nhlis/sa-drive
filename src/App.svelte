<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { api, listenDragDrop, toApiError, type ApiError } from "./api";
  import { applyAccountTheme, applyNeutralTheme } from "./colors";
  import ToastHost from "./components/ToastHost.svelte";
  import TransferPanel from "./components/TransferPanel.svelte";
  import AccountSettingsDialog from "./dialogs/AccountSettingsDialog.svelte";
  import AddAccountDialog from "./dialogs/AddAccountDialog.svelte";
  import HelpDialog from "./dialogs/HelpDialog.svelte";
  import { getPref, setPref } from "./prefs";
  import Spinner from "./components/Spinner.svelte";
  import EmptyState from "./screens/EmptyState.svelte";
  import VaultError from "./screens/VaultError.svelte";
  import { theme } from "./theme.svelte";
  import { toasts } from "./toasts.svelte";
  import { transfers } from "./transfers.svelte";
  import type { AccountsState, Browse, DrivesState, HelpSection } from "./types";
  import Workspace from "./Workspace.svelte";

  type DialogState =
    | { kind: "add"; paths: string[]; nonce: number }
    | { kind: "settings"; id: string }
    | { kind: "help"; section?: HelpSection };

  const HOME: Browse = { loc: { kind: "my" }, trail: [] };

  let accountsState = $state.raw<AccountsState | null>(null);
  let fatal = $state.raw<ApiError | null>(null);
  let selectedId = $state<string | null>(getPref<string | null>("account", null));
  let browse = $state.raw<Browse>(HOME);
  let drives = $state.raw<DrivesState>({ list: [], loading: false, error: null });
  let dialog = $state.raw<DialogState | null>(null);
  let dragging = $state(false);
  let workspace: ReturnType<typeof Workspace> | undefined = $state();

  const accounts = $derived(accountsState?.accounts ?? []);
  const account = $derived(accounts.find((a) => a.id === selectedId) ?? null);

  function select(id: string | null) {
    selectedId = id;
    setPref("account", id);
    browse = HOME;
  }

  async function loadAccounts() {
    try {
      const s = await api.accounts.list();
      accountsState = s;
      fatal = null;
      // Account đang chọn không còn (đã xoá): chuyển sang account đầu tiên.
      if (!s.accounts.some((a) => a.id === selectedId)) select(s.accounts[0]?.id ?? null);
    } catch (e) {
      accountsState = null;
      fatal = toApiError(e);
    }
  }

  let drivesGen = 0;
  async function loadDrives(accountId: string) {
    const my = ++drivesGen;
    drives = { list: [], loading: true, error: null };
    try {
      const list = await api.drive.sharedDrives(accountId);
      if (my === drivesGen) drives = { list, loading: false, error: null };
    } catch (e) {
      if (my === drivesGen) drives = { list: [], loading: false, error: toApiError(e) };
    }
  }

  // Màu nhận diện (nút Mới, mục đang chọn, dòng được chọn, viền focus) theo account đang chọn và theo chế độ sáng/tối của hệ điều hành.
  $effect(() => {
    const dark = theme.dark;
    if (account) applyAccountTheme(account.color, dark);
    else applyNeutralTheme(dark);
  });

  // Danh sách Shared Drive phụ thuộc account và danh tính giả danh.
  $effect(() => {
    const id = account?.id;
    void account?.impersonate;
    untrack(() => {
      if (id) void loadDrives(id);
      else drives = { list: [], loading: false, error: null };
    });
  });

  function onDrop(paths: string[]) {
    const keys = paths.filter((p) => /\.json$/i.test(p));
    // Chưa có account (hoặc đang mở hộp thoại thêm): coi file .json là khoá service account.
    if (accounts.length === 0 || dialog?.kind === "add") {
      if (keys.length > 0) dialog = { kind: "add", paths: keys, nonce: Date.now() };
      else toasts.info("Hãy thả file khoá JSON của service account.");
      return;
    }
    if (dialog) return; // đang mở hộp thoại khác
    void workspace?.handleDrop(paths);
  }

  onMount(() => {
    transfers.start();
    void loadAccounts();

    let alive = true;
    let unlisten: (() => void) | undefined;
    listenDragDrop({
      onEnter: () => (dragging = true),
      onLeave: () => (dragging = false),
      onDrop,
    })
      .then((u) => (alive ? (unlisten = u) : u()))
      .catch(() => {});
    return () => {
      alive = false;
      unlisten?.();
    };
  });

  const openHelp = (section?: HelpSection) => (dialog = { kind: "help", section });
  const openAdd = () => (dialog = { kind: "add", paths: [], nonce: 0 });
  const closeDialog = () => (dialog = null);
</script>

{#if fatal}
  <VaultError error={fatal} onretry={loadAccounts} onreset={loadAccounts} />
{:else if !accountsState}
  <div class="boot" aria-busy="true">
    <Spinner size={48} />
    <span class="sr-only">Đang mở kho khoá…</span>
  </div>
{:else if accounts.length === 0}
  <EmptyState onadd={openAdd} onhelp={openHelp} />
{:else}
  {#if account}
    {@const acc = account}
    {#key acc.id}
      <Workspace
        bind:this={workspace}
        account={acc}
        {accounts}
        weakKeyStorage={accountsState.weakKeyStorage}
        {browse}
        {drives}
        {dragging}
        onbrowse={(b) => (browse = b)}
        onselectaccount={select}
        onaddaccount={openAdd}
        onsettings={() => (dialog = { kind: "settings", id: acc.id })}
        onhelp={openHelp}
        onretrydrives={() => loadDrives(acc.id)}
      />
    {/key}
  {/if}
  <TransferPanel {accounts} />
{/if}

<ToastHost />

{#if dialog?.kind === "add"}
  {@const d = dialog}
  {#key d.nonce}
    <AddAccountDialog
      initialPaths={d.paths}
      onclose={closeDialog}
      ondone={(id) => {
        void loadAccounts().then(() => {
          if (id) select(id);
        });
      }}
    />
  {/key}
{/if}

{#if dialog?.kind === "settings"}
  {@const id = dialog.id}
  {@const acc = accounts.find((a) => a.id === id)}
  {#if acc}
    <AccountSettingsDialog account={acc} onclose={closeDialog} onchange={loadAccounts} onremoved={() => void loadAccounts()} />
  {/if}
{/if}

{#if dialog?.kind === "help"}
  <HelpDialog section={dialog.section} onclose={closeDialog} />
{/if}
