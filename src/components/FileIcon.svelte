<script lang="ts">
  import { kindOf } from "../fileKind";
  import { FILE_PATHS, VIEW_BOX } from "../icons";

  let { mime, name, size = 20 }: { mime: string; name: string; size?: number } = $props();

  const kind = $derived(kindOf(mime, name));
</script>

<svg class="fi fi-{kind}" width={size} height={size} viewBox={VIEW_BOX} fill="currentColor" aria-hidden="true" focusable="false">
  <path d={FILE_PATHS[kind]} />
</svg>
