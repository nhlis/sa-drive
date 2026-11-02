import type { DriveFile, Sort } from "./types";
import { FOLDER_MIME } from "./types";

const nf1 = new Intl.NumberFormat("vi-VN", { maximumFractionDigits: 1 });
const nf0 = new Intl.NumberFormat("vi-VN", { maximumFractionDigits: 0 });

export function formatBytes(n: number | null | undefined): string {
  if (n == null) return "—";
  if (n < 1024) return `${nf0.format(n)} B`;
  const units = ["KB", "MB", "GB", "TB", "PB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 ? nf1.format(v) : nf0.format(v)} ${units[i]}`;
}

const pad = (n: number) => String(n).padStart(2, "0");

export function formatDate(iso: string | null | undefined): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "—";
  return `${pad(d.getDate())}/${pad(d.getMonth() + 1)}/${d.getFullYear()} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function formatSpeed(bps: number): string {
  return `${formatBytes(bps)}/s`;
}

export function formatEta(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "";
  if (seconds < 60) return `${Math.ceil(seconds)} giây`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)} phút ${pad(Math.floor(seconds % 60))} giây`;
  return `${Math.floor(seconds / 3600)} giờ ${pad(Math.floor((seconds % 3600) / 60))} phút`;
}

export function isFolder(f: Pick<DriveFile, "mimeType">): boolean {
  return f.mimeType === FOLDER_MIME;
}

const collator = new Intl.Collator("vi", { numeric: true, sensitivity: "base" });

/** Thư mục luôn đứng trước; sau đó sắp theo cột được chọn. */
export function sortFiles(files: DriveFile[], sort: Sort): DriveFile[] {
  const dir = sort.dir === "asc" ? 1 : -1;
  const out = files.slice();
  out.sort((a, b) => {
    const fa = isFolder(a) ? 0 : 1;
    const fb = isFolder(b) ? 0 : 1;
    if (fa !== fb) return fa - fb;
    let c = 0;
    if (sort.key === "name") c = collator.compare(a.name, b.name);
    else if (sort.key === "size") c = (a.size ?? -1) - (b.size ?? -1);
    else c = (Date.parse(a.modifiedTime ?? "") || 0) - (Date.parse(b.modifiedTime ?? "") || 0);
    if (c === 0) c = collator.compare(a.name, b.name);
    return c * dir;
  });
  return out;
}

export function ownerLabel(f: DriveFile): string {
  if (f.driveId && f.owners.length === 0) return "Shared Drive";
  const o = f.owners[0];
  if (!o) return "—";
  if (o.me || f.ownedByMe) return "SA này";
  return o.displayName || o.emailAddress || "—";
}

export function plural(n: number, one: string): string {
  return `${nf0.format(n)} ${one}`;
}

/** Tách phần tên và đuôi để chọn sẵn phần tên khi đổi tên file. */
export function stemLength(name: string): number {
  const i = name.lastIndexOf(".");
  return i > 0 ? i : name.length;
}

export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    try {
      const ta = document.createElement("textarea");
      ta.value = text;
      ta.style.position = "fixed";
      ta.style.opacity = "0";
      document.body.appendChild(ta);
      ta.select();
      const ok = document.execCommand("copy");
      ta.remove();
      return ok;
    } catch {
      return false;
    }
  }
}
