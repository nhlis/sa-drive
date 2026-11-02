<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";
  import type { IconName } from "../icons";
  import Icon from "./Icon.svelte";

  interface Props extends Omit<HTMLButtonAttributes, "children"> {
    variant?: "filled" | "tonal" | "outlined" | "text";
    /** Hành động phá huỷ: dùng màu error thay cho primary */
    error?: boolean;
    /** Chiều cao: 40px mặc định; "compact" 36px và "dense" 32px theo thang mật độ của MD3 */
    size?: "default" | "compact" | "dense";
    icon?: IconName;
    children: Snippet;
  }

  let { variant = "text", error = false, size = "default", icon, children, type = "button", ...rest }: Props = $props();
</script>

<button {type} class="md-btn {variant}" class:error class:compact={size === "compact"} class:dense={size === "dense"} class:with-icon={icon} {...rest}>
  {#if icon}<Icon name={icon} size={18} />{/if}
  {@render children()}
</button>
