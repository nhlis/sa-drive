<script lang="ts">
  import { onMount } from "svelte";
  import Button from "../components/Button.svelte";
  import Modal from "../components/Modal.svelte";
  import TextField from "../components/TextField.svelte";
  import { stemLength } from "../format";
  import { errorText } from "../util";

  interface Props {
    title: string;
    label: string;
    initial?: string;
    confirmLabel?: string;
    /** Chọn sẵn phần tên, không chọn đuôi file */
    selectStem?: boolean;
    onsubmit: (value: string) => Promise<void>;
    onclose: () => void;
  }

  let { title, label, initial = "", confirmLabel = "Lưu", selectStem = false, onsubmit, onclose }: Props = $props();

  // svelte-ignore state_referenced_locally
  let value = $state(initial);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let input = $state<HTMLInputElement | HTMLTextAreaElement>();

  onMount(() => {
    input?.focus();
    input?.setSelectionRange(0, selectStem ? stemLength(initial) : initial.length);
  });

  async function submit(e: Event) {
    e.preventDefault();
    const v = value.trim();
    if (!v) {
      error = "Tên không được để trống.";
      return;
    }
    busy = true;
    error = null;
    try {
      await onsubmit(v);
      onclose();
    } catch (err) {
      error = errorText(err);
      busy = false;
    }
  }
</script>

<Modal {title} {onclose} {busy} width={440}>
  <form id="prompt-form" class="md-form" onsubmit={submit}>
    <TextField bind:el={input} {label} bind:value disabled={busy} {error} />
  </form>
  {#snippet footer()}
    <Button onclick={onclose} disabled={busy}>Huỷ</Button>
    <Button variant="filled" type="submit" form="prompt-form" disabled={busy}>{busy ? "Đang lưu…" : confirmLabel}</Button>
  {/snippet}
</Modal>
