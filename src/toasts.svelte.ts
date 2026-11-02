import { toApiError } from "./api";

export interface Toast {
  id: number;
  kind: "ok" | "err" | "info";
  text: string;
  hint?: string | null;
}

class ToastStore {
  items = $state.raw<Toast[]>([]);
  #seq = 1;

  dismiss = (id: number): void => {
    this.items = this.items.filter((t) => t.id !== id);
  };

  #push(kind: Toast["kind"], text: string, hint?: string | null): void {
    const id = this.#seq++;
    this.items = [...this.items.slice(-3), { id, kind, text, hint }];
    window.setTimeout(() => this.dismiss(id), kind === "err" ? 14000 : 4500);
  }

  ok = (text: string): void => this.#push("ok", text);
  info = (text: string): void => this.#push("info", text);

  /** Hiện lỗi từ backend: thông báo + gợi ý cách khắc phục (nếu có). */
  err = (e: unknown, prefix?: string): void => {
    const a = toApiError(e);
    this.#push("err", prefix ? `${prefix}: ${a.message}` : a.message, a.hint);
  };
}

export const toasts = new ToastStore();
