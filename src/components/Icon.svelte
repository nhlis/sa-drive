<script lang="ts">
  import { PATHS, VIEW_BOX, type IconName } from "../icons";

  let { name, size = 16, class: className = "" }: { name: IconName; size?: number; class?: string } = $props();
</script>

<svg class={className} width={size} height={size} viewBox={VIEW_BOX} fill="currentColor" aria-hidden="true" focusable="false">
  <path d={PATHS[name]} />
</svg>
