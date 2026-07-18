<script lang="ts">
  import type { ActionsConfig } from "$lib/types";

  export interface ActionsPolicyDraft {
    allowedCommands: string;
    maxPatchBytes: number;
    permissionMode: string;
    autoStart: boolean;
    autoRecover: boolean;
  }

  interface Props {
    allowedCommands: string;
    maxPatchBytes: number;
    permissionMode: string;
    autoStart: boolean;
    autoRecover: boolean;
    onSave: (draft: ActionsPolicyDraft) => void | Promise<void>;
  }

  const PERMISSION_MODE_OPTIONS = [
    { value: "trusted", label: "受信任" },
    { value: "safe", label: "安全受限" },
    { value: "dangerous", label: "完全放开" },
  ] as const;

  let { allowedCommands, maxPatchBytes, permissionMode, autoStart, autoRecover, onSave }: Props = $props();

  let draftCommands = $state("");
  let draftMaxPatch = $state(200_000);
  let draftMode = $state("dangerous");
  let draftAutoStart = $state(true);
  let draftAutoRecover = $state(true);
  let saving = $state(false);

  const dirty = $derived(
      draftCommands !== allowedCommands ||
      draftMaxPatch !== maxPatchBytes ||
      draftMode !== permissionMode ||
      draftAutoStart !== autoStart ||
      draftAutoRecover !== autoRecover,
  );

  $effect(() => {
    draftCommands = allowedCommands;
    draftMaxPatch = maxPatchBytes;
    draftMode = permissionMode;
    draftAutoStart = autoStart;
    draftAutoRecover = autoRecover;
  });

  async function save() {
    if (saving || !dirty) return;
    saving = true;
    try {
      await onSave({
        allowedCommands: draftCommands.trim(),
        maxPatchBytes: draftMaxPatch,
        permissionMode: draftMode,
        autoStart: draftAutoStart,
        autoRecover: draftAutoRecover,
      });
    } finally {
      saving = false;
    }
  }
</script>

<form
  class="grid gap-3"
  onsubmit={(event) => {
    event.preventDefault();
    void save();
  }}
>
  <label class="grid gap-1">
    <span class="text-xs text-[var(--color-text-muted)]">允许命令（逗号分隔）</span>
    <input
      type="text"
      class="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 font-mono text-sm"
      placeholder="pytest,python,cargo,npm,..."
      bind:value={draftCommands}
    />
  </label>
  <label class="flex items-center gap-2 text-sm">
    <input type="checkbox" bind:checked={draftAutoStart} />
    <span>应用启动时自动启动 Actions</span>
  </label>
  <label class="flex items-center gap-2 text-sm">
    <input type="checkbox" bind:checked={draftAutoRecover} />
    <span>端口异常时自动恢复 Actions</span>
  </label>
  <label class="grid gap-1">
    <span class="text-xs text-[var(--color-text-muted)]">最大 Patch 字节数</span>
    <input
      type="number"
      min="1024"
      max="50000000"
      class="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 text-sm"
      bind:value={draftMaxPatch}
    />
  </label>
  <label class="grid gap-1">
    <span class="text-xs text-[var(--color-text-muted)]">权限模式</span>
    <select
      class="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 text-sm"
      bind:value={draftMode}
    >
      {#each PERMISSION_MODE_OPTIONS as option}
        <option value={option.value}>{option.label}</option>
      {/each}
    </select>
  </label>
  <p class="text-xs text-[var(--color-text-muted)]">
    作用于 Actions gateway 的 exec_command 白名单与 apply_patch 大小限制；“完全放开”模式下运行时会跳过这些门槛。
  </p>
  <div class="flex justify-end pt-1">
    <button
      type="submit"
      class="rounded-md bg-[var(--color-accent)] px-3 py-1.5 text-sm font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
      disabled={saving || !dirty}
    >
      {saving ? "保存中…" : "保存策略"}
    </button>
  </div>
</form>
