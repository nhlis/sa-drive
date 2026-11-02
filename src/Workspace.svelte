<script lang="ts">
  import { untrack } from "svelte";
  import { api, pickFiles, pickFolder, toApiError, type ApiError } from "./api";
  import Button from "./components/Button.svelte";
  import ContextMenu from "./components/ContextMenu.svelte";
  import FileTable from "./components/FileTable.svelte";
  import Icon from "./components/Icon.svelte";
  import IconButton from "./components/IconButton.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import Spinner from "./components/Spinner.svelte";
  import TopBar from "./components/TopBar.svelte";
  import ConfirmDialog from "./dialogs/ConfirmDialog.svelte";
  import ConfirmRemoveDialog from "./dialogs/ConfirmRemoveDialog.svelte";
  import MoveDialog from "./dialogs/MoveDialog.svelte";
  import PromptDialog from "./dialogs/PromptDialog.svelte";
  import ShareDialog from "./dialogs/ShareDialog.svelte";
  import { copyText, formatBytes, isFolder, plural, sortFiles } from "./format";
  import type { MenuItem } from "./menu";
  import { getPref, setPref } from "./prefs";
  import { toasts } from "./toasts.svelte";
  import { transfers } from "./transfers.svelte";
  import {
    FOLDER_MIME,
    SHORTCUT_MIME,
    type About,
    type AccountInfo,
    type Browse,
    type Capabilities,
    type Crumb,
    type DriveFile,
    type DrivesState,
    type HelpSection,
    type ListParams,
    type Loc,
    type Sort,
    type TableCommand,
  } from "./types";
  import { mapPool } from "./util";

  interface Props {
    /** Service account đang dùng */
    account: AccountInfo;
    /** Tất cả account (để chuyển qua lại ở góc trên bên phải) */
    accounts: AccountInfo[];
    weakKeyStorage: boolean;
    browse: Browse;
    drives: DrivesState;
    /** Đang kéo file vào cửa sổ (sự kiện gốc từ Tauri) */
    dragging: boolean;
    onbrowse: (b: Browse) => void;
    onselectaccount: (id: string) => void;
    onaddaccount: () => void;
    onsettings: () => void;
    onhelp: (section?: HelpSection) => void;
    onretrydrives: () => void;
  }

  let { account, accounts, weakKeyStorage, browse, drives, dragging, onbrowse, onselectaccount, onaddaccount, onsettings, onhelp, onretrydrives }: Props = $props();

  // ───────────── vị trí đang xem ─────────────

  interface Plan {
    params: ListParams;
    /** Thư mục nhận file tải lên; null nghĩa là không tải lên được ở kiểu xem này */
    parentId: string | null;
    driveId: string | null;
    /** Tên thư mục hiện tại */
    title: string;
  }

  const LOC_LABEL = {
    my: "Drive của SA",
    owned: "Tất cả file của SA",
    shared: "Được chia sẻ với SA",
    trash: "Thùng rác",
  } as const;

  function locLabel(loc: Loc): string {
    switch (loc.kind) {
      case "drive":
        return loc.name;
      case "search":
        return `Kết quả cho “${loc.query}”`;
      default:
        return LOC_LABEL[loc.kind];
    }
  }

  function planFor(b: Browse): Plan {
    const last = b.trail[b.trail.length - 1];
    if (last) {
      return { params: { mode: "children", parentId: last.id, driveId: last.driveId }, parentId: last.id, driveId: last.driveId, title: last.name };
    }
    const loc = b.loc;
    switch (loc.kind) {
      case "my":
        return { params: { mode: "children", parentId: "root" }, parentId: "root", driveId: null, title: locLabel(loc) };
      case "drive":
        // Gốc của Shared Drive có id trùng với id của drive.
        return { params: { mode: "children", parentId: loc.id, driveId: loc.id }, parentId: loc.id, driveId: loc.id, title: loc.name };
      case "owned":
        return { params: { mode: "owned" }, parentId: null, driveId: null, title: locLabel(loc) };
      case "shared":
        return { params: { mode: "shared" }, parentId: null, driveId: null, title: locLabel(loc) };
      case "trash":
        return { params: { mode: "trash" }, parentId: null, driveId: null, title: locLabel(loc) };
      case "search":
        return { params: { mode: "search", query: loc.query, driveId: loc.driveId }, parentId: null, driveId: loc.driveId, title: locLabel(loc) };
    }
  }

  const plan = $derived(planFor(browse));
  const inTrash = $derived(browse.loc.kind === "trash");
  const canUpload = $derived(plan.parentId != null);
  const uploadHint = "Mở Drive của SA, một Shared Drive hoặc một thư mục để tải lên.";

  const crumbs = $derived.by(() => {
    const out: { label: string; go: () => void }[] = [{ label: locLabel(browse.loc), go: () => onbrowse({ loc: browse.loc, trail: [] }) }];
    browse.trail.forEach((c, i) => out.push({ label: c.name, go: () => onbrowse({ loc: browse.loc, trail: browse.trail.slice(0, i + 1) }) }));
    return out;
  });

  function enter(c: Crumb) {
    onbrowse({ loc: browse.loc, trail: [...browse.trail, c] });
  }

  function goUp() {
    if (browse.trail.length > 0) onbrowse({ loc: browse.loc, trail: browse.trail.slice(0, -1) });
    else if (browse.loc.kind === "search") onbrowse(browse.loc.prev);
  }

  // ───────────── dữ liệu ─────────────

  /** Số mục tối đa tự tải; vượt quá thì dừng và để người dùng chọn "Tải tiếp". */
  const MAX_ITEMS = 10_000;

  let files = $state.raw<DriveFile[]>([]);
  let nextToken = $state<string | null>(null);
  let loading = $state(true);
  let paging = $state(false);
  let loadingMore = $state(false);
  let loadError = $state<ApiError | null>(null);
  let sort = $state<Sort>(getPref<Sort>("sort", { key: "name", dir: "asc" }));
  let selected = $state.raw<ReadonlySet<string>>(new Set());
  let active = $state<string | null>(null);
  let about = $state.raw<About | null>(null);
  let aboutLoading = $state(true);
  let aboutError = $state<ApiError | null>(null);
  let query = $state("");
  let topbar: ReturnType<typeof TopBar> | undefined = $state();

  const sorted = $derived(sortFiles(files, sort));
  const selFiles = $derived(sorted.filter((f) => selected.has(f.id)));
  const one = $derived(selFiles.length === 1 ? selFiles[0] : null);
  const selBytes = $derived(selFiles.reduce((n, f) => n + (isFolder(f) ? 0 : (f.size ?? 0)), 0));

  let gen = 0;

  async function loadPages(b: Browse, silent: boolean): Promise<void> {
    const my = ++gen;
    const params = planFor(b).params;
    if (!silent) {
      loading = true;
      paging = false;
      loadError = null;
      files = [];
      nextToken = null;
    }
    let token: string | null = null;
    let acc: DriveFile[] = [];
    let total = 0;
    try {
      for (;;) {
        const page = await api.drive.list(account.id, { ...params, pageToken: token });
        if (gen !== my) return;
        total += page.files.length;
        token = page.nextPageToken || null;
        if (silent) {
          acc = acc.concat(page.files);
        } else {
          files = files.concat(page.files);
          loading = false;
          paging = !!token && total < MAX_ITEMS;
        }
        if (!token || total >= MAX_ITEMS) break;
      }
      if (silent) files = acc;
      nextToken = token;
      loading = false;
      paging = false;
      loadError = null;
      // Bỏ những mục đã chọn nhưng không còn trong danh sách.
      const ids = new Set(files.map((f) => f.id));
      if (Array.from(selected).some((id) => !ids.has(id))) selected = new Set(Array.from(selected).filter((id) => ids.has(id)));
    } catch (e) {
      if (gen !== my) return;
      loading = false;
      paging = false;
      if (!silent) loadError = toApiError(e);
    }
  }

  async function loadMore() {
    let token = nextToken;
    if (!token || loadingMore) return;
    const my = gen;
    loadingMore = true;
    paging = true;
    try {
      let added = 0;
      while (token && added < MAX_ITEMS) {
        const page = await api.drive.list(account.id, { ...plan.params, pageToken: token });
        if (gen !== my) return;
        added += page.files.length;
        token = page.nextPageToken || null;
        files = files.concat(page.files);
      }
      nextToken = token;
    } catch (e) {
      if (gen === my) toasts.err(e, "Không tải thêm được");
    } finally {
      loadingMore = false;
      paging = false;
    }
  }

  const reload = () => loadPages(browse, false);

  let aboutGen = 0;
  async function refreshAbout(initial: boolean) {
    const my = ++aboutGen;
    if (initial) {
      aboutLoading = true;
      aboutError = null;
    }
    try {
      const a = await api.drive.about(account.id);
      if (my === aboutGen) {
        about = a;
        aboutError = null;
      }
    } catch (e) {
      if (my === aboutGen && initial) aboutError = toApiError(e);
    } finally {
      if (my === aboutGen) aboutLoading = false;
    }
  }

  // Tải danh sách mỗi khi đổi vị trí duyệt.
  $effect(() => {
    const b = browse;
    untrack(() => {
      selected = new Set();
      active = null;
      query = b.loc.kind === "search" ? b.loc.query : "";
      void loadPages(b, false);
    });
  });

  // Đọc dung lượng; đọc lại khi đổi người dùng giả danh vì dung lượng khi đó là của người ấy.
  $effect(() => {
    void account.impersonate;
    untrack(() => {
      about = null;
      void refreshAbout(true);
    });
  });

  // Khi có truyền tải xong thì làm mới danh sách (gộp theo nhịp 2,5 giây để không gọi API dồn dập).
  $effect(() => {
    const id = account.id;
    let timer: number | null = null;
    const off = transfers.onDone((t) => {
      if (t.accountId !== id || timer != null) return;
      timer = window.setTimeout(() => {
        timer = null;
        void loadPages(browse, true);
        void refreshAbout(false);
      }, 2500);
    });
    return () => {
      off();
      if (timer != null) window.clearTimeout(timer);
    };
  });

  // ───────────── tìm kiếm ─────────────

  const scopeName = $derived(plan.driveId ? (drives.list.find((d) => d.id === plan.driveId)?.name ?? "Shared Drive") : "Drive của SA");

  function currentDriveId(b: Browse): string | null {
    const last = b.trail[b.trail.length - 1];
    return last ? last.driveId : b.loc.kind === "drive" ? b.loc.id : null;
  }

  function doSearch() {
    const q = query.trim();
    if (!q) {
      clearSearch();
      return;
    }
    const base: Browse = browse.loc.kind === "search" ? browse.loc.prev : browse;
    onbrowse({ loc: { kind: "search", query: q, driveId: currentDriveId(base), prev: base }, trail: [] });
  }

  function clearSearch() {
    query = "";
    if (browse.loc.kind === "search") onbrowse(browse.loc.prev);
  }

  // ───────────── thao tác ─────────────

  type ModalState =
    | { kind: "newFolder" }
    | { kind: "rename"; file: DriveFile }
    | { kind: "remove"; files: DriveFile[] }
    | { kind: "move"; files: DriveFile[]; driveId: string | null }
    | { kind: "share"; file: DriveFile }
    | { kind: "emptyTrash" };

  let modal = $state.raw<ModalState | null>(null);
  let menu = $state.raw<{ x: number; y: number; items: MenuItem[] } | null>(null);

  const closeModal = () => (modal = null);
  const cap = (f: DriveFile, k: keyof Capabilities) => f.capabilities?.[k] !== false;

  async function startUpload(paths: string[]) {
    if (plan.parentId == null) {
      toasts.info(uploadHint);
      return;
    }
    try {
      await api.transfers.upload(account.id, plan.parentId, paths);
      toasts.info(`Đã thêm ${plural(paths.length, "mục")} vào hàng đợi tải lên`);
    } catch (e) {
      toasts.err(e, "Không bắt đầu được việc tải lên");
    }
  }

  async function uploadFiles() {
    if (!canUpload) return toasts.info(uploadHint);
    const paths = await pickFiles("Chọn file để tải lên");
    if (paths && paths.length > 0) await startUpload(paths);
  }

  async function uploadFolder() {
    if (!canUpload) return toasts.info(uploadHint);
    const dir = await pickFolder("Chọn thư mục để tải lên");
    if (dir) await startUpload([dir]);
  }

  /** Nút "Mới" ở thanh bên: mở menu ngay dưới nút. */
  function openNewMenu(anchor: HTMLElement) {
    if (!canUpload) {
      toasts.info(uploadHint);
      return;
    }
    const r = anchor.getBoundingClientRect();
    menu = {
      x: r.left,
      y: r.bottom + 6,
      items: [
        { label: "Thư mục mới", icon: "folderPlus", run: () => (modal = { kind: "newFolder" }) },
        { label: "Tải file lên", icon: "upload", separatorBefore: true, run: uploadFiles },
        { label: "Tải thư mục lên", icon: "folder", run: uploadFolder },
      ],
    };
  }

  /** App gọi khi người dùng thả file vào cửa sổ. */
  export async function handleDrop(paths: string[]) {
    if (!canUpload) {
      toasts.info(uploadHint);
      return;
    }
    await startUpload(paths);
  }

  async function download(fs: DriveFile[], ask: boolean) {
    const items = fs
      .filter((f) => cap(f, "canDownload"))
      .map((f) => ({ id: f.id, name: f.name, mimeType: f.mimeType, size: f.size, driveId: f.driveId }));
    if (items.length === 0) {
      toasts.info("Không có mục nào được phép tải xuống.");
      return;
    }
    let dir = ask ? null : getPref<string | null>("downloadDir", null);
    if (!dir) {
      dir = await pickFolder("Chọn thư mục lưu file tải xuống");
      if (!dir) return;
      setPref("downloadDir", dir);
    }
    try {
      await api.transfers.download(account.id, items, dir);
      toasts.info(`Đang tải ${plural(items.length, "mục")} về ${dir}`);
    } catch (e) {
      setPref("downloadDir", null); // thư mục có thể đã bị xoá hoặc đổi tên: lần sau hỏi lại
      toasts.err(e, "Không bắt đầu được việc tải xuống");
    }
  }

  function open(f: DriveFile) {
    if (inTrash) {
      if (isFolder(f)) toasts.info("Khôi phục thư mục này để xem nội dung bên trong.");
      return;
    }
    if (f.mimeType === SHORTCUT_MIME) {
      const d = f.shortcutDetails;
      if (d?.targetId && d.targetMimeType === FOLDER_MIME) enter({ id: d.targetId, name: f.name, driveId: f.driveId });
      else toasts.info("Đây là lối tắt tới một file. Hãy mở file gốc để tải xuống.");
      return;
    }
    if (isFolder(f)) enter({ id: f.id, name: f.name, driveId: f.driveId });
    else void download([f], false);
  }

  async function doCreateFolder(name: string) {
    if (plan.parentId == null) throw new Error(uploadHint);
    const f = await api.drive.createFolder(account.id, name, plan.parentId);
    files = [f, ...files];
    selected = new Set([f.id]);
    active = f.id;
    toasts.ok(`Đã tạo thư mục “${f.name}”`);
  }

  async function doRename(f: DriveFile, name: string) {
    const u = await api.drive.rename(account.id, f.id, name);
    files = files.map((x) => (x.id === u.id ? u : x));
  }

  /** Chạy một thao tác lên nhiều file; trả về các id thành công và lỗi đầu tiên. */
  async function batch<T>(fs: DriveFile[], fn: (f: DriveFile) => Promise<T>) {
    const res = await mapPool(fs, 4, fn);
    const done = new Map<string, T>();
    let failed = 0;
    let firstErr: unknown = null;
    res.forEach((r, i) => {
      if (r.status === "fulfilled") done.set(fs[i].id, r.value);
      else {
        failed++;
        firstErr ??= r.reason;
      }
    });
    return { done, failed, firstErr };
  }

  function dropFromList(ids: ReadonlySet<string>) {
    files = files.filter((f) => !ids.has(f.id));
    selected = new Set(Array.from(selected).filter((id) => !ids.has(id)));
  }

  async function doRemove(fs: DriveFile[], mode: "trash" | "delete") {
    const r = await batch(fs, (f) => (mode === "trash" ? api.drive.trash(account.id, f.id) : api.drive.remove(account.id, f.id)));
    dropFromList(new Set(r.done.keys()));
    void refreshAbout(false);
    if (r.done.size > 0) {
      toasts.ok(mode === "trash" ? `Đã chuyển ${plural(r.done.size, "mục")} vào thùng rác` : `Đã xoá vĩnh viễn ${plural(r.done.size, "mục")}`);
    }
    if (r.failed > 0) {
      // Không thành công mục nào: để hộp thoại hiện lỗi. Có một phần thành công: báo riêng và đóng hộp thoại.
      if (r.done.size === 0) throw toApiError(r.firstErr);
      toasts.err(r.firstErr, `${plural(r.failed, "mục")} không xử lý được`);
    }
  }

  async function restore(fs: DriveFile[]) {
    const r = await batch(fs, (f) => api.drive.restore(account.id, f.id));
    dropFromList(new Set(r.done.keys()));
    if (r.done.size > 0) toasts.ok(`Đã khôi phục ${plural(r.done.size, "mục")}`);
    if (r.failed > 0) toasts.err(r.firstErr, `${plural(r.failed, "mục")} không khôi phục được`);
  }

  async function doEmptyTrash() {
    await api.drive.emptyTrash(account.id);
    files = [];
    selected = new Set();
    void refreshAbout(false);
    toasts.ok("Đã dọn sạch thùng rác");
  }

  async function doMove(fs: DriveFile[], newParent: string) {
    const r = await batch(fs, (f) => api.drive.move(account.id, f.id, newParent, f.parents));
    if (r.done.size > 0) {
      const here = plan.parentId;
      // Ở danh sách thư mục, mục đã chuyển đi nơi khác thì biến mất; ở danh sách phẳng chỉ cần cập nhật cha.
      files = files.flatMap((f) => {
        const u = r.done.get(f.id);
        if (!u) return [f];
        return here != null && newParent !== here ? [] : [u];
      });
      selected = new Set(Array.from(selected).filter((id) => !r.done.has(id) || (here == null || newParent === here)));
      toasts.ok(`Đã di chuyển ${plural(r.done.size, "mục")}`);
    }
    if (r.failed > 0) {
      if (r.done.size === 0) throw toApiError(r.firstErr);
      toasts.err(r.firstErr, `${plural(r.failed, "mục")} không di chuyển được`);
    }
  }

  function startRename() {
    if (one && cap(one, "canRename")) modal = { kind: "rename", file: one };
  }

  function startShare() {
    if (one) modal = { kind: "share", file: one };
  }

  function startRemove() {
    if (selFiles.length > 0) modal = { kind: "remove", files: selFiles };
  }

  function startMove() {
    const sel = selFiles;
    if (sel.length === 0) return;
    const drive = sel[0].driveId ?? null;
    if (sel.some((f) => (f.driveId ?? null) !== drive)) {
      toasts.info("Chỉ di chuyển được các mục nằm trong cùng một drive.");
      return;
    }
    if (!sel.every((f) => cap(f, "canMoveItemWithinDrive"))) {
      toasts.info("Service account không có quyền di chuyển một số mục đã chọn.");
      return;
    }
    modal = { kind: "move", files: sel, driveId: drive };
  }

  async function copyId(f: DriveFile) {
    if (await copyText(f.id)) toasts.ok("Đã sao chép ID");
    else toasts.info(`ID: ${f.id}`);
  }

  function onCommand(c: TableCommand) {
    if (c === "up") goUp();
    else if (c === "delete") startRemove();
    else startRename();
  }

  // ───────────── menu chuột phải ─────────────

  function menuItems(target: DriveFile | null): MenuItem[] {
    if (!target) {
      if (inTrash) {
        return [
          { label: "Làm mới", icon: "refresh", run: reload },
          { label: "Dọn sạch thùng rác…", icon: "trash", danger: true, separatorBefore: true, disabled: files.length === 0, run: () => (modal = { kind: "emptyTrash" }) },
        ];
      }
      return [
        { label: "Tải file lên", icon: "upload", disabled: !canUpload, run: uploadFiles },
        { label: "Tải thư mục lên", icon: "folder", disabled: !canUpload, run: uploadFolder },
        { label: "Thư mục mới", icon: "folderPlus", disabled: !canUpload, run: () => (modal = { kind: "newFolder" }) },
        { label: "Làm mới", icon: "refresh", separatorBefore: true, run: reload },
      ];
    }
    const sel = selFiles;
    if (inTrash) {
      return [
        { label: "Khôi phục", icon: "restore", disabled: !sel.every((f) => cap(f, "canUntrash")), run: () => void restore(sel) },
        { label: "Xoá vĩnh viễn…", icon: "trash", danger: true, separatorBefore: true, run: startRemove },
      ];
    }
    const items: MenuItem[] = [];
    if (one && isFolder(one)) items.push({ label: "Mở", icon: "folder", run: () => open(one) });
    items.push(
      { label: "Tải xuống", icon: "download", disabled: !sel.every((f) => cap(f, "canDownload")), run: () => void download(sel, false) },
      { label: "Tải xuống vào…", icon: "download", disabled: !sel.every((f) => cap(f, "canDownload")), run: () => void download(sel, true) },
      { label: "Chia sẻ…", icon: "share", separatorBefore: true, disabled: !one || !cap(one, "canShare"), run: startShare },
      { label: "Đổi tên", icon: "edit", disabled: !one || !cap(one, "canRename"), run: startRename },
      { label: "Di chuyển…", icon: "move", run: startMove },
      { label: "Sao chép ID", icon: "copy", disabled: !one, run: () => one && void copyId(one) },
      { label: "Xoá…", icon: "trash", danger: true, separatorBefore: true, run: startRemove },
    );
    return items;
  }

  function onContext(f: DriveFile | null, x: number, y: number) {
    if (f && !selected.has(f.id)) {
      selected = new Set([f.id]);
      active = f.id;
    }
    menu = { x, y, items: menuItems(f) };
  }

  function onWindowKey(e: KeyboardEvent) {
    if (modal || menu) return;
    const mod = e.ctrlKey || e.metaKey;
    if (mod && (e.key === "f" || e.key === "F")) {
      e.preventDefault();
      topbar?.focusSearch();
    } else if (e.key === "F5" || (mod && (e.key === "r" || e.key === "R"))) {
      e.preventDefault();
      void reload();
    }
  }

  // ───────────── hiển thị ─────────────

  const noQuotaHere = $derived(!account.impersonate && about?.storageQuota?.limit === 0 && plan.parentId != null && plan.driveId == null);
  const rootName = (driveId: string | null) => (driveId ? (drives.list.find((d) => d.id === driveId)?.name ?? "Shared Drive") : LOC_LABEL.my);
</script>

<svelte:window onkeydown={onWindowKey} />

{#snippet placeholder()}
  {#if loading}
    <div class="skeleton-table" aria-busy="true" aria-label="Đang tải danh sách">
      {#each [0, 1, 2, 3, 4, 5, 6, 7] as i (i)}
        <div class="sk-row" style:opacity={1 - i * 0.1}>
          <span class="skeleton" style:width="20px" style:height="20px"></span>
          <span class="skeleton" style:width="{34 - ((i * 7) % 18)}%"></span>
        </div>
      {/each}
    </div>
  {:else if loadError}
    <div class="state err" role="alert">
      <Icon name="alert" size={26} />
      <h2>Không tải được danh sách</h2>
      <p>{loadError.message}</p>
      {#if loadError.hint}<p class="hint">{loadError.hint}</p>{/if}
      <Button variant="outlined" onclick={reload}>Thử lại</Button>
    </div>
  {:else if browse.loc.kind === "search"}
    <div class="state">
      <Icon name="search" size={26} />
      <h2>Không có kết quả</h2>
      <p>Không file nào khớp “{browse.loc.query}”.</p>
    </div>
  {:else if inTrash}
    <div class="state">
      <Icon name="trash" size={26} />
      <h2>Thùng rác trống</h2>
      <p>File chuyển vào thùng rác của service account này sẽ hiện ở đây.</p>
    </div>
  {:else if browse.loc.kind === "shared" && browse.trail.length === 0}
    <div class="state">
      <Icon name="users" size={26} />
      <h2>Chưa có gì được chia sẻ với SA này</h2>
      <p>Chia sẻ thư mục cho email của service account (quyền Editor trở lên) để nó thấy và quản lý được.</p>
      <Button variant="outlined" icon="copy" onclick={async () => ((await copyText(account.clientEmail)) ? toasts.ok("Đã sao chép email của SA") : toasts.info(account.clientEmail))}>
        Sao chép email của SA
      </Button>
    </div>
  {:else if browse.loc.kind === "owned" && browse.trail.length === 0}
    <div class="state">
      <Icon name="inbox" size={26} />
      <h2>SA này chưa sở hữu file nào</h2>
      <p>File do service account tạo ra thuộc về nó sẽ hiện ở đây, kể cả khi nằm trong thư mục con.</p>
    </div>
  {:else}
    <div class="state">
      <Icon name="folder" size={26} />
      <h2>Thư mục trống</h2>
      {#if browse.loc.kind === "my" && browse.trail.length === 0 && drives.list.length > 0}
        <p>Drive riêng của SA chưa có gì. Service account này có {plural(drives.list.length, "Shared Drive")}, chọn ở thanh bên.</p>
      {:else}
        <p>Kéo file vào cửa sổ hoặc bấm Tải file lên.</p>
      {/if}
    </div>
  {/if}
{/snippet}

<div class="shell">
  <TopBar
    bind:this={topbar}
    bind:query
    {account}
    {accounts}
    placeholder={inTrash ? "Không tìm được trong thùng rác" : `Tìm theo tên trong ${scopeName}`}
    searchDisabled={inTrash}
    onsearch={doSearch}
    onclear={clearSearch}
    onhelp={() => onhelp()}
    {onselectaccount}
    {onaddaccount}
    {onsettings}
  />

  <Sidebar
    loc={browse.loc}
    {drives}
    canNew={canUpload}
    {weakKeyStorage}
    {about}
    {aboutLoading}
    {aboutError}
    onnew={openNewMenu}
    onplace={(loc) => onbrowse({ loc, trail: [] })}
    {onhelp}
    {onretrydrives}
    onretryabout={() => refreshAbout(true)}
  />

  <section class="card" aria-label="Không gian làm việc của {account.label}">
    <!-- Thanh thao tác nằm đè đúng chỗ thanh tiêu đề, để bảng không bị đẩy xuống khi chọn dòng đầu tiên. -->
    <div class="head">
      <div class="titlebar" class:covered={selFiles.length > 0}>
        <nav class="crumbs" aria-label="Đường dẫn">
          {#if browse.trail.length > 0 || browse.loc.kind === "search"}
            <IconButton icon="arrowUp" label="Lên một cấp" title="Lên một cấp (Backspace)" onclick={goUp} />
          {/if}
          {#each crumbs as c, i (i)}
            {#if i > 0}<Icon name="chevronRight" size={16} class="crumb-sep" />{/if}
            {#if i < crumbs.length - 1}
              <button class="crumb" onclick={c.go} title={c.label}>{c.label}</button>
            {:else}
              <h1 class="crumb current" aria-current="page" title={c.label}>{c.label}</h1>
            {/if}
          {/each}
        </nav>
        <div class="title-actions">
          {#if inTrash}
            <Button variant="outlined" error size="compact" icon="trash" onclick={() => (modal = { kind: "emptyTrash" })} disabled={files.length === 0}>Dọn sạch thùng rác</Button>
          {/if}
          <IconButton icon="refresh" label="Làm mới" title="Làm mới (F5)" onclick={reload} />
        </div>
      </div>

      {#if selFiles.length > 0}
        <div class="selbar" role="toolbar" aria-label="Thao tác với mục đã chọn">
          <IconButton small icon="close" label="Bỏ chọn" title="Bỏ chọn (Esc)" onclick={() => (selected = new Set())} />
          <span class="sel-count">
            Đã chọn {plural(selFiles.length, "mục")}{#if selBytes > 0}{" "}<span class="sel-size num">· {formatBytes(selBytes)}</span>{/if}
          </span>
          <span class="sel-sep" aria-hidden="true"></span>
          {#if inTrash}
            <Button
              size="dense"
              icon="restore"
              aria-label="Khôi phục"
              title="Khôi phục"
              onclick={() => void restore(selFiles)}
              disabled={!selFiles.every((f) => cap(f, "canUntrash"))}
            >
              <span class="lbl">Khôi phục</span>
            </Button>
            <Button size="dense" error icon="trash" aria-label="Xoá vĩnh viễn" title="Xoá vĩnh viễn (Delete)" onclick={startRemove}><span class="lbl">Xoá vĩnh viễn</span></Button>
          {:else}
            <Button
              size="dense"
              icon="download"
              aria-label="Tải xuống"
              title="Tải xuống"
              onclick={() => void download(selFiles, false)}
              disabled={!selFiles.every((f) => cap(f, "canDownload"))}
            >
              <span class="lbl">Tải xuống</span>
            </Button>
            <Button size="dense" icon="edit" aria-label="Đổi tên" title="Đổi tên (F2)" onclick={startRename} disabled={!one || !cap(one, "canRename")}>
              <span class="lbl">Đổi tên</span>
            </Button>
            <Button size="dense" icon="move" aria-label="Di chuyển" title="Di chuyển" onclick={startMove}><span class="lbl">Di chuyển</span></Button>
            <Button size="dense" icon="share" aria-label="Chia sẻ" title="Chia sẻ" onclick={startShare} disabled={!one || !cap(one, "canShare")}>
              <span class="lbl">Chia sẻ</span>
            </Button>
            <Button size="dense" error icon="trash" aria-label="Xoá" title="Xoá (Delete)" onclick={startRemove}><span class="lbl">Xoá</span></Button>
          {/if}
        </div>
      {/if}
    </div>

    {#if noQuotaHere}
      <div class="banner warn" role="note">
        <Icon name="alert" size={18} />
        <p>
          Service account này không có dung lượng riêng, nên Google sẽ từ chối tải lên ở đây (<code>storageQuotaExceeded</code>). Chỉ tải lên được
          trong Shared Drive hoặc khi dùng domain-wide delegation.
        </p>
        <Button size="compact" onclick={() => onhelp("quota")}>Cách khắc phục</Button>
      </div>
    {/if}

    <div class="table-area">
      <FileTable
        files={sorted}
        {selected}
        {active}
        {sort}
        {placeholder}
        onselect={(ids) => (selected = ids)}
        onactive={(id) => (active = id)}
        onsort={(s) => {
          sort = s;
          setPref("sort", s);
        }}
        onopen={open}
        oncontext={onContext}
        oncommand={onCommand}
      />

      {#if dragging}
        <div class="drop" class:blocked={!canUpload} aria-hidden="true">
          <div class="drop-card">
            {#if canUpload}
              <Icon name="upload" size={28} />
              <strong>Thả để tải lên “{plan.title}”</strong>
              <span>Tải lên bằng service account {account.label}</span>
            {:else}
              <Icon name="alert" size={28} />
              <strong>Không thể tải lên ở đây</strong>
              <span>{uploadHint}</span>
            {/if}
          </div>
        </div>
      {/if}
    </div>

    <footer class="statusbar">
      <span>
        {#if loading}Đang tải…{:else}{plural(files.length, "mục")}{/if}
      </span>
      {#if paging}<span class="busy"><Spinner size={16} /> Đang tải thêm…</span>{/if}
      {#if nextToken && !paging}
        <span>
          Mới hiển thị {plural(files.length, "mục")} đầu tiên.
          <button class="link" onclick={loadMore} disabled={loadingMore}>Tải tiếp</button>
        </span>
      {/if}
    </footer>
  </section>
</div>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

{#if modal?.kind === "newFolder"}
  <PromptDialog title="Thư mục mới" label="Tên thư mục" confirmLabel="Tạo" onsubmit={doCreateFolder} onclose={closeModal} />
{/if}

{#if modal?.kind === "rename"}
  {@const file = modal.file}
  <PromptDialog
    title="Đổi tên"
    label="Tên mới"
    initial={file.name}
    selectStem={!isFolder(file)}
    confirmLabel="Đổi tên"
    onsubmit={(n) => doRename(file, n)}
    onclose={closeModal}
  />
{/if}

{#if modal?.kind === "remove"}
  {@const fs = modal.files}
  <ConfirmRemoveDialog {account} files={fs} {inTrash} onconfirm={(mode) => doRemove(fs, mode)} onclose={closeModal} />
{/if}

{#if modal?.kind === "move"}
  {@const m = modal}
  <MoveDialog
    {account}
    files={m.files}
    driveId={m.driveId}
    rootName={rootName(m.driveId)}
    fromParent={plan.parentId}
    onsubmit={(p) => doMove(m.files, p)}
    onclose={closeModal}
  />
{/if}

{#if modal?.kind === "share"}
  <ShareDialog {account} file={modal.file} onclose={closeModal} />
{/if}

{#if modal?.kind === "emptyTrash"}
  <ConfirmDialog
    {account}
    title="Dọn sạch thùng rác"
    message="Mọi mục trong thùng rác của service account này sẽ bị xoá vĩnh viễn và không thể khôi phục."
    confirmLabel="Dọn sạch thùng rác"
    danger
    onconfirm={doEmptyTrash}
    onclose={closeModal}
  />
{/if}
