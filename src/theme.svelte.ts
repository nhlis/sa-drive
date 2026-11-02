import { prefersDark } from "./colors";

/** Chế độ sáng/tối của hệ điều hành, theo dõi realtime. */
class Theme {
  dark = $state(prefersDark());

  constructor() {
    const mq = typeof window !== "undefined" ? window.matchMedia?.("(prefers-color-scheme: dark)") : undefined;
    mq?.addEventListener("change", () => {
      this.dark = mq.matches;
    });
  }
}

export const theme = new Theme();
