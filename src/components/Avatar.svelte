<script lang="ts">
  import { accountColor, accountInk } from "../colors";
  import { theme } from "../theme.svelte";
  import type { AccountInfo } from "../types";

  /** Ảnh đại diện của service account: chữ cái đầu của tên hiển thị trên nền màu nhận diện. */
  let { account, size = 32 }: { account: AccountInfo; size?: number } = $props();

  // Array.from tách theo ký tự thật, không cắt đôi emoji hay chữ ngoài BMP.
  const initial = $derived((Array.from(account.label.trim())[0] ?? "?").toUpperCase());
</script>

<span
  class="avatar"
  aria-hidden="true"
  style:--size="{size}px"
  style:--av-bg={accountColor(account.color, theme.dark)}
  style:--av-fg={accountInk(account.color, theme.dark)}
>{initial}</span>
