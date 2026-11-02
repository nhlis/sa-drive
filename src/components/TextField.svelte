<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    label: string;
    value?: string;
    type?: "text" | "email";
    /** > 0: nhiều dòng */
    rows?: number;
    placeholder?: string;
    support?: string;
    error?: string | null;
    disabled?: boolean;
    readonly?: boolean;
    mono?: boolean;
    el?: HTMLInputElement | HTMLTextAreaElement;
    onfocus?: (e: FocusEvent) => void;
    trailing?: Snippet;
  }

  let {
    label,
    value = $bindable(""),
    type = "text",
    rows = 0,
    placeholder,
    support,
    error = null,
    disabled = false,
    readonly = false,
    mono = false,
    el = $bindable(),
    onfocus,
    trailing,
  }: Props = $props();

  const id = $props.id();
  let focused = $state(false);
  const floating = $derived(focused || value !== "");
  const note = $derived(error ?? support);

  function handleFocus(e: FocusEvent) {
    focused = true;
    onfocus?.(e);
  }
</script>

<div class="md-field" class:focused class:floating class:invalid={!!error} class:disabled class:mono>
  <div class="md-field-box">
    {#if rows > 0}
      <textarea
        {id}
        bind:this={el}
        bind:value
        class="md-field-input"
        {rows}
        {placeholder}
        {disabled}
        {readonly}
        spellcheck="false"
        autocomplete="off"
        aria-invalid={!!error}
        aria-describedby={note ? `${id}-note` : undefined}
        onfocus={handleFocus}
        onblur={() => (focused = false)}
      ></textarea>
    {:else}
      <input
        {id}
        bind:this={el}
        bind:value
        class="md-field-input"
        {type}
        {placeholder}
        {disabled}
        {readonly}
        spellcheck="false"
        autocomplete="off"
        aria-invalid={!!error}
        aria-describedby={note ? `${id}-note` : undefined}
        onfocus={handleFocus}
        onblur={() => (focused = false)}
      />
    {/if}
    <label class="md-field-label" for={id}>{label}</label>
    <fieldset class="md-field-outline" aria-hidden="true"><legend><span>{label}</span></legend></fieldset>
    {#if trailing}{@render trailing()}{/if}
  </div>
  {#if note}<p class="md-field-support" id="{id}-note">{note}</p>{/if}
</div>
