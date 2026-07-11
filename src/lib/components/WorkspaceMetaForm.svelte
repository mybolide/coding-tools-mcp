<script lang="ts">
  interface Props {
    name: string;
    path: string;
    onSave: (name: string) => void | Promise<void>;
  }

  let { name, path, onSave }: Props = $props();

  let draftName = $state("");
  let saving = $state(false);

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
    <p class="tx-mono truncate rounded-[10px] border border-transparent px-2.5 py-2 text-[var(--color-text-secondary)]">
      {path}
    </p>
  </div>
  <button type="submit" class="tx-btn-primary shrink-0" disabled={saving || !dirty}>
    {saving ? "保存中…" : "保存名称"}
  </button>
</form>
