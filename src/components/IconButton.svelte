<script lang="ts">
  import type { HTMLButtonAttributes } from "svelte/elements";
  import type { IconName } from "../icons";
  import Icon from "./Icon.svelte";

  interface Props extends Omit<HTMLButtonAttributes, "children" | "aria-label"> {
    icon: IconName;
    /** Nhãn cho trình đọc màn hình và tooltip */
    label: string;
    small?: boolean;
  }

  let { icon, label, small = false, type = "button", ...rest }: Props = $props();
</script>

<button {type} class="md-icon-btn" class:sm={small} aria-label={label} title={label} {...rest}>
  <Icon name={icon} size={small ? 16 : 20} />
</button>
