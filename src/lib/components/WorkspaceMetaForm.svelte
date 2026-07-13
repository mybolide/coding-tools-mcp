<script lang="ts">
  import { FolderOpen } from "@lucide/svelte";
  import { openWorkspaceDirectory } from "$lib/api/workspaces";
  import { showToast } from "$lib/stores/toast";

  interface Props {
    name: string;
    path: string;
    onSave: (name: string) => void | Promise<void>;
  }

  let { name, path, onSave }: Props = $props();

  let draftName = $state("");
  let saving = $state(false);
  let opening = $state(false);

  const dirty = $derived(draftName.trim() !== name && draftName.trim().length > 0);

  $effect(() => {
    draftName = name;
  });

  async function save() {
    if (saving || !dirty) return;
    saving = true;
    try {
      await onSave(draftName.trim());
    } finally {
      saving = false;
    }
  }

  async function openDirectory() {
    if (opening || !path.trim()) return;
    opening = true;
    try {
      await openWorkspaceDirectory(path);
    } catch (error) {
      showToast(String(error), {
        kind: "error",
        title: "无法打开目录",
      });
    } finally {
      opening = false;
    }
  }
</script>

<form
  class="flex flex-col gap-3 sm:flex-row sm:items-end"
  onsubmit={(event) => {
    event.preventDefault();
    void save();
  }}
>
  <label class="tx-field min-w-0 flex-1">
    <span class="tx-label">工作区名称</span>
    <input type="text" class="tx-input" bind:value={draftName} />
  </label>
  <div class="tx-field min-w-0 flex-1">
    <span class="tx-label">路径</span>
    <div class="flex min-w-0 items-center gap-2">
      <p
        class="tx-mono min-w-0 flex-1 truncate rounded-[10px] border border-transparent px-2.5 py-2 text-[var(--color-text-secondary)]"
        title={path}
      >
        {path}
      </p>
      <button
        type="button"
        class="tx-btn-ghost shrink-0 px-2.5 py-1.5 text-xs"
        disabled={opening || !path.trim()}
        onclick={() => void openDirectory()}
      >
        <FolderOpen size={14} class="inline-block" />
        <span class="ml-1">{opening ? "打开中…" : "打开目录"}</span>
      </button>
    </div>
  </div>
  <button type="submit" class="tx-btn-primary shrink-0" disabled={saving || !dirty}>
    {saving ? "保存中…" : "保存名称"}
  </button>
</form>
