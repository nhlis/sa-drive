/** Chạy `fn` trên từng phần tử với tối đa `limit` tác vụ song song; không bao giờ ném lỗi (trả về settled results). */
export async function mapPool<T, R>(
  items: readonly T[],
  limit: number,
  fn: (item: T, index: number) => Promise<R>,
  onStep?: (finished: number) => void,
): Promise<PromiseSettledResult<R>[]> {
  const out: PromiseSettledResult<R>[] = new Array(items.length);
  let next = 0;
  let finished = 0;
  const worker = async () => {
    for (;;) {
      const i = next++;
      if (i >= items.length) return;
      try {
        out[i] = { status: "fulfilled", value: await fn(items[i], i) };
      } catch (reason) {
        out[i] = { status: "rejected", reason };
      }
      onStep?.(++finished);
    }
  };
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, worker));
  return out;
}

export const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]{2,}$/;

/** Thông báo lỗi kèm gợi ý khắc phục (nếu có) trên một dòng. */
export function errorText(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) {
    const o = e as { message: unknown; hint?: unknown };
    const m = String(o.message);
    return typeof o.hint === "string" && o.hint ? `${m} ${o.hint}` : m;
  }
  return String(e);
}
