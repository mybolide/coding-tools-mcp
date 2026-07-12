<script lang="ts">
  import { onMount } from "svelte";
  import { message } from "@tauri-apps/plugin-dialog";
  import type { DownloadConfig, SoftwareStatus } from "$lib/api/software";
  import {
    listSoftware,
    installSoftware,
    uninstallSoftware,
    getDownloadConfig,
    setDownloadConfig,
  } from "$lib/api/software";

  let software = $state<SoftwareStatus[]>([]);
  let loading = $state(true);
  let installing = $state<string | null>(null);
  let uninstalling = $state<string | null>(null);

  let downloadConfig = $state<DownloadConfig>({
    githubMirror: "https://gh-proxy.com",
    proxyMode: "system",
    proxyUrl: "",
  });
  let configChanged = $state(false);

  async function refresh() {
    loading = true;
    try {
      software = await listSoftware();
      downloadConfig = await getDownloadConfig();
      configChanged = false;
    } finally {
      loading = false;
    }
  }

  async function install(kind: string) {
    installing = kind;
    try {
      await installSoftware(kind);
      await refresh();
    } catch (e) {
      await message(String(e), { title: "安装失败", kind: "error" });
    } finally {
      installing = null;
    }
  }

  async function uninstall(kind: string) {
    uninstalling = kind;
    try {
      await uninstallSoftware(kind);
      await refresh();
    } catch (e) {
      await message(String(e), { title: "卸载失败", kind: "error" });
    } finally {
      uninstalling = null;
    }
  }

  async function saveConfig() {
    try {
      await setDownloadConfig(downloadConfig);
      configChanged = false;
      await message("下载配置已保存。", { title: "已保存", kind: "info" });
    } catch (e) {
      await message(String(e), { title: "保存失败", kind: "error" });
    }
  }

  onMount(refresh);
</script>

<section class="page-scroll">
  <header class="page-header">
    <p class="page-kicker">全局设置</p>
    <h2 class="page-title">软件管理</h2>
    <p class="mt-2 max-w-2xl text-sm text-[var(--color-text-muted)]">
      在此安装或卸载 frpc 和 cloudflared 隧道客户端。安装的软件会放入应用缓存目录，可统一管理。
    </p>
  </header>

  <div class="page-body flex flex-col gap-6">
    <!-- Binary status -->
    <div class="tx-card p-4">
      <h3 class="text-sm font-semibold">状态</h3>
      {#if loading}
        <p class="mt-4 text-sm text-[var(--color-text-muted)]">加载中…</p>
      {:else if software.length === 0}
        <p class="mt-4 text-sm text-[var(--color-text-muted)]">暂无信息。</p>
      {:else}
        <ul class="mt-4 space-y-2">
          {#each software as s (s.kind)}
            <li class="tx-panel flex items-center justify-between gap-3 px-3 py-2">
              <div class="min-w-0">
                <p class="text-sm font-medium">{s.name}</p>
                <p class="font-mono text-xs text-[var(--color-text-muted)]">
                  {s.installed ? s.path : "未安装"}
                  · {s.managed ? "可管理" : "系统安装"}
                </p>
              </div>
              <div class="flex shrink-0 gap-2">
                {#if s.installed}
                  {#if s.managed}
                    <button
                      type="button"
                      class="text-xs text-red-400 hover:underline disabled:opacity-50"
                      disabled={uninstalling === s.kind}
                      onclick={() => uninstall(s.kind)}
                    >
                      {uninstalling === s.kind ? "卸载中…" : "卸载"}
                    </button>
                  {:else}
                    <span class="text-xs text-[var(--color-text-muted)]">系统安装</span>
                  {/if}
                {:else}
                  <button
                    type="button"
                    class="text-xs text-[var(--color-accent)] hover:underline disabled:opacity-50"
                    disabled={installing === s.kind}
                    onclick={() => install(s.kind)}
                  >
                    {installing === s.kind ? "安装中…" : "安装"}
                  </button>
                {/if}
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <!-- Download config -->
    <div class="tx-card p-4">
      <h3 class="text-sm font-semibold">下载设置</h3>
      <form
        class="mt-4 grid gap-3"
        onsubmit={(e) => { e.preventDefault(); void saveConfig(); }}
      >
        <label class="grid gap-1">
          <span class="text-xs text-[var(--color-text-muted)]">GitHub 镜像</span>
          <input
            type="text"
            class="tx-input tx-mono"
            placeholder="https://gh-proxy.com"
            bind:value={downloadConfig.githubMirror}
            oninput={() => (configChanged = true)}
          />
          <span class="text-xs text-[var(--color-text-muted)]">留空则直连 GitHub，默认使用 gh-proxy.com 加速</span>
        </label>
        <label class="grid gap-1">
          <span class="text-xs text-[var(--color-text-muted)]">代理模式</span>
          <select
            class="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 text-sm"
            bind:value={downloadConfig.proxyMode}
            onchange={() => (configChanged = true)}
          >
            <option value="system">系统代理（默认）</option>
            <option value="none">无代理</option>
            <option value="manual">手动代理地址</option>
          </select>
        </label>
        {#if downloadConfig.proxyMode === "manual"}
          <label class="grid gap-1">
            <span class="text-xs text-[var(--color-text-muted)]">代理地址</span>
            <input
              type="text"
              class="tx-input tx-mono"
              placeholder="http://127.0.0.1:7890"
              bind:value={downloadConfig.proxyUrl}
              oninput={() => (configChanged = true)}
            />
          </label>
        {/if}
        <div class="flex justify-end pt-1">
          <button
            type="submit"
            class="rounded-md bg-[var(--color-accent)] px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
            disabled={!configChanged}
          >
            保存设置
          </button>
        </div>
      </form>
    </div>
  </div>
</section>
