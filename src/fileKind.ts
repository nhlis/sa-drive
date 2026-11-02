import { FOLDER_MIME, SHORTCUT_MIME } from "./types";

export type Kind =
  | "folder"
  | "shortcut"
  | "image"
  | "video"
  | "audio"
  | "archive"
  | "pdf"
  | "doc"
  | "sheet"
  | "slide"
  | "code"
  | "text"
  | "other";

const EXT: Record<string, Kind> = {
  jpg: "image", jpeg: "image", png: "image", gif: "image", webp: "image", svg: "image", bmp: "image", heic: "image", avif: "image",
  mp4: "video", mkv: "video", mov: "video", avi: "video", webm: "video",
  mp3: "audio", wav: "audio", flac: "audio", m4a: "audio", ogg: "audio",
  zip: "archive", gz: "archive", tgz: "archive", tar: "archive", "7z": "archive", rar: "archive", bz2: "archive", xz: "archive",
  pdf: "pdf",
  doc: "doc", docx: "doc", odt: "doc", rtf: "doc",
  xls: "sheet", xlsx: "sheet", csv: "sheet", ods: "sheet",
  ppt: "slide", pptx: "slide", odp: "slide",
  js: "code", ts: "code", tsx: "code", jsx: "code", json: "code", py: "code", rs: "code", go: "code", java: "code", c: "code",
  cpp: "code", h: "code", css: "code", html: "code", sh: "code", sql: "code", yml: "code", yaml: "code", xml: "code", toml: "code",
  txt: "text", md: "text", log: "text",
};

export function kindOf(mime: string, name: string): Kind {
  if (mime === FOLDER_MIME) return "folder";
  if (mime === SHORTCUT_MIME) return "shortcut";
  if (mime.startsWith("image/")) return "image";
  if (mime.startsWith("video/")) return "video";
  if (mime.startsWith("audio/")) return "audio";
  if (mime === "application/pdf") return "pdf";
  if (mime === "application/vnd.google-apps.document") return "doc";
  if (mime === "application/vnd.google-apps.spreadsheet") return "sheet";
  if (mime === "application/vnd.google-apps.presentation") return "slide";
  const ext = name.includes(".") ? name.split(".").pop()!.toLowerCase() : "";
  return EXT[ext] ?? (mime.startsWith("text/") ? "text" : "other");
}
