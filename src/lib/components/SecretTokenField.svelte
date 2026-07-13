<script lang="ts">
  import { secretIsSet, setSecret, type SecretKey } from "$lib/api/secrets";
  import SecretInput from "$lib/components/SecretInput.svelte";

  interface Props {
    workspaceId: string;
    secretKey: SecretKey;
    label?: string;
    onSaved?: () => void;
    hasPending?: boolean;
  }

  let {
    workspaceId,
    secretKey,
    label = "Cloudflare Tunnel Token",
    onSaved,
    hasPending = $bindable(false),
  }: Props = $props();

  let draft = $state("");
  let saved = $state(false);
  let loading = $state(true);

  const placeholder = $derived(saved && !draft ? "已保存（点击更新）" : "粘贴 Tunnel Token");

  $effect(() => {
    hasPending = draft.trim().length > 0;
  });

  $effect(() => {
    workspaceId;
    secretKey;
    void load();
  });

  async function load() {
    loading = true;
    try {
      draft = "";
      saved = await secretIsSet(workspaceId, secretKey);
    } finally {
      loading = false;
    }
  }

  export async function saveIfDirty(): Promise<boolean> {
    if (!draft.trim()) return false;
    await setSecret(workspaceId, secretKey, draft.trim());
    saved = true;
    draft = "";
    onSaved?.();
    return true;
  }

  export function hasPendingValue(): boolean {
    return hasPending;
  }
</script>

<label class="grid gap-1">
  <span class="text-xs text-[var(--color-text-muted)]">{label}</span>
  <SecretInput
    bind:value={draft}
    {placeholder}
    disabled={loading}
    showCopy={false}
  />
</label>
