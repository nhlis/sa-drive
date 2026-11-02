import { api, onTransferUpdate, toApiError } from "./api";
import type { TransferInfo } from "./types";

type DoneListener = (t: TransferInfo) => void;

/**
 * Gom danh sách truyền tải từ backend và các sự kiện cập nhật (backend đã giãn tần suất,
 * ở đây gom thêm theo nhịp 100 ms để giao diện không bị vẽ lại liên tục).
 */
class TransferStore {
  items = $state.raw<TransferInfo[]>([]);

  #speeds = new Map<string, { t: number; done: number; bps: number }>();
  #lastState = new Map<string, string>();
  #listeners = new Set<DoneListener>();
  #buffer: TransferInfo[] = [];
  #timer: number | null = null;
  #started = false;

  /** Bắt đầu nhận sự kiện. An toàn khi gọi nhiều lần. */
  start = (): void => {
    if (this.#started) return;
    this.#started = true;

    onTransferUpdate((batch) => {
      for (const t of batch) this.#buffer.push(t);
      if (this.#timer == null) this.#timer = window.setTimeout(this.#flush, 100);
    }).catch(() => {});

    api.transfers
      .list()
      .then((list) => {
        // Sự kiện có thể đã đến trước khi danh sách trả về: ưu tiên bản mới hơn.
        const byId = new Map(list.map((t) => [t.id, t] as const));
        for (const t of this.items) byId.set(t.id, t);
        for (const t of byId.values()) if (!this.#lastState.has(t.id)) this.#lastState.set(t.id, t.state);
        this.items = Array.from(byId.values());
      })
      .catch(() => {});
  };

  #flush = (): void => {
    this.#timer = null;
    const batch = this.#buffer;
    this.#buffer = [];
    if (batch.length === 0) return;
    const now = performance.now();
    const justDone: TransferInfo[] = [];

    for (const t of batch) {
      const prev = this.#lastState.get(t.id);
      if (t.state === "done" && prev !== "done") justDone.push(t);
      this.#lastState.set(t.id, t.state);

      if (t.state === "running") {
        const s = this.#speeds.get(t.id);
        if (s && now > s.t) {
          const inst = ((t.done - s.done) / (now - s.t)) * 1000;
          if (inst >= 0) s.bps = s.bps === 0 ? inst : s.bps * 0.7 + inst * 0.3;
          s.t = now;
          s.done = t.done;
        } else if (!s) {
          this.#speeds.set(t.id, { t: now, done: t.done, bps: 0 });
        }
      } else {
        this.#speeds.delete(t.id);
      }
    }

    const next = this.items.slice();
    const index = new Map<string, number>();
    next.forEach((t, i) => index.set(t.id, i));
    for (const t of batch) {
      const i = index.get(t.id);
      if (i === undefined) {
        index.set(t.id, next.length);
        next.push(t);
      } else {
        next[i] = t;
      }
    }
    this.items = next;
    for (const t of justDone) this.#listeners.forEach((l) => l(t));
  };

  /** Tốc độ trung bình trượt (byte/giây) của một truyền tải đang chạy. */
  speedOf = (id: string): number => this.#speeds.get(id)?.bps ?? 0;

  cancel = (id: string): void => void api.transfers.cancel(id).catch(() => {});
  cancelAll = (): void => void api.transfers.cancelAll().catch(() => {});

  retry = async (id: string): Promise<void> => {
    try {
      await api.transfers.retry(id);
    } catch (e) {
      throw toApiError(e);
    }
  };

  clearFinished = async (): Promise<void> => {
    const rest = await api.transfers.clearFinished();
    const keep = new Set(rest.map((t) => t.id));
    for (const id of Array.from(this.#lastState.keys())) if (!keep.has(id)) this.#lastState.delete(id);
    rest.forEach((t) => this.#lastState.set(t.id, t.state));
    this.items = rest;
  };

  /** Gọi khi một truyền tải vừa chuyển sang "done". Trả về hàm huỷ đăng ký. */
  onDone = (l: DoneListener): (() => void) => {
    this.#listeners.add(l);
    return () => void this.#listeners.delete(l);
  };
}

export const transfers = new TransferStore();
