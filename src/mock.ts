// Backend giả chỉ dùng khi chạy giao diện trong trình duyệt thường (npm run dev, không có Tauri).
// Mô phỏng đủ hành vi để thiết kế/kiểm tra giao diện, kể cả lỗi "SA không có dung lượng".
// Không được đóng gói vào bản production (xem loadMock trong api.ts).

import type {
  About,
  AccountInfo,
  DriveFile,
  ImportResult,
  ListPage,
  ListParams,
  Permission,
  SharedDrive,
  TransferInfo,
} from "./types";
import { FOLDER_MIME } from "./types";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
let seq = 100;
const uid = () => `mock${seq++}`;
const DAY = 86_400_000;
const iso = (daysAgo: number) => new Date(Date.now() - daysAgo * DAY).toISOString();
const GB = 1024 ** 3;
const MB = 1024 ** 2;

function mk(p: Partial<DriveFile> & Pick<DriveFile, "id" | "name" | "mimeType">): DriveFile {
  return {
    size: null,
    modifiedTime: iso(3),
    createdTime: iso(30),
    parents: ["root"],
    owners: [{ displayName: "SA", emailAddress: "", me: true }],
    ownedByMe: true,
    shared: false,
    trashed: false,
    webViewLink: `https://drive.google.com/file/d/${p.id}/view`,
    driveId: null,
    md5Checksum: null,
    capabilities: {
      canDelete: true,
      canTrash: true,
      canUntrash: true,
      canRename: true,
      canEdit: true,
      canDownload: true,
      canAddChildren: true,
      canShare: true,
      canMoveItemWithinDrive: true,
    },
    shortcutDetails: null,
    ...p,
  };
}
const dir = (id: string, name: string, parent = "root", extra: Partial<DriveFile> = {}) =>
  mk({ id, name, mimeType: FOLDER_MIME, parents: [parent], ...extra });
const fil = (id: string, name: string, mime: string, size: number, parent = "root", extra: Partial<DriveFile> = {}) =>
  mk({ id, name, mimeType: mime, size, parents: [parent], ...extra });

const accounts: AccountInfo[] = [
  {
    id: "acc-1",
    label: "Backup bot",
    clientEmail: "backup-bot@acme-prod-4821.iam.gserviceaccount.com",
    projectId: "acme-prod-4821",
    privateKeyId: "3f9c1a0b7d",
    color: 0,
    impersonate: null,
    addedAt: Date.now() - 90 * DAY,
  },
  {
    id: "acc-2",
    label: "Báo cáo tự động",
    clientEmail: "reports@acme-analytics-1190.iam.gserviceaccount.com",
    projectId: "acme-analytics-1190",
    privateKeyId: "a81e22c9f0",
    color: 1,
    impersonate: null,
    addedAt: Date.now() - 20 * DAY,
  },
  {
    id: "acc-3",
    label: "Kho ảnh (DWD)",
    clientEmail: "photos@acme-media-7733.iam.gserviceaccount.com",
    projectId: "acme-media-7733",
    privateKeyId: "c70d51be42",
    color: 4,
    impersonate: "minh@acme.vn",
    addedAt: Date.now() - 5 * DAY,
  },
];

const quotas: Record<string, About> = {
  "acc-1": {
    user: { displayName: "backup-bot", emailAddress: "backup-bot@acme-prod-4821.iam.gserviceaccount.com" },
    storageQuota: { limit: 15 * GB, usage: 6.2 * GB, usageInDrive: 6.2 * GB, usageInDriveTrash: 0.3 * GB },
  },
  "acc-2": {
    user: { displayName: "reports", emailAddress: "reports@acme-analytics-1190.iam.gserviceaccount.com" },
    storageQuota: { limit: 0, usage: 0, usageInDrive: 0, usageInDriveTrash: 0 },
  },
  "acc-3": {
    user: { displayName: "Minh", emailAddress: "minh@acme.vn" },
    storageQuota: { limit: null, usage: 120 * GB, usageInDrive: 118 * GB, usageInDriveTrash: 0 },
  },
};

const drives: Record<string, SharedDrive[]> = {
  "acc-1": [],
  "acc-2": [
    { id: "SD1", name: "Kế toán 2026", colorRgb: null },
    { id: "SD2", name: "Marketing", colorRgb: null },
  ],
  "acc-3": [],
};

const PDF = "application/pdf";
const XLSX = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
const db: Record<string, DriveFile[]> = {
  "acc-1": [
    dir("b1", "Backups 2026"),
    dir("b2", "Ảnh sản phẩm", "root", { shared: true }),
    dir("b3", "Nhật ký hệ thống (4.000 file)"),
    fil("f1", "README.txt", "text/plain", 1260),
    fil("f2", "db-dump-2026-10-20.sql.gz", "application/gzip", 1.8 * GB),
    fil("f3", "cau-hinh-prod.json", "application/json", 4_820),
    fil("f4", "báo cáo tháng 9.xlsx", XLSX, 482_100),
    fil("f5", "ke-hoach-q4.pdf", PDF, 3.4 * MB, "root", { shared: true }),
    fil("f6", "demo-san-pham.mp4", "video/mp4", 214 * MB),
    fil("f7", "logo.png", "image/png", 88_400),
    fil("f8", "ghi-chu.md", "text/markdown", 2_130),
    fil("f9", "restore.sh", "application/x-sh", 920),
    dir("b4", "Lưu trữ cũ", "root", { modifiedTime: iso(200) }),
    fil("f10", "ban-ghi-am.mp3", "audio/mpeg", 9.2 * MB),
    ...[19, 18, 17, 16, 15].map((d) => fil(`t${d}`, `2026-10-${d}.tar.gz`, "application/gzip", (0.9 + d / 50) * GB, "b1", { modifiedTime: iso(21 - d + 1) })),
    ...Array.from({ length: 6 }, (_, i) => fil(`p${i}`, `san-pham-${String(i + 1).padStart(2, "0")}.jpg`, "image/jpeg", (1.1 + i / 7) * MB, "b2")),
    ...Array.from({ length: 4000 }, (_, i) =>
      fil(`l${i}`, `app-${String(i).padStart(4, "0")}.log`, "text/plain", 1_000 + ((i * 7919) % 90_000), "b3", { modifiedTime: iso(i / 100) }),
    ),
  ],
  "acc-2": [
    dir("k1", "Báo cáo quý 3", "SD1", { driveId: "SD1", owners: [] }),
    dir("k2", "Hoá đơn", "SD1", { driveId: "SD1", owners: [] }),
    fil("k3", "so-quy-thang-10.xlsx", XLSX, 1.2 * MB, "SD1", { driveId: "SD1", owners: [] }),
    fil("k4", "bao-cao-thue.pdf", PDF, 2.7 * MB, "k1", { driveId: "SD1", owners: [] }),
    fil("m1", "banner-thang-11.png", "image/png", 2.1 * MB, "SD2", { driveId: "SD2", owners: [] }),
    dir("sh1", "Tài liệu chia sẻ từ Minh", "root", {
      ownedByMe: false,
      owners: [{ displayName: "Nguyễn Văn Minh", emailAddress: "minh@acme.vn", me: false }],
      shared: true,
    }),
  ],
  "acc-3": [
    dir("c1", "Sự kiện 2026"),
    fil("c2", "danh-sach-khach-moi.xlsx", XLSX, 62_000),
    fil("c3", "thiep-moi.pdf", PDF, 1.9 * MB),
    fil("c4", "IMG_0042.jpg", "image/jpeg", 4.4 * MB, "c1"),
    mk({ id: "c5", name: "tài liệu cũ.docx", mimeType: "application/vnd.openxmlformats-officedocument.wordprocessingml.document", size: 31_000, trashed: true }),
  ],
};

const perms: Record<string, Permission[]> = {};
function permsOf(fileId: string): Permission[] {
  perms[fileId] ??= [
    { id: "p-owner", type: "user", role: "owner", emailAddress: "backup-bot@acme-prod-4821.iam.gserviceaccount.com", displayName: "backup-bot", domain: null, deleted: false, permissionDetails: null },
    { id: "p-minh", type: "user", role: "writer", emailAddress: "minh@acme.vn", displayName: "Nguyễn Văn Minh", domain: null, deleted: false, permissionDetails: null },
  ];
  return perms[fileId];
}

function find(acc: string, id: string): DriveFile {
  const f = db[acc]?.find((x) => x.id === id);
  if (!f) throw { code: "drive:notFound", message: "Google Drive báo lỗi 404: File not found.", hint: "Không tìm thấy mục này, hoặc service account chưa được chia sẻ quyền truy cập." };
  return f;
}

// ───────────── truyền tải giả ─────────────

type Listener = (b: TransferInfo[]) => void;
const listeners = new Set<Listener>();
const transfers: TransferInfo[] = [];
const timers = new Map<string, ReturnType<typeof setInterval>>();
const emit = (b: TransferInfo[]) => listeners.forEach((l) => l(b.map((x) => ({ ...x }))));

export function mockOnTransfer(cb: Listener): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

function simulate(t: TransferInfo, onDone: () => void, speed = 6 * MB) {
  t.state = "running";
  emit([t]);
  const tick = setInterval(() => {
    t.done = Math.min(t.total, t.done + speed * 0.15 * (0.6 + Math.random() * 0.8));
    if (t.done >= t.total) {
      clearInterval(tick);
      timers.delete(t.id);
      t.state = "done";
      t.done = t.total;
      onDone();
    }
    emit([t]);
  }, 150);
  timers.set(t.id, tick);
}

function newTransfer(p: Partial<TransferInfo> & Pick<TransferInfo, "accountId" | "kind" | "name" | "total">): TransferInfo {
  const t: TransferInfo = { id: uid(), done: 0, state: "queued", message: null, localPath: null, remoteId: null, parentId: null, createdAt: Date.now(), ...p };
  transfers.push(t);
  return t;
}

const QUOTA_MSG =
  "Google Drive báo lỗi 403: Service Accounts do not have storage quota. Service account này không có dung lượng lưu trữ riêng (hoặc đã đầy). SA tạo sau 15/04/2025 không sở hữu được file: hãy tải lên trong một Shared Drive mà SA là thành viên, hoặc dùng domain-wide delegation. Với SA cũ, hãy xoá bớt file do SA sở hữu.";

export function mockPick(kind: "files" | "folder" | "json"): string[] | null {
  if (kind === "json") return ["/home/demo/keys/reports-sa-key.json"];
  if (kind === "folder") return ["/home/demo/Downloads"];
  return ["/home/demo/bao-cao-q3.pdf", "/home/demo/anh-01.jpg", "/home/demo/du-lieu-lon.zip"];
}

// ───────────── điểm vào ─────────────

export async function mockInvoke(cmd: string, a: Record<string, unknown>): Promise<unknown> {
  await sleep(80 + Math.random() * 120);
  const acc = a.accountId as string;

  switch (cmd) {
    case "accounts_list":
      return { accounts: accounts.map((x) => ({ ...x })), weakKeyStorage: false };
    case "account_add":
    case "account_add_from_files": {
      const n = accounts.length + 1;
      const info: AccountInfo = {
        id: `acc-${n}`,
        label: `service-account-${n}`,
        clientEmail: `sa-${n}@demo-${1000 + n}.iam.gserviceaccount.com`,
        projectId: `demo-${1000 + n}`,
        privateKeyId: "deadbeef00",
        color: n % 8,
        impersonate: null,
        addedAt: Date.now(),
      };
      accounts.push(info);
      db[info.id] = [];
      drives[info.id] = [];
      quotas[info.id] = { user: { displayName: info.label, emailAddress: info.clientEmail }, storageQuota: { limit: 0, usage: 0, usageInDrive: 0, usageInDriveTrash: 0 } };
      if (cmd === "account_add") return { account: info, status: "added" };
      return ((a.paths as string[]) ?? []).map<ImportResult>((p) => ({ path: p, ok: true, outcome: { account: info, status: "added" }, error: null }));
    }
    case "account_update": {
      const x = accounts.find((y) => y.id === a.id);
      if (!x) throw { code: "error", message: "Không tìm thấy account." };
      if (a.label) x.label = a.label as string;
      if (a.color != null) x.color = a.color as number;
      if (a.impersonate != null) x.impersonate = (a.impersonate as string) || null;
      return { ...x };
    }
    case "account_remove":
      accounts.splice(accounts.findIndex((y) => y.id === a.id), 1);
      return null;
    case "account_export_key": {
      const x = accounts.find((y) => y.id === a.id);
      if (!x) throw { code: "error", message: "Không tìm thấy account (có thể đã bị xoá)." };
      await sleep(400); // hộp thoại lưu của hệ điều hành
      return { path: `/home/demo/Downloads/${x.clientEmail.split("@")[0]}-${x.privateKeyId.slice(0, 8)}.json` };
    }
    case "vault_reset":
      return null;

    case "drive_about":
      return quotas[acc];
    case "drive_shared_drives":
      return drives[acc] ?? [];
    case "drive_list": {
      const p = a.params as ListParams;
      const all = db[acc] ?? [];
      let out: DriveFile[];
      if (p.mode === "children") {
        const parent = p.parentId || "root";
        out = all.filter((f) => !f.trashed && f.parents.includes(parent));
        if (p.foldersOnly) out = out.filter((f) => f.mimeType === FOLDER_MIME);
      } else if (p.mode === "owned") out = all.filter((f) => !f.trashed && f.ownedByMe && !f.driveId);
      else if (p.mode === "shared") out = all.filter((f) => !f.trashed && f.ownedByMe === false);
      else if (p.mode === "trash") out = all.filter((f) => f.trashed);
      else out = all.filter((f) => !f.trashed && f.name.toLowerCase().includes((p.query ?? "").toLowerCase()));
      const start = p.pageToken ? Number(p.pageToken) : 0;
      const page = out.slice(start, start + 200);
      const next = start + 200 < out.length ? String(start + 200) : null;
      return { files: page, nextPageToken: next } satisfies ListPage;
    }
    case "drive_create_folder": {
      const parent = (a.parentId as string) || "root";
      const inDrive = drives[acc]?.find((d) => d.id === parent) ?? db[acc].find((f) => f.id === parent && f.driveId);
      const f = dir(uid(), a.name as string, parent, inDrive ? { driveId: (inDrive as DriveFile).driveId ?? (inDrive as SharedDrive).id, owners: [] } : {});
      db[acc].push(f);
      return f;
    }
    case "drive_rename": {
      const f = find(acc, a.fileId as string);
      f.name = a.name as string;
      return f;
    }
    case "drive_move": {
      const f = find(acc, a.fileId as string);
      f.parents = [a.newParentId as string];
      return f;
    }
    case "drive_trash":
      find(acc, a.fileId as string).trashed = true;
      return null;
    case "drive_restore":
      find(acc, a.fileId as string).trashed = false;
      return null;
    case "drive_delete":
      db[acc].splice(db[acc].indexOf(find(acc, a.fileId as string)), 1);
      return null;
    case "drive_empty_trash":
      db[acc] = db[acc].filter((f) => !f.trashed);
      return null;
    case "drive_permissions":
      return permsOf(a.fileId as string).map((p) => ({ ...p }));
    case "drive_share_user": {
      const p: Permission = { id: uid(), type: "user", role: a.role as string, emailAddress: a.email as string, displayName: null, domain: null, deleted: false, permissionDetails: null };
      permsOf(a.fileId as string).push(p);
      return p;
    }
    case "drive_share_anyone": {
      const p: Permission = { id: "anyoneWithLink", type: "anyone", role: "reader", emailAddress: null, displayName: null, domain: null, deleted: false, permissionDetails: null };
      permsOf(a.fileId as string).push(p);
      return p;
    }
    case "drive_unshare": {
      const l = permsOf(a.fileId as string);
      l.splice(l.findIndex((p) => p.id === a.permissionId), 1);
      return null;
    }

    case "transfer_upload": {
      const parent = (a.parentId as string) || "root";
      const created = (a.paths as string[]).map((path, i) => {
        const name = path.split("/").pop() ?? path;
        const big = name.includes("lon");
        const t = newTransfer({ accountId: acc, kind: "upload", name, total: big ? 640 * MB : (3 + i * 9) * MB, localPath: path, parentId: parent });
        return t;
      });
      emit(created);
      const noQuota = quotas[acc]?.storageQuota?.limit === 0;
      const inDrive = !!drives[acc]?.some((d) => d.id === parent) || !!db[acc]?.find((f) => f.id === parent)?.driveId;
      created.forEach((t, i) =>
        setTimeout(() => {
          if (noQuota && !inDrive) {
            t.state = "error";
            t.message = QUOTA_MSG;
            emit([t]);
            return;
          }
          simulate(t, () => {
            const f = fil(uid(), t.name, "application/octet-stream", t.total, parent, inDrive ? { driveId: drives[acc]?.[0]?.id ?? null, owners: [] } : {});
            db[acc].push(f);
            t.remoteId = f.id;
          });
        }, 300 + i * 400),
      );
      return null;
    }
    case "transfer_download": {
      const items = a.items as { id: string; name: string; size: number | null }[];
      const created = items.map((it) => newTransfer({ accountId: acc, kind: "download", name: it.name, total: it.size ?? 0, localPath: `${a.destDir}/${it.name}` }));
      emit(created);
      created.forEach((t, i) => setTimeout(() => simulate(t, () => {}, 14 * MB), 200 + i * 300));
      return null;
    }
    case "transfer_list":
      return transfers.map((t) => ({ ...t }));
    case "transfer_cancel": {
      const t = transfers.find((x) => x.id === a.id);
      if (t && (t.state === "running" || t.state === "queued")) {
        clearInterval(timers.get(t.id));
        t.state = "cancelled";
        emit([t]);
      }
      return null;
    }
    case "transfer_cancel_all":
      transfers.filter((t) => t.state === "running" || t.state === "queued").forEach((t) => {
        clearInterval(timers.get(t.id));
        t.state = "cancelled";
      });
      emit(transfers);
      return null;
    case "transfer_retry": {
      const t = transfers.find((x) => x.id === a.id);
      if (t) {
        t.state = "queued";
        t.done = 0;
        t.message = null;
        emit([t]);
        simulate(t, () => {});
      }
      return null;
    }
    case "transfer_clear_finished": {
      for (let i = transfers.length - 1; i >= 0; i--) if (["done", "error", "cancelled"].includes(transfers[i].state)) transfers.splice(i, 1);
      return transfers.map((t) => ({ ...t }));
    }
    default:
      throw { code: "error", message: `Lệnh giả chưa hỗ trợ: ${cmd}` };
  }
}
