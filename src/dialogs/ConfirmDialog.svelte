<script lang="ts">
  import AccountRow from "../components/AccountRow.svelte";
  import Button from "../components/Button.svelte";
  import Modal from "../components/Modal.svelte";
  import type { AccountInfo } from "../types";
  import { errorText } from "../util";

  interface Props {
    title: string;
    message: string;
    confirmLabel: string;
    account: AccountInfo;
    danger?: boolean;
    onconfirm: () => Promise<void>;
    onclose: () => void;
  }

  let { title, message, confirmLabel, account, danger = false, onconfirm, onclose }: Props = $props();

  let busy = $state(false);
  let error = $state<string | null>(null);

  async function confirm() {
    busy = true;
    error = null;
    try {
      await onconfirm();
      onclose();
    } catch (e) {
      error = errorText(e);
      busy = false;
    }
  }
</script>

<Modal {title} {onclose} {busy} width={480}>
  <p>{message}</p>
  <AccountRow {account} />
  {#if error}<p class="md-note error" role="alert">{error}</p>{/if}

  {#snippet footer()}
    <Button data-autofocus onclick={onclose} disabled={busy}>Huỷ</Button>
    <Button variant="filled" error={danger} onclick={confirm} disabled={busy}>{busy ? "Đang xử lý…" : confirmLabel}</Button>
  {/snippet}
</Modal>
