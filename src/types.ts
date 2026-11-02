// Kiểu dữ liệu dùng chung, khớp với JSON mà backend Rust trả về (camelCase).

export interface AccountInfo {
  id: string;
  label: string;
  clientEmail: string;
  projectId: string;
  privateKeyId: string;
  color: number;
  impersonate: string | null;
  addedAt: number;
}

export interface AccountsState {
  accounts: AccountInfo[];
  /** true khi master key phải nằm trong file vì keychain hệ điều hành không dùng được */
  weakKeyStorage: boolean;
}

export interface AddOutcome {
  account: AccountInfo;
  status: "added" | "updated";
}

export interface ExportOutcome {
  /** Nơi người dùng đã chọn lưu file khoá */
  path: string;
}

export interface ImportResult {
  path: string;
  ok: boolean;
  outcome: AddOutcome | null;
  error: string | null;
}

export interface Owner {
  displayName?: string | null;
  emailAddress?: string | null;
  me?: boolean | null;
}

export interface Capabilities {
  canDelete?: boolean | null;
  canTrash?: boolean | null;
  canUntrash?: boolean | null;
  canRename?: boolean | null;
  canEdit?: boolean | null;
  canDownload?: boolean | null;
  canAddChildren?: boolean | null;
  canShare?: boolean | null;
  canMoveItemWithinDrive?: boolean | null;
}

export interface DriveFile {
  id: string;
  name: string;
  mimeType: string;
  size: number | null;
  modifiedTime: string | null;
  createdTime: string | null;
  parents: string[];
  owners: Owner[];
  ownedByMe: boolean | null;
  shared: boolean;
  trashed: boolean;
  webViewLink: string | null;
  driveId: string | null;
  md5Checksum: string | null;
  capabilities: Capabilities | null;
  shortcutDetails: { targetId: string | null; targetMimeType: string | null } | null;
}

export interface ListPage {
  files: DriveFile[];
  nextPageToken: string | null;
}

export type ListMode = "children" | "owned" | "shared" | "trash" | "search";

export interface ListParams {
  mode: ListMode;
  parentId?: string | null;
  driveId?: string | null;
  query?: string | null;
  foldersOnly?: boolean | null;
  pageToken?: string | null;
}

export interface SharedDrive {
  id: string;
  name: string;
  colorRgb: string | null;
}

export interface About {
  user: { displayName: string | null; emailAddress: string | null } | null;
  storageQuota: {
    /** null = không giới hạn */
    limit: number | null;
    usage: number | null;
    usageInDrive: number | null;
    usageInDriveTrash: number | null;
  } | null;
}

export interface Permission {
  id: string;
  type: string; // user | group | domain | anyone
  role: string; // owner | organizer | fileOrganizer | writer | commenter | reader
  emailAddress: string | null;
  displayName: string | null;
  domain: string | null;
  deleted: boolean | null;
  permissionDetails: { inherited: boolean | null; permissionType: string | null; role: string | null }[] | null;
}

export type TransferKind = "upload" | "download";
export type TransferState = "queued" | "running" | "done" | "error" | "cancelled";

export interface TransferInfo {
  id: string;
  accountId: string;
  kind: TransferKind;
  name: string;
  total: number;
  done: number;
  state: TransferState;
  message: string | null;
  localPath: string | null;
  remoteId: string | null;
  parentId: string | null;
  createdAt: number;
}

export interface DownloadItem {
  id: string;
  name: string;
  mimeType: string;
  size: number | null;
  driveId: string | null;
}

export const FOLDER_MIME = "application/vnd.google-apps.folder";
export const SHORTCUT_MIME = "application/vnd.google-apps.shortcut";

// ───── điều hướng ─────

export type Loc =
  | { kind: "my" }
  | { kind: "owned" }
  | { kind: "shared" }
  | { kind: "trash" }
  | { kind: "drive"; id: string; name: string }
  | { kind: "search"; query: string; driveId: string | null; prev: Browse };

export interface Crumb {
  id: string;
  name: string;
  driveId: string | null;
}

export interface Browse {
  loc: Loc;
  /** Các thư mục đã mở bên dưới gốc của `loc` */
  trail: Crumb[];
}

export type SortKey = "name" | "size" | "modified";
export interface Sort {
  key: SortKey;
  dir: "asc" | "desc";
}

// ───── trạng thái giao diện dùng chung ─────

export type HelpSection = "quota" | "setup" | "dwd" | "security";

export type TableCommand = "delete" | "rename" | "up";

export interface DrivesState {
  list: SharedDrive[];
  loading: boolean;
  /** Lỗi tải danh sách Shared Drive (ApiError thoả cấu trúc này) */
  error: { message: string; hint: string | null } | null;
}
