<script lang="ts">
  import CopyButton from "$lib/components/CopyButton.svelte";
  import { restartRuntime } from "$lib/api/workspaces";
  import {
    getWorkspaceSecret,
    regenerateWorkspaceSecret,
    getSharedSecret,
    regenerateSharedSecret,
    type WorkspaceSecretKey,
    type SharedSecretKey,
  } from "$lib/api/secrets";
  import type { AuthConfig } from "$lib/types";

  interface Props {
    workspaceId: string;
    auth: AuthConfig;
    onSaveProfile: (auth: AuthConfig) => void | Promise<void>;
  }

  const AUTH_OPTIONS = [
    { value: "oauth", label: "OAuth" },
    { value: "bearer", label: "Bearer Token" },
    { value: "noauth", label: "不启用认证" },
  ] as const;

  let { workspaceId, auth, onSaveProfile }: Props = $props();

  let draft = $state<AuthConfig>({ type: "oauth", oauth_client_id: "", use_shared_secrets: false });
  let saving = $state(false);
  let secrets = $state<Partial<Record<WorkspaceSecretKey, string>>>({});
  let loadedSecrets = $state<Partial<Record<WorkspaceSecretKey, string>>>({});
  let regenerating = $state<WorkspaceSecretKey | null>(null);

  const secretsDirty = $derived(
    (Object.keys(secrets) as WorkspaceSecretKey[]).some(
      (k) => secrets[k] !== loadedSecrets[k],
    ),
  );

  const dirty = $derived(
    draft.type !== auth.type ||
      draft.oauth_client_id !== auth.oauth_client_id ||
      draft.use_shared_secrets !== !!auth.use_shared_secrets ||
      secretsDirty,
  );

  const showOAuth = $derived(draft.type === "oauth");
  const showBearer = $derived(draft.type === "bearer");

  $effect(() => {
    draft = { type: auth.type, oauth_client_id: auth.oauth_client_id, use_shared_secrets: !!auth.use_shared_secrets };
  });

  $effect(() => {
    void loadSecrets(workspaceId, draft.type, draft.use_shared_secrets ?? false);
  });

  async function loadSecrets(id: string, authType: string, useShared: boolean) {
    const keys: WorkspaceSecretKey[] = [];
    if (authType === "oauth") {
      keys.push("oauth_client_secret", "oauth_password");
    } else if (authType === "bearer") {
      keys.push("bearer_token");
    }
    if (keys.length === 0) {
      secrets = {};
      return;
    }
    const loaded = await Promise.all(
      keys.map(async (key) => {
        const value = useShared
          ? await getSharedSecret(key as SharedSecretKey)
          : await getWorkspaceSecret(id, key);
        return [key, value ?? ""] as const;
      }),
    );
    secrets = Object.fromEntries(loaded);
    loadedSecrets = Object.fromEntries(loaded);
  }

  async function save() {
    if (saving || !dirty) return;
    saving = true;
    try {
      await onSaveProfile({ ...draft });
      loadedSecrets = { ...secrets };
    } finally {
      saving = false;
    }
  }

  async function regenerate(key: WorkspaceSecretKey) {
    if (regenerating) return;
    regenerating = key;
    try {
      const value = draft.use_shared_secrets
        ? await regenerateSharedSecret(key as SharedSecretKey)
        : await regenerateWorkspaceSecret(workspaceId, key);
      restartRuntime(workspaceId).catch(() => {});
      secrets = { ...secrets, [key]: value };
    } finally {
      regenerating = null;
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
    <span class="text-xs text-[var(--color-text-muted)]">认证类型</span>
    <select
      class="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 text-sm"
      bind:value={draft.type}
    >
      {#each AUTH_OPTIONS as option}
        <option value={option.value}>{option.label}</option>
      {/each}
    </select>
  </label>

  <label class="flex items-center gap-2">
    <input
      type="checkbox"
      class="h-4 w-4"
      bind:checked={draft.use_shared_secrets}
    />
    <span class="text-xs text-[var(--color-text-muted)]">使用全局共享密钥（在「设置 → 共享密钥」中管理）</span>
  </label>

  {#if showOAuth}
    <label class="grid gap-1">
      <span class="text-xs text-[var(--color-text-muted)]">OAuth 客户端 ID</span>
      <input
        type="text"
        class="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 font-mono text-sm"
        bind:value={draft.oauth_client_id}
      />
    </label>

    <div class="grid gap-1">
      <span class="text-xs text-[var(--color-text-muted)]">OAuth 客户端密钥</span>
      <div class="flex gap-2">
        <input
          type="password"
          readonly
          class="min-w-0 flex-1 rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 font-mono text-sm"
          value={secrets.oauth_client_secret ?? ""}
          placeholder="加载中…"
        />
        <CopyButton value={secrets.oauth_client_secret ?? ""} />
        <button
          type="button"
          class="shrink-0 rounded-md border border-[var(--color-border)] px-2.5 py-1 text-xs text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-surface-hover)] disabled:opacity-50"
          disabled={regenerating === "oauth_client_secret"}
          onclick={() => {
            void regenerate("oauth_client_secret");
          }}
        >
          {regenerating === "oauth_client_secret" ? "生成中…" : "重新生成"}
        </button>
      </div>
    </div>

    <div class="grid gap-1">
      <span class="text-xs text-[var(--color-text-muted)]">授权口令</span>
      <div class="flex gap-2">
        <input
          type="password"
          readonly
          class="min-w-0 flex-1 rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 font-mono text-sm"
          value={secrets.oauth_password ?? ""}
          placeholder="ChatGPT 首次授权时输入这个口令"
        />
        <CopyButton value={secrets.oauth_password ?? ""} />
        <button
          type="button"
          class="shrink-0 rounded-md border border-[var(--color-border)] px-2.5 py-1 text-xs text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-surface-hover)] disabled:opacity-50"
          disabled={regenerating === "oauth_password"}
          onclick={() => {
            void regenerate("oauth_password");
          }}
        >
          {regenerating === "oauth_password" ? "生成中…" : "重新生成"}
        </button>
      </div>
    </div>
  {/if}

  {#if showBearer}
    <div class="grid gap-1">
      <span class="text-xs text-[var(--color-text-muted)]">Bearer Token</span>
      <div class="flex gap-2">
        <input
          type="password"
          readonly
          class="min-w-0 flex-1 rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 font-mono text-sm"
          value={secrets.bearer_token ?? ""}
          placeholder="加载中…"
        />
        <CopyButton value={secrets.bearer_token ?? ""} />
        <button
          type="button"
          class="shrink-0 rounded-md border border-[var(--color-border)] px-2.5 py-1 text-xs text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-surface-hover)] disabled:opacity-50"
          disabled={regenerating === "bearer_token"}
          onclick={() => {
            void regenerate("bearer_token");
          }}
        >
          {regenerating === "bearer_token" ? "生成中…" : "重新生成"}
        </button>
      </div>
    </div>
  {/if}

  <div class="flex justify-end pt-1">
    <button
      type="submit"
      class="rounded-md bg-[var(--color-accent)] px-3 py-1.5 text-sm font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
      disabled={saving || !dirty}
    >
      {saving ? "保存中…" : "保存配置"}
    </button>
  </div>
</form>
