<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    label: string;
    value?: string;
    disabled?: boolean;
    children: Snippet;
  }

  let { label, value = $bindable(""), disabled = false, children }: Props = $props();

  const id = $props.id();
  let focused = $state(false);
</script>

<div class="md-field floating" class:focused class:disabled>
  <div class="md-field-box">
    <select {id} bind:value class="md-field-input md-select" {disabled} onfocus={() => (focused = true)} onblur={() => (focused = false)}>
      {@render children()}
    </select>
    <label class="md-field-label" for={id}>{label}</label>
    <fieldset class="md-field-outline" aria-hidden="true"><legend><span>{label}</span></legend></fieldset>
    <span class="md-select-arrow"><Icon name="chevronDown" size={20} /></span>
  </div>
</div>
