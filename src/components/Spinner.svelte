<script lang="ts">
  /** Vòng tiến trình không xác định (circular indeterminate) của MD3. */
  let { size = 16 }: { size?: number } = $props();
</script>

<span class="spin" style:--spin-size="{size}px" aria-hidden="true">
  <svg viewBox="0 0 48 48" focusable="false"><circle cx="24" cy="24" r="20" /></svg>
</span>
