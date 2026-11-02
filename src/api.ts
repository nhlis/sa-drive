// Lớp mỏng bọc `invoke` của Tauri bằng các hàm có kiểu.
// Khi chạy `npm run dev` trong trình duyệt thường (không có Tauri), tự động dùng backend giả ở ./mock
// để làm giao diện mà không cần credentials. Nhánh này bị loại khỏi bản build production.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import type {
  About,
  AccountInfo,
  AccountsState,
  AddOutcome,
  DownloadItem,
  DriveFile,
  ExportOutcome,
  ImportResult,
  ListPage,
  ListParams,
  Permission,
  SharedDrive,
  TransferInfo,
} from "./types";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export class ApiError extends Error {
  code: string;
  hint: string | null;
  constructor(message: string, code = "error", hint: string | null = null) {
    super(message);
    this.name = "ApiError";
    this.code = code;
    this.hint = hint;
  }
}

export function toApiError(e: unknown): ApiError {
  if (e instanceof ApiError) return e;
  if (e && typeof e === "object" && "message" in e) {
    const o = e as { code?: string; message: string; hint?: string | null };
    return new ApiError(o.message, o.code ?? "error", o.hint ?? null);
  }
  return new ApiError(String(e));
}

type MockModule = typeof import("./mock");
let mockPromise: Promise<MockModule> | null = null;
function loadMock(): Promise<MockModule> {
  if (!import.meta.env.DEV) {
    return Promise.reject(new ApiError("Ứng dụng này phải chạy trong cửa sổ Tauri.", "no_tauri"));
  }
  mockPromise ??= import("./mock");
  return mockPromise;
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    if (!isTauri) {
      const m = await loadMock();
      return (await m.mockInvoke(cmd, args ?? {})) as T;
    }
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toApiError(e);
  }
}

export const api = {
  accounts: {
    list: () => call<AccountsState>("accounts_list"),
    add: (json: string, label?: string) => call<AddOutcome>("account_add", { json, label: label ?? null }),
    addFromFiles: (paths: string[]) => call<ImportResult[]>("account_add_from_files", { paths }),
    /** `impersonate`: undefined = giữ nguyên, "" = xoá, chuỗi = đặt email giả danh */
    update: (id: string, patch: { label?: string; color?: number; impersonate?: string }) =>
      call<AccountInfo>("account_update", {
        id,
        label: patch.label ?? null,
        color: patch.color ?? null,
        impersonate: patch.impersonate ?? null,
      }),
    remove: (id: string) => call<void>("account_remove", { id }),
    /** Rust mở hộp thoại lưu của hệ điều hành rồi ghi file khoá JSON. `null` = người dùng đóng hộp thoại. */
    exportKey: (id: string) => call<ExportOutcome | null>("account_export_key", { id }),
    resetVault: () => call<void>("vault_reset"),
  },
  drive: {
    about: (accountId: string) => call<About>("drive_about", { accountId }),
    sharedDrives: (accountId: string) => call<SharedDrive[]>("drive_shared_drives", { accountId }),
    list: (accountId: string, params: ListParams) => call<ListPage>("drive_list", { accountId, params }),
    createFolder: (accountId: string, name: string, parentId: string) =>
      call<DriveFile>("drive_create_folder", { accountId, name, parentId }),
    rename: (accountId: string, fileId: string, name: string) =>
      call<DriveFile>("drive_rename", { accountId, fileId, name }),
    move: (accountId: string, fileId: string, newParentId: string, oldParentIds: string[]) =>
      call<DriveFile>("drive_move", { accountId, fileId, newParentId, oldParentIds }),
    trash: (accountId: string, fileId: string) => call<void>("drive_trash", { accountId, fileId }),
    restore: (accountId: string, fileId: string) => call<void>("drive_restore", { accountId, fileId }),
    remove: (accountId: string, fileId: string) => call<void>("drive_delete", { accountId, fileId }),
    emptyTrash: (accountId: string) => call<void>("drive_empty_trash", { accountId }),
    permissions: (accountId: string, fileId: string) => call<Permission[]>("drive_permissions", { accountId, fileId }),
    shareUser: (accountId: string, fileId: string, email: string, role: string, notify: boolean) =>
      call<Permission>("drive_share_user", { accountId, fileId, email, role, notify }),
    shareAnyone: (accountId: string, fileId: string) => call<Permission>("drive_share_anyone", { accountId, fileId }),
    unshare: (accountId: string, fileId: string, permissionId: string) =>
      call<void>("drive_unshare", { accountId, fileId, permissionId }),
  },
  transfers: {
    upload: (accountId: string, parentId: string, paths: string[]) =>
      call<void>("transfer_upload", { accountId, parentId, paths }),
    download: (accountId: string, items: DownloadItem[], destDir: string) =>
      call<void>("transfer_download", { accountId, items, destDir }),
    list: () => call<TransferInfo[]>("transfer_list"),
    cancel: (id: string) => call<void>("transfer_cancel", { id }),
    cancelAll: () => call<void>("transfer_cancel_all"),
    retry: (id: string) => call<void>("transfer_retry", { id }),
    clearFinished: () => call<TransferInfo[]>("transfer_clear_finished"),
  },
};

// ───────────── sự kiện ─────────────

export async function onTransferUpdate(cb: (batch: TransferInfo[]) => void): Promise<() => void> {
  if (!isTauri) {
    const m = await loadMock();
    return m.mockOnTransfer(cb);
  }
  return listen<TransferInfo[]>("transfer-update", (e) => cb(e.payload));
}

export async function listenDragDrop(h: {
  onEnter: () => void;
  onLeave: () => void;
  onDrop: (paths: string[]) => void;
}): Promise<() => void> {
  if (!isTauri) return () => {};
  return getCurrentWebview().onDragDropEvent((ev) => {
    const p = ev.payload;
    if (p.type === "enter") h.onEnter();
    else if (p.type === "leave") h.onLeave();
    else if (p.type === "drop") {
      h.onLeave();
      h.onDrop(p.paths);
    }
  });
}

// ───────────── hộp thoại hệ thống ─────────────

export async function pickFiles(title: string): Promise<string[] | null> {
  if (!isTauri) return (await loadMock()).mockPick("files");
  const r = await openDialog({ multiple: true, directory: false, title });
  return Array.isArray(r) ? r : r ? [r] : null;
}

export async function pickFolder(title: string): Promise<string | null> {
  if (!isTauri) return (await loadMock()).mockPick("folder")?.[0] ?? null;
  const r = await openDialog({ multiple: false, directory: true, title });
  return typeof r === "string" ? r : null;
}

export async function pickJsonFiles(): Promise<string[] | null> {
  if (!isTauri) return (await loadMock()).mockPick("json");
  const r = await openDialog({
    multiple: true,
    directory: false,
    title: "Chọn file khoá JSON của service account",
    filters: [{ name: "Khoá JSON", extensions: ["json"] }],
  });
  return Array.isArray(r) ? r : r ? [r] : null;
}

export async function openExternal(url: string): Promise<void> {
  if (!/^https:\/\//i.test(url)) return; // chỉ mở liên kết https
  if (!isTauri) {
    window.open(url, "_blank", "noopener");
    return;
  }
  await openUrl(url);
}

export async function revealPath(path: string): Promise<void> {
  if (!isTauri) return;
  await revealItemInDir(path);
}
