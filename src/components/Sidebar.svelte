<script lang="ts">
  import type { IconName } from "../icons";
  import type { About, DrivesState, HelpSection, Loc } from "../types";
  import Icon from "./Icon.svelte";
  import StorageMeter from "./StorageMeter.svelte";

  interface Props {
    loc: Loc;
    drives: DrivesState;
    /** Vị trí hiện tại có nhận được file tải lên không (quyết định nút "Mới" có dùng được) */
    canNew: boolean;
    weakKeyStorage: boolean;
    about: About | null;
    aboutLoading: boolean;
    aboutError: { message: string } | null;
    onnew: (anchor: HTMLElement) => void;
    onplace: (loc: Loc) => void;
    onhelp: (section?: HelpSection) => void;
    onretrydrives: () => void;
    onretryabout: () => void;
  }

  let { loc, drives, canNew, weakKeyStorage, about, aboutLoading, aboutError, onnew, onplace, onhelp, onretrydrives, onretryabout }: Props = $props();

  const FIXED: { kind: "my" | "owned" | "shared"; icon: IconName; label: string; hint: string }[] = [
    { kind: "my", icon: "drive", label: "Drive của SA", hint: "Cây thư mục bắt đầu từ My Drive của service account" },
    { kind: "owned", icon: "inbox", label: "Tất cả file của SA", hint: "Mọi file do service account này sở hữu, ở bất kỳ thư mục nào" },
    { kind: "shared", icon: "users", label: "Được chia sẻ với SA", hint: "Thư mục và file người khác đã chia sẻ cho email của service account" },
  ];

  // Khi đang tìm kiếm, mục được tô sáng là nơi bắt đầu tìm.
  const base = $derived<Loc>(loc.kind === "search" ? loc.prev.loc : loc);

  let newBtn: HTMLButtonElement;
</script>

{#snippet place(target: Loc, icon: IconName, label: string, hint: string, on: boolean)}
  <button class="place" class:on aria-current={on ? "page" : undefined} title={hint} onclick={() => onplace(target)}>
    <Icon name={icon} size={19} />
    <span class="place-label">{label}</span>
  </button>
{/snippet}

<nav class="side" aria-label="Điều hướng">
  <!-- Không dùng disabled: nút vẫn bấm được để báo lý do vì sao chưa tải lên được ở đây. -->
  <button
    bind:this={newBtn}
    class="new-btn"
    class:off={!canNew}
    aria-haspopup="menu"
    aria-disabled={!canNew}
    title={canNew ? undefined : "Mở Drive của SA, một Shared Drive hoặc một thư mục để thêm file."}
    onclick={() => onnew(newBtn)}
  >
    <Icon name="plus" size={22} />
    Mới
  </button>

  <div class="places">
    {#each FIXED as f (f.kind)}
      {@render place({ kind: f.kind }, f.icon, f.label, f.hint, base.kind === f.kind)}
    {/each}

    <div class="place-group">Shared Drive</div>
    {#if drives.loading}
      <div class="place-note"><span class="skeleton" style:width="120px"></span></div>
    {:else if drives.error}
      <div class="place-note err">
        <span>Không tải được danh sách.</span>
        <button class="link" onclick={onretrydrives}>Thử lại</button>
      </div>
    {:else if drives.list.length === 0}
      <div class="place-note">
        <span>Chưa là thành viên Shared Drive nào.</span>
        <button class="link" onclick={() => onhelp("quota")}>Cách thêm</button>
      </div>
    {/if}
    {#each drives.list as d (d.id)}
      {@render place({ kind: "drive", id: d.id, name: d.name }, "folder", d.name, `Shared Drive “${d.name}”`, base.kind === "drive" && base.id === d.id)}
    {/each}

    <div class="place-gap"></div>
    {@render place({ kind: "trash" }, "trash", "Thùng rác", "Thùng rác của Drive service account (không gồm Shared Drive)", base.kind === "trash")}
  </div>

  <div class="side-foot">
    {#if weakKeyStorage}
      <div class="weak" role="note">
        <Icon name="alert" size={15} />
        <span>
          Keychain của hệ điều hành không dùng được, nên khoá mã hoá đang nằm trong một file cạnh kho khoá. Bảo vệ kém hơn.
          <button class="link" onclick={() => onhelp("security")}>Chi tiết</button>
        </span>
      </div>
    {/if}
    <StorageMeter {about} loading={aboutLoading} error={aboutError} onretry={onretryabout} onhelp={() => onhelp("quota")} />
  </div>
</nav>
