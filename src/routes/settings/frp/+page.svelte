<script lang="ts">
  import { onMount } from "svelte";
  import { message } from "@tauri-apps/plugin-dialog";
  import {
    deleteFrpProfile,
    listFrpProfiles,
    saveFrpProfile,
    type FrpProfileDto,
  } from "$lib/api/settings";

  let profiles = $state<FrpProfileDto[]>([]);
  let loading = $state(true);
  let saving = $state(false);
  let editingId = $state<string | null>(null);
  let name = $state("");
  let server = $state("");
  let serverPort = $state(7000);
  let token = $state("");

  async function refresh() {
    loading = true;
    try {
      profiles = await listFrpProfiles();
    } finally {
      loading = false;
    }
  }

  function resetForm() {
    editingId = null;
    name = "";
    server = "";
    serverPort = 7000;
    token = "";
  }

  function editProfile(profile: FrpProfileDto) {
    editingId = profile.id;
    name = profile.name;
    server = profile.server;
    serverPort = profile.serverPort;
    token = "";
  }

  async function save() {
    if (!name.trim() || !server.trim()) {
      await message("请填写配置名称和服务器地址。", { title: "无法保存", kind: "warning" });
      return;
    }
    saving = true;
    try {
      await saveFrpProfile(
        {
          id: editingId ?? "",
          name: name.trim(),
          server: server.trim(),
          serverPort,
        },
        token.trim() || undefined,
      );
      resetForm();
      await refresh();
    } catch (error) {
      await message(String(error), { title: "保存失败", kind: "error" });
    } finally {
      saving = false;
    }
  }

  async function removeProfile(profile: FrpProfileDto) {
    try {
      await deleteFrpProfile(profile.id);
      if (editingId === profile.id) {
        resetForm();
      }
      await refresh();
    } catch (error) {
      await message(String(error), { title: "删除失败", kind: "error" });
    }
  }

  onMount(refresh);
</script>

<section class="page-scroll">
  <header class="page-header">
    <p class="page-kicker">全局设置</p>
    <h2 class="page-title">FRP 配置</h2>
    <p class="mt-2 max-w-2xl text-sm text-[var(--color-text-muted)]">
      在此配置 FRP 服务器、端口与 Token。各工作区只需选择配置并填写自己的子域名；修改子域名后保存会自动更新
      frpc 配置并重启隧道。
    </p>
  </header>

  <div class="page-body grid flex-1 gap-8 lg:grid-cols-2">
    <div class="tx-card p-4">
      <h3 class="text-sm font-semibold">{editingId ? "编辑配置" : "新建配置"}</h3>
      <form
        class="mt-4 grid gap-3"
        onsubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <label class="grid gap-1">
          <span class="text-xs text-[var(--color-text-muted)]">名称</span>
          <input
            type="text"
            class="tx-input"
            placeholder="公司 FRP"
            bind:value={name}
          />
        </label>
        <label class="grid gap-1">
          <span class="text-xs text-[var(--color-text-muted)]">服务器域名</span>
          <input
            type="text"
            class="tx-input tx-mono"
            placeholder="frp.example.com"
            bind:value={server}
          />
        </label>
        <label class="grid gap-1">
          <span class="text-xs text-[var(--color-text-muted)]">端口</span>
          <input
            type="number"
            min="1"
            max="65535"
            class="tx-input"
            bind:value={serverPort}
          />
        </label>
        <label class="grid gap-1">
          <span class="text-xs text-[var(--color-text-muted)]">
            Token {editingId ? "（留空则保持不变）" : ""}
          </span>
          <input
            type="password"
            class="tx-input tx-mono"
            placeholder="frp auth token"
            bind:value={token}
          />
        </label>
        <div class="flex gap-2 pt-1">
          <button
            type="submit"
            class="rounded-md bg-[var(--color-accent)] px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
            disabled={saving}
          >
            {saving ? "保存中…" : editingId ? "更新" : "添加"}
          </button>
          {#if editingId}
            <button
              type="button"
              class="tx-btn-ghost"
              onclick={resetForm}
            >
              取消
            </button>
          {/if}
        </div>
      </form>
    </div>

    <div class="tx-card p-4">
      <h3 class="text-sm font-semibold">已保存的配置</h3>
      {#if loading}
        <p class="mt-4 text-sm text-[var(--color-text-muted)]">加载中…</p>
      {:else if profiles.length === 0}
        <p class="mt-4 text-sm text-[var(--color-text-muted)]">暂无 FRP 配置。</p>
      {:else}
        <ul class="mt-4 space-y-2">
          {#each profiles as profile (profile.id)}
            <li
              class="tx-panel flex items-center justify-between gap-3 px-3 py-2"
            >
              <div class="min-w-0">
                <p class="truncate text-sm font-medium">{profile.name}</p>
                <p class="truncate font-mono text-xs text-[var(--color-text-muted)]">
                  {profile.server}:{profile.serverPort}
                  · Token {profile.hasToken ? "已配置" : "未配置"}
                </p>
              </div>
              <div class="flex shrink-0 gap-2">
                <button
                  type="button"
                  class="text-xs text-[var(--color-accent)] hover:underline"
                  onclick={() => editProfile(profile)}
                >
                  编辑
                </button>
                <button
                  type="button"
                  class="text-xs text-red-400 hover:underline"
                  onclick={() => removeProfile(profile)}
                >
                  删除
                </button>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
</section>
