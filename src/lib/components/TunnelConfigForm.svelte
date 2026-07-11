<script lang="ts">
  import { onMount } from "svelte";
  import { listFrpProfiles, type FrpProfileDto } from "$lib/api/settings";
  import SecretTokenField from "$lib/components/SecretTokenField.svelte";

  export interface TunnelFormConfig {
    type: string;
    public_url: string;
    frp_server: string;
    frp_subdomain: string;
    frp_profile_id: string;
    frp_server_port: number;
    cloudflare_mode: string;
  }

  interface Props {
    workspaceId: string;
    service: "mcp" | "actions";
    config: TunnelFormConfig;
    onSave: (config: TunnelFormConfig) => void | Promise<void>;
  }

  let { workspaceId, service, config, onSave }: Props = $props();

  let draft = $state<TunnelFormConfig>({
    type: "none",
    public_url: "",
    frp_server: "",
    frp_subdomain: "",
    frp_profile_id: "",
    frp_server_port: 7000,
    cloudflare_mode: "quick",
  });
  let saving = $state(false);
  let tokenField = $state<SecretTokenField | null>(null);
  let tokenPending = $state(false);
  let frpProfiles = $state<FrpProfileDto[]>([]);
  let legacyFrpOpen = $state(false);

  const secretKey = $derived(
    service === "mcp"
      ? draft.type === "frp"
        ? ("frp_token" as const)
        : ("cloudflare_token" as const)
      : draft.type === "frp"
        ? ("actions_frp_token" as const)
        : ("actions_cloudflare_token" as const),
  );

  const selectedProfile = $derived(
    frpProfiles.find((profile) => profile.id === draft.frp_profile_id) ?? null,
  );

  const useGlobalProfile = $derived(Boolean(draft.frp_profile_id && selectedProfile));

  const dirty = $derived(
    draft.type !== config.type ||
      draft.public_url !== config.public_url ||
      draft.frp_server !== config.frp_server ||
      draft.frp_subdomain !== config.frp_subdomain ||
      draft.frp_profile_id !== config.frp_profile_id ||
      draft.frp_server_port !== config.frp_server_port ||
      draft.cloudflare_mode !== config.cloudflare_mode ||
      tokenPending,
  );

  const showFrp = $derived(draft.type === "frp");
  const showCloudflare = $derived(draft.type === "cloudflare");
  const showCloudflareToken = $derived(showCloudflare && draft.cloudflare_mode === "named");
  const showLegacyFrpToken = $derived(showFrp && !useGlobalProfile);

  $effect(() => {
    draft = { ...config, frp_profile_id: config.frp_profile_id ?? "" };
  });

  onMount(async () => {
    frpProfiles = await listFrpProfiles();
  });

  async function save() {
    if (saving || !dirty) return;
    saving = true;
    try {
      if (showLegacyFrpToken) {
        await tokenField?.saveIfDirty();
      }
      await onSave({ ...draft });
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
    <span class="text-xs text-[var(--color-text-muted)]">隧道类型</span>
    <select
      class="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 text-sm"
      bind:value={draft.type}
    >
      <option value="none">未配置</option>
      <option value="frp">FRP</option>
      <option value="cloudflare">Cloudflare</option>
    </select>
  </label>

  {#if showFrp}
    <label class="grid gap-1">
      <span class="text-xs text-[var(--color-text-muted)]">FRP 配置</span>
      <select
        class="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 text-sm"
        bind:value={draft.frp_profile_id}
      >
        <option value="">手动填写（旧版）</option>
        {#each frpProfiles as profile (profile.id)}
          <option value={profile.id}>
            {profile.name} · {profile.server}:{profile.serverPort}
          </option>
        {/each}
      </select>
      {#if frpProfiles.length === 0}
        <p class="text-[11px] text-[var(--color-text-muted)]">
          请先在侧边栏「FRP 配置」中添加全局服务器配置。
        </p>
      {/if}
    </label>

    {#if useGlobalProfile && selectedProfile}
      <div class="rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-2 text-xs">
        <p class="text-[var(--color-text-secondary)]">
          服务器：{selectedProfile.server}:{selectedProfile.serverPort}
        </p>
        <p class="mt-1 text-[var(--color-text-muted)]">
          Token：{selectedProfile.hasToken ? "已配置" : "未配置"}
        </p>
      </div>
    {/if}

    <label class="grid gap-1">
      <span class="text-xs text-[var(--color-text-muted)]">子域名</span>
      <input
        type="text"
        class="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 font-mono text-sm"
        placeholder="my-mcp"
        bind:value={draft.frp_subdomain}
      />
      <p class="text-[11px] text-[var(--color-text-muted)]">
        每个工作区使用独立子域名；保存后若隧道已连接会自动重启 frpc。
      </p>
    </label>

    {#if !useGlobalProfile}
      <button
        type="button"
        class="text-left text-xs text-[var(--color-accent)] hover:underline"
        onclick={() => {
          legacyFrpOpen = !legacyFrpOpen;
        }}
      >
        {legacyFrpOpen ? "收起" : "展开"}手动 FRP 配置
      </button>
    {/if}

    {#if !useGlobalProfile && legacyFrpOpen}
      <label class="grid gap-1">
        <span class="text-xs text-[var(--color-text-muted)]">FRP 服务器</span>
        <input
          type="text"
          class="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 font-mono text-sm"
          placeholder="example.com"
          bind:value={draft.frp_server}
        />
      </label>

      <label class="grid gap-1">
        <span class="text-xs text-[var(--color-text-muted)]">FRP 服务器端口</span>
        <input
          type="number"
          min="1"
          max="65535"
          class="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 text-sm"
          bind:value={draft.frp_server_port}
        />
      </label>

      {#if showLegacyFrpToken}
        <SecretTokenField
          bind:this={tokenField}
          bind:hasPending={tokenPending}
          {workspaceId}
          secretKey={secretKey}
          label="FRP Token（可选）"
        />
      {/if}
    {/if}
  {/if}

  {#if showCloudflare}
    <label class="grid gap-1">
      <span class="text-xs text-[var(--color-text-muted)]">Cloudflare 模式</span>
      <select
        class="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 text-sm"
        bind:value={draft.cloudflare_mode}
      >
        <option value="quick">Quick Tunnel</option>
        <option value="named">Named Tunnel</option>
      </select>
    </label>

    {#if showCloudflareToken}
      <SecretTokenField
        bind:this={tokenField}
        bind:hasPending={tokenPending}
        {workspaceId}
        secretKey={secretKey}
      />
    {/if}
  {/if}

  <label class="grid gap-1">
    <span class="text-xs text-[var(--color-text-muted)]">
      公网 URL
      {#if service === "actions"}
        <span class="text-[var(--color-text-muted)]">（OpenAPI 根地址）</span>
      {/if}
    </span>
    <input
      type="url"
      class="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 font-mono text-sm"
      placeholder="https://..."
      bind:value={draft.public_url}
    />
  </label>

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
