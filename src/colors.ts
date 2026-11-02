// Màu của từng account: primary, secondary và các container của chúng (token --md-sys-color-* của MD3); phần trung tính cố định nằm trong tokens.css.

export interface AccountColor {
  name: string;
  light: string;
  dark: string;
}

export const PALETTE: AccountColor[] = [
  { name: "Lam ngọc", light: "#0B7F78", dark: "#38C2B8" },
  { name: "Xanh coban", light: "#2D56D6", dark: "#7C9BFF" },
  { name: "Tím", light: "#7140C4", dark: "#B28CFF" },
  { name: "Hồng sẫm", light: "#BC2E78", dark: "#F27DB5" },
  { name: "Cam đỏ", light: "#C7421F", dark: "#FF8A66" },
  { name: "Hổ phách", light: "#A06208", dark: "#F0B24B" },
  { name: "Rêu", light: "#4B7A1D", dark: "#9BCB5E" },
  { name: "Xám lam", light: "#4B6076", dark: "#9DB3C9" },
];

type RGB = [number, number, number];

const hexToRgb = (hex: string): RGB => {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
};
const toLinear = (c: number) => {
  const s = c / 255;
  return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
};
const fromLinear = (v: number) => 255 * (v <= 0.0031308 ? v * 12.92 : 1.055 * v ** (1 / 2.4) - 0.055);
const luminance = (hex: string) => {
  const [r, g, b] = hexToRgb(hex).map(toLinear);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
const lightness = (y: number) => (y > 216 / 24389 ? 116 * Math.cbrt(y) - 16 : (24389 / 27) * y); // CIE L* (= tone của M3)

function contrast(a: string, b: string): number {
  const [la, lb] = [luminance(a), luminance(b)];
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

// OKLCH: đổi độ sáng mà giữ hue

function toOklch(hex: string): { hue: number; chroma: number } {
  const [r, g, b] = hexToRgb(hex).map(toLinear);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  const a = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
  const bb = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  return { hue: Math.atan2(bb, a), chroma: Math.hypot(a, bb) };
}

function oklabToLinear(L: number, a: number, b: number): RGB {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s, -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s];
}

const toHex = (lin: RGB) =>
  "#" + lin.map((v) => Math.round(Math.min(255, Math.max(0, fromLinear(Math.min(1, Math.max(0, v)))))).toString(16).padStart(2, "0")).join("");

// Cùng hue, L* = tone; giảm chroma nếu vượt gam sRGB
function toneColor(hue: number, chroma: number, tone: number): string {
  if (tone >= 100) return "#ffffff";
  if (tone <= 0) return "#000000";
  for (let c = chroma; c >= 0; c -= 0.002) {
    const [a, b] = [c * Math.cos(hue), c * Math.sin(hue)];
    let [lo, hi] = [0, 1];
    for (let i = 0; i < 22; i++) {
      const mid = (lo + hi) / 2;
      const [r, g, bl] = oklabToLinear(mid, a, b);
      if (lightness(0.2126 * r + 0.7152 * g + 0.0722 * bl) < tone) lo = mid;
      else hi = mid;
    }
    const rgb = oklabToLinear((lo + hi) / 2, a, b);
    if (rgb.every((v) => v >= -0.0005 && v <= 1.0005)) return toHex(rgb);
  }
  return "#808080";
}

// Đổi tone cho tới khi tương phản với bg đạt min: tối dần trên nền sáng, sáng dần trên nền tối
function fitContrast(hex: string, bg: string, min: number, dark: boolean): string {
  if (contrast(hex, bg) >= min) return hex;
  const { hue, chroma } = toOklch(hex);
  const from = lightness(luminance(hex));
  for (let tone = from + (dark ? 0.5 : -0.5); dark ? tone < 98 : tone > 5; tone += dark ? 0.5 : -0.5) {
    const c = toneColor(hue, chroma, tone);
    if (contrast(c, bg) >= min) return c;
  }
  return hex;
}

export function accountColor(index: number, dark: boolean): string {
  const c = PALETTE[((index % PALETTE.length) + PALETTE.length) % PALETTE.length];
  return dark ? c.dark : c.light;
}

const INK_DARK = "#10151A";
const INK_LIGHT = "#FFFFFF";

// Chữ trên nền màu: trắng hoặc gần đen, màu nào tương phản hơn
const inkFor = (hex: string) => (contrast(hex, INK_LIGHT) >= contrast(hex, INK_DARK) ? INK_LIGHT : INK_DARK);

export const accountInk = (index: number, dark: boolean) => inkFor(accountColor(index, dark));

// Bề mặt đậm nhất của chế độ hiện tại: primary phải đọc rõ trên đó thì đọc rõ trên mọi bề mặt còn lại
function referenceSurface(dark: boolean): string {
  const v = getComputedStyle(document.documentElement).getPropertyValue("--md-sys-color-surface-container-highest").trim();
  return /^#[0-9a-f]{6}$/i.test(v) ? v : dark ? "#333537" : "#e0e3e6";
}

// Đặt các vai trò màu của account lên <html>. Tính bằng JS, không dùng color-mix vì WebView cũ
export function applyAccountTheme(index: number, dark: boolean): void {
  const hex = accountColor(index, dark);
  const root = document.documentElement.style;
  const { hue, chroma } = toOklch(hex);
  const tone = (c: number, t: number) => toneColor(hue, Math.min(chroma, c), t);

  const primary = fitContrast(hex, referenceSurface(dark), 4.8, dark);
  root.setProperty("--md-sys-color-primary", primary);
  root.setProperty("--md-sys-color-on-primary", inkFor(primary));
  // Container: tone 90 (sáng) / 30 (tối); chữ trên container: tone 10 / 90
  const [fill, on] = dark ? [30, 90] : [90, 10];
  root.setProperty("--md-sys-color-primary-container", tone(0.04, fill));
  root.setProperty("--md-sys-color-on-primary-container", tone(0.04, on));
  root.setProperty("--md-sys-color-secondary", tone(0.02, dark ? 80 : 40));
  root.setProperty("--md-sys-color-on-secondary", tone(0.02, dark ? 20 : 100));
  root.setProperty("--md-sys-color-secondary-container", tone(0.02, fill));
  root.setProperty("--md-sys-color-on-secondary-container", tone(0.02, on));
  root.setProperty("--md-sys-color-inverse-primary", tone(0.04, dark ? 40 : 80));
}

// Chưa chọn account: màu xám lam
export function applyNeutralTheme(dark: boolean): void {
  applyAccountTheme(7, dark);
}

export function prefersDark(): boolean {
  return typeof window !== "undefined" && window.matchMedia?.("(prefers-color-scheme: dark)").matches;
}
