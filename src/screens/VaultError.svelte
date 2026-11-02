<script lang="ts">
  import { api } from "../api";
  import Button from "../components/Button.svelte";
  import Icon from "../components/Icon.svelte";
  import { errorText } from "../util";

  interface Props {
    error: { message: string; hint: string | null; code: string };
    onretry: () => void;
    onreset: () => void;
  }

  let { error, onretry, onreset }: Props = $props();

  let confirming = $state(false);
  let busy = $state(false);
  let failure = $state<string | null>(null);

  async function reset() {
    busy = true;
    failure = null;
    try {
      await api.accounts.resetVault();
      onreset();
    } catch (e) {
      failure = errorText(e);
      busy = false;
    }
  }
</script>

<main class="welcome">
  <div class="welcome-card">
    <h1>Không mở được kho khoá</h1>
    <div class="vault-err" role="alert">
      <Icon name="alert" size={20} />
      <div>
        <p class="vault-msg">{error.message}</p>
        {#if error.hint}<p class="vault-hint">{error.hint}</p>{/if}
      </div>
    </div>

    {#if !confirming}
      <div class="welcome-actions">
        <Button variant="filled" onclick={onretry}>Thử lại</Button>
        <Button variant="outlined" onclick={() => (confirming = true)}>Đặt lại kho khoá…</Button>
      </div>
    {:else}
      <p class="note-danger">
        Mọi khoá đã nhập sẽ bị xoá khỏi máy này (file cũ được giữ lại với đuôi .bak). Bạn cần nhập lại các file JSON. Khoá trên Google Cloud không bị ảnh
        hưởng.
      </p>
      <div class="welcome-actions">
        <Button variant="outlined" onclick={() => (confirming = false)} disabled={busy}>Không đặt lại</Button>
        <Button variant="filled" error onclick={reset} disabled={busy}>{busy ? "Đang đặt lại…" : "Đặt lại kho khoá"}</Button>
      </div>
      {#if failure}<p class="form-err" role="alert">{failure}</p>{/if}
    {/if}
  </div>
</main>
