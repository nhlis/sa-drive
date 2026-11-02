<script lang="ts">
  import type { AccountInfo } from "../types";
  import Avatar from "./Avatar.svelte";

  /** Account đang thực hiện thao tác */
  let { account }: { account: AccountInfo } = $props();
</script>

<div class="md-account">
  <Avatar {account} size={40} />
  <div class="md-account-text">
    <span class="md-account-lead">Thực hiện bằng</span>
    <span class="md-account-name">{account.label}</span>
    <span class="md-account-email" title={account.clientEmail}>{account.clientEmail}</span>
  </div>
</div>
