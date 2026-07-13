<script lang="ts">
  import { onMount } from "svelte";
  import { listFrpProfiles, type FrpProfileDto } from "$lib/api/settings";
  import { testTunnel as invokeTunnelTest } from "$lib/api/tunnel";
  import SecretTokenField from "$lib/components/SecretTokenField.svelte";
  import { showToast } from "$lib/stores/toast";

  export interface TunnelFormConfig {
    type: string;
    public_url: string;
    frp_server: string;
    frp_subdomain: string;
    frp_profile_id: string;
    frp_server_port: number;
    cloudflare_mode: string;
    use_proxy: boolean;
  }

  export interface SaveTunnelOptions {
    skipTunnelRestart?: boolean;
    skipServicePrompt?: boolean;
  }

  interface Props {
    workspaceId: string;
    service: "mcp" | "actions";
    config: TunnelFormConfig;
    onSave: (config: TunnelFormConfig, options?: SaveTunnelOptions) => void | Promise<void>;
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
    use_proxy: true,
  });
  let saving = $state(false);
  let testing = $state(false);
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
      draft.use_proxy !== config.use_proxy ||
      tokenPending,
  );

  const showFrp = $derived(draft.type === "frp");
  const showCloudflare = $derived(draft.type === "cloudflare");
  const showCloudflareToken = $derived(showCloudflare && draft.cloudflare_mode === "named");
  const showLegacyFrpToken = $derived(showFrp && !useGlobalProfile);
  const canTest = $derived(draft.type === "frp" || draft.type === "cloudflare");

  $effect(() => {
    draft = {
      ...config,
      frp_profile_id: config.frp_profile_id ?? "",
      use_proxy: config.use_proxy ?? true,
    };
  });

  onMount(async () => {
    frpProfiles = await listFrpProfiles();
  });

  async function saveDraft(options?: SaveTunnelOptions) {
    if (tokenField && (showLegacyFrpToken || showCloudflareToken)) {
      await tokenField.saveIfDirty();
    }
    await onSave({ ...draft }, options);
  }

  async function save() {
    if (saving || !dirty) return;
    saving = true;
    try {
      await saveDraft();
    } finally {
      saving = false;
    }
  }

  async function testTunnelConnection() {
    if (!canTest || testing) return;
    testing = true;
    try {
      if (dirty) {
        await saveDraft({ skipTunnelRestart: true, skipServicePrompt: true });
      }

      const result = await invokeTunnelTest(workspaceId, service);
      if (result.publicUrl && draft.cloudflare_mode === "quick") {
        draft.public_url = result.publicUrl;
      }

      if (result.success && result.publicUrl) {
        const detail = `${result.message}\n${result.publicUrl}${
          result.keptRunning ? "" : "\n\n如需长期使用，请先启动服务。"
        }`;
        showToast(detail, { title: "测试成功", kind: "success", duration: 8000 });
      } else if (result.success) {
        showToast(result.message, { title: "测试成功", kind: "success" });
      } else {
        showToast(result.message, { title: "测试未完成", kind: "warning", duration: 7000 });
      }
    } catch (error) {
      showToast(String(error), { title: "测试失败", kind: "error", duration: 8000 });
    } finally {
      testing = false;
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

  {#if canTest}
    <label class="flex items-start gap-2 rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-2.5">
      <input
        type="checkbox"
        class="mt-0.5 h-4 w-4"
        bind:checked={draft.use_proxy}
      />
      <span class="grid gap-0.5">
        <span class="text-xs font-medium text-[var(--color-text-secondary)]">使用网络代理</span>
        <span class="text-[11px] text-[var(--color-text-muted)]">
          启用后通过「设置 → 通用」中的全局代理连接隧道；关闭则直连（适合海外或已全局翻墙的环境）。
        </span>
      </span>
    </label>
  {/if}

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

  <div class="flex justify-end gap-2 pt-1">
    {#if canTest}
      <button
        type="button"
        class="tx-btn-ghost px-3 py-1.5 text-sm disabled:opacity-50"
        disabled={testing || saving}
        onclick={() => void testTunnelConnection()}
      >
        {testing ? "测试中…" : "测试连接"}
      </button>
    {/if}
    <button
      type="submit"
      class="rounded-md bg-[var(--color-accent)] px-3 py-1.5 text-sm font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
      disabled={saving || testing || !dirty}
    >
      {saving ? "保存中…" : "保存配置"}
    </button>
  </div>
</form>
