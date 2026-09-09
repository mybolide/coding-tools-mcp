<script lang="ts">
  import { discoverUpstreamTools } from "$lib/api/workspaces";
  import type { DiscoveredUpstreamTool, UpstreamMcpConfig } from "$lib/types";

  type Transport = UpstreamMcpConfig["type"];
  type UnknownRecord = Record<string, unknown>;

  interface Props {
    configs?: UpstreamMcpConfig[];
    onSave: (configs: UpstreamMcpConfig[]) => void | Promise<void>;
  }

  let { configs = [], onSave }: Props = $props();
  let draft = $state<UpstreamMcpConfig[]>([]);
  let saving = $state(false);
  let validationError = $state("");
  let discoveredByConfig = $state<Record<string, DiscoveredUpstreamTool[]>>({});
  let discoveringByConfig = $state<Record<string, boolean>>({});
  let discoveryErrorByConfig = $state<Record<string, string>>({});
  let addMenuOpen = $state(false);
  let importOpen = $state(false);
  let importText = $state("");
  let importError = $state("");

  function cloneConfigs(source: UpstreamMcpConfig[]): UpstreamMcpConfig[] {
    return source.map((config) => ({
      ...config,
      type: config.type ?? "stdio",
      args: [...(config.args ?? [])],
      env: { ...(config.env ?? {}) },
      url: config.url ?? "",
      headers: { ...(config.headers ?? {}) },
      allowed_tools: [...(config.allowed_tools ?? [])],
      disabled_tools: [...(config.disabled_tools ?? [])],
    }));
  }

  $effect(() => {
    draft = cloneConfigs(configs);
  });

  function newId() {
    return `mcp${crypto.randomUUID().replaceAll("-", "")}`;
  }

  function blankConfig(type: Transport = "stdio"): UpstreamMcpConfig {
    return {
      id: newId(),
      name: type === "stdio" ? "本地 MCP" : "HTTP MCP",
      enabled: false,
      type,
      command: "",
      args: [],
      env: {},
      url: "",
      headers: {},
      tool_prefix: type === "stdio" ? "local" : "http",
      allowed_tools: [],
      disabled_tools: [],
    };
  }

  function add(config: UpstreamMcpConfig) {
    draft = [...draft, config];
  }

  function quickCreate() {
    addMenuOpen = false;
    add(blankConfig());
  }

  function openImport() {
    addMenuOpen = false;
    importError = "";
    importOpen = true;
  }

  function remove(index: number) {
    draft = draft.filter((_, current) => current !== index);
  }

  function update(index: number, patch: Partial<UpstreamMcpConfig>) {
    draft = draft.map((config, current) => (current === index ? { ...config, ...patch } : config));
  }

  function lines(value: string): string[] {
    return value.split(/\r?\n/).map((item) => item.trim()).filter(Boolean);
  }

  function recordText(record: Record<string, string>): string {
    return Object.entries(record).map(([key, value]) => `${key}=${value}`).join("\n");
  }

  function parseRecord(value: string): Record<string, string> {
    const parsed: Record<string, string> = {};
    for (const line of value.split(/\r?\n/)) {
      const separator = line.indexOf("=");
      if (separator < 1) continue;
      parsed[line.slice(0, separator).trim()] = line.slice(separator + 1);
    }
    return parsed;
  }

  function discoveredTools(config: UpstreamMcpConfig): DiscoveredUpstreamTool[] {
    return discoveredByConfig[config.id] ?? [];
  }

  function isToolEnabled(config: UpstreamMcpConfig, name: string): boolean {
    if ((config.allowed_tools ?? []).length > 0) return config.allowed_tools.includes(name);
    return !(config.disabled_tools ?? []).includes(name);
  }

  function setToolEnabled(index: number, name: string, enabled: boolean) {
    const config = draft[index];
    if (!config) return;
    const legacyDisabled = (config.allowed_tools ?? []).length > 0
      ? discoveredTools(config).filter((tool) => !config.allowed_tools.includes(tool.name)).map((tool) => tool.name)
      : [...(config.disabled_tools ?? [])];
    const disabled = new Set(legacyDisabled);
    if (enabled) disabled.delete(name);
    else disabled.add(name);
    update(index, { allowed_tools: [], disabled_tools: [...disabled].sort() });
  }

  async function discover(index: number) {
    const config = draft[index];
    if (!config || discoveringByConfig[config.id]) return;
    discoveryErrorByConfig = { ...discoveryErrorByConfig, [config.id]: "" };
    discoveringByConfig = { ...discoveringByConfig, [config.id]: true };
    try {
      const tools = await discoverUpstreamTools(config);
      discoveredByConfig = { ...discoveredByConfig, [config.id]: tools };
      if ((config.allowed_tools ?? []).length > 0) {
        update(index, {
          allowed_tools: [],
          disabled_tools: tools.filter((tool) => !config.allowed_tools.includes(tool.name)).map((tool) => tool.name).sort(),
        });
      }
    } catch (error) {
      discoveryErrorByConfig = { ...discoveryErrorByConfig, [config.id]: `获取工具失败：${String(error)}` };
    } finally {
      discoveringByConfig = { ...discoveringByConfig, [config.id]: false };
    }
  }

  function validHeaderName(value: string): boolean {
    return /^[!#$%&'*+.^_`|~0-9A-Za-z-]+$/.test(value);
  }

  function validateOne(config: UpstreamMcpConfig, requireTransportFields: boolean): string {
    if (!config.name.trim()) return "每个本地 MCP 都需要名称。";
    if (!/^[a-z0-9-]+$/.test(config.tool_prefix)) return `「${config.name}」的工具前缀只能使用小写字母、数字和短横线。`;
    if (config.type === "stdio" && (config.enabled || requireTransportFields) && !config.command.trim()) return `「${config.name}」缺少 stdio 启动命令。`;
    if (config.type === "streamableHttp") {
      try {
        const url = new URL(config.url);
        if (!["http:", "https:"].includes(url.protocol) || !url.hostname) throw new Error("invalid");
      } catch {
        return `「${config.name}」需要有效的 http 或 https Streamable HTTP 地址。`;
      }
      for (const [key, value] of Object.entries(config.headers)) {
        if (!validHeaderName(key) || /[\r\n]/.test(value)) return `「${config.name}」包含无效 HTTP 请求头。`;
      }
    }
    if (new Set(config.allowed_tools).size !== config.allowed_tools.length) return `「${config.name}」的工具白名单有重复项。`;
    if (new Set(config.disabled_tools ?? []).size !== (config.disabled_tools ?? []).length) return `「${config.name}」的关闭工具列表有重复项。`;
    return "";
  }

  function validate(next: UpstreamMcpConfig[]): string {
    const prefixes = new Set<string>();
    for (const config of next) {
      const error = validateOne(config, false);
      if (error) return error;
      if (config.enabled && prefixes.has(config.tool_prefix)) return `已启用的工具前缀重复：${config.tool_prefix}`;
      if (config.enabled) prefixes.add(config.tool_prefix);
    }
    return "";
  }

  function isRecord(value: unknown): value is UnknownRecord {
    return typeof value === "object" && value !== null && !Array.isArray(value);
  }

  function stringRecord(value: unknown, field: string): Record<string, string> {
    if (value === undefined) return {};
    if (!isRecord(value)) throw new Error(`${field} 必须是键值对象。`);
    const result: Record<string, string> = {};
    for (const [key, item] of Object.entries(value)) {
      if (typeof item !== "string") throw new Error(`${field}.${key} 必须是字符串。`);
      result[key] = item;
    }
    return result;
  }

  function stringList(value: unknown, field: string): string[] {
    if (value === undefined) return [];
    if (!Array.isArray(value) || value.some((item) => typeof item !== "string")) throw new Error(`${field} 必须是字符串数组。`);
    return [...value] as string[];
  }

  function normalizedTransport(value: unknown, raw: UnknownRecord): Transport {
    if (value === undefined) return typeof raw.url === "string" ? "streamableHttp" : "stdio";
    if (value === "stdio") return "stdio";
    if (value === "streamableHttp" || value === "streamable_http" || value === "http") return "streamableHttp";
    throw new Error("type 必须是 stdio 或 streamableHttp。");
  }

  function slug(value: string): string {
    const normalized = value.toLowerCase().replace(/[^a-z0-9-]+/g, "-").replace(/^-+|-+$/g, "");
    return normalized || "local";
  }

  function importedConfig(raw: unknown, fallbackName?: string): UpstreamMcpConfig {
    if (!isRecord(raw)) throw new Error("每个 MCP 配置必须是对象。");
    const name = typeof raw.name === "string" && raw.name.trim() ? raw.name.trim() : (fallbackName || "本地 MCP");
    const type = normalizedTransport(raw.type ?? raw.transport, raw);
    const config: UpstreamMcpConfig = {
      id: newId(),
      name,
      enabled: false,
      type,
      command: typeof raw.command === "string" ? raw.command : "",
      args: stringList(raw.args, "args"),
      env: stringRecord(raw.env, "env"),
      url: typeof raw.url === "string" ? raw.url : "",
      headers: stringRecord(raw.headers, "headers"),
      tool_prefix: typeof raw.tool_prefix === "string" ? raw.tool_prefix : (typeof raw.toolPrefix === "string" ? raw.toolPrefix : slug(name)),
      allowed_tools: stringList(raw.allowed_tools ?? raw.allowedTools, "allowed_tools"),
      disabled_tools: stringList(raw.disabled_tools ?? raw.disabledTools, "disabled_tools"),
    };
    const error = validateOne(config, true);
    if (error) throw new Error(error);
    return config;
  }

  function importFromJson() {
    importError = "";
    try {
      const parsed: unknown = JSON.parse(importText);
      let imported: UpstreamMcpConfig[];
      if (isRecord(parsed) && isRecord(parsed.mcpServers)) {
        imported = Object.entries(parsed.mcpServers).map(([name, config]) => importedConfig(config, name));
      } else if (Array.isArray(parsed)) {
        imported = parsed.map((config) => importedConfig(config));
      } else {
        imported = [importedConfig(parsed)];
      }
      if (imported.length === 0) throw new Error("JSON 中没有可导入的 MCP 配置。");
      draft = [...draft, ...imported];
      importText = "";
      importOpen = false;
    } catch (error) {
      importError = `导入失败：${error instanceof Error ? error.message : String(error)}`;
    }
  }

  async function save() {
    if (saving) return;
    const next = cloneConfigs(draft);
    validationError = validate(next);
    if (validationError) return;
    saving = true;
    try {
      await onSave(next);
    } finally {
      saving = false;
    }
  }
</script>

<div class="grid gap-4">
  <div class="flex flex-wrap items-center justify-between gap-2">
    <div>
      <p class="text-sm font-medium">本地 MCP</p>
      <p class="mt-1 text-xs text-[var(--color-text-muted)]">仅代理管理员显式配置的 stdio 或 Streamable HTTP 服务。启用后自动发现并默认公开所有工具；可按工具逐项关闭。</p>
    </div>
    <div class="relative">
      <button type="button" class="tx-btn-ghost" aria-expanded={addMenuOpen} onclick={() => addMenuOpen = !addMenuOpen}>+ 添加⌄</button>
      {#if addMenuOpen}
        <div class="absolute right-0 z-10 mt-1 grid min-w-32 overflow-hidden rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] py-1 shadow-lg">
          <button type="button" class="px-3 py-2 text-left text-sm hover:bg-[var(--color-surface-hover)]" onclick={quickCreate}>快速创建</button>
          <button type="button" class="px-3 py-2 text-left text-sm hover:bg-[var(--color-surface-hover)]" onclick={openImport}>从 JSON 导入</button>
        </div>
      {/if}
    </div>
  </div>

  {#if importOpen}
    <section class="grid gap-3 rounded-lg border border-[var(--color-border)] p-4">
      <div>
        <p class="text-sm font-medium">从 JSON 导入</p>
        <p class="mt-1 text-xs text-[var(--color-text-muted)]">支持单个配置、配置数组，或 <code>{`{ "mcpServers": { "名称": 配置 } }`}</code>。导入后默认关闭，可编辑确认后再保存。</p>
      </div>
      <textarea class="tx-input min-h-40 font-mono text-xs" bind:value={importText} placeholder={'{\n  "mcpServers": {\n    "local-http": {\n      "type": "streamableHttp",\n      "url": "http://127.0.0.1:3000/mcp",\n      "headers": { "Authorization": "Bearer …" }\n    }\n  }\n}'}></textarea>
      {#if importError}<p class="text-sm text-[var(--danger)]">{importError}</p>{/if}
      <div class="flex justify-end gap-2">
        <button type="button" class="tx-btn-ghost" onclick={() => { importOpen = false; importError = ""; }}>取消</button>
        <button type="button" class="tx-btn-ghost" onclick={importFromJson}>导入草稿</button>
      </div>
    </section>
  {/if}

  {#if draft.length === 0}
    <p class="rounded-md border border-dashed border-[var(--color-border)] p-3 text-sm text-[var(--color-text-muted)]">还没有本地 MCP。未配置时，本应用的工具目录和行为保持不变。</p>
  {/if}

  {#each draft as config, index (config.id)}
    <section class="grid gap-3 rounded-lg border border-[var(--color-border)] p-4">
      <div class="flex items-center justify-between gap-3">
        <label class="flex items-center gap-2 text-sm font-medium">
          <input type="checkbox" checked={config.enabled} onchange={(event) => update(index, { enabled: (event.currentTarget as HTMLInputElement).checked })} />
          启用 {config.name || "本地 MCP"}
        </label>
        <button type="button" class="text-sm text-[var(--danger)]" onclick={() => remove(index)}>移除</button>
      </div>
      <div class="grid gap-3 md:grid-cols-3">
        <label class="grid gap-1"><span class="text-xs text-[var(--color-text-muted)]">名称</span><input class="tx-input" value={config.name} oninput={(event) => update(index, { name: (event.currentTarget as HTMLInputElement).value })} /></label>
        <label class="grid gap-1"><span class="text-xs text-[var(--color-text-muted)]">传输</span><select class="tx-input" value={config.type} onchange={(event) => update(index, { type: (event.currentTarget as HTMLSelectElement).value as Transport })}><option value="stdio">stdio（本机进程）</option><option value="streamableHttp">Streamable HTTP</option></select></label>
        <label class="grid gap-1"><span class="text-xs text-[var(--color-text-muted)]">工具前缀</span><input class="tx-input font-mono" value={config.tool_prefix} oninput={(event) => update(index, { tool_prefix: (event.currentTarget as HTMLInputElement).value.trim() })} placeholder="local" /></label>
      </div>

      {#if config.type === "streamableHttp"}
        <label class="grid gap-1"><span class="text-xs text-[var(--color-text-muted)]">Streamable HTTP 地址</span><input class="tx-input font-mono" value={config.url} oninput={(event) => update(index, { url: (event.currentTarget as HTMLInputElement).value })} placeholder="http://127.0.0.1:3000/mcp" /></label>
        <label class="grid gap-1"><span class="text-xs text-[var(--color-text-muted)]">请求头（每行 KEY=VALUE，仅发送给此 HTTP 地址）</span><textarea class="tx-input min-h-24 font-mono" value={recordText(config.headers)} oninput={(event) => update(index, { headers: parseRecord((event.currentTarget as HTMLTextAreaElement).value) })}></textarea></label>
      {:else}
        <label class="grid gap-1"><span class="text-xs text-[var(--color-text-muted)]">启动命令</span><input class="tx-input font-mono" value={config.command} oninput={(event) => update(index, { command: (event.currentTarget as HTMLInputElement).value })} placeholder="C:\\Program Files\\nodejs\\node.exe" /></label>
        <label class="grid gap-1"><span class="text-xs text-[var(--color-text-muted)]">参数（每行一个，不经 shell 执行）</span><textarea class="tx-input min-h-16 font-mono" value={config.args.join("\n")} oninput={(event) => update(index, { args: lines((event.currentTarget as HTMLTextAreaElement).value) })}></textarea></label>
        <label class="grid gap-1"><span class="text-xs text-[var(--color-text-muted)]">环境变量（每行 KEY=VALUE，仅注入本机子进程）</span><textarea class="tx-input min-h-24 font-mono" value={recordText(config.env)} oninput={(event) => update(index, { env: parseRecord((event.currentTarget as HTMLTextAreaElement).value) })}></textarea></label>
      {/if}

      <div class="grid gap-2 rounded-md border border-[var(--color-border)] p-3">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <div><p class="text-sm font-medium">已发现工具</p><p class="mt-1 text-xs text-[var(--color-text-muted)]">点击获取工具后，可逐项决定是否对公网 MCP 公开。未关闭的工具默认公开。</p></div>
          <button type="button" class="tx-btn-ghost" disabled={discoveringByConfig[config.id]} onclick={() => void discover(index)}>{discoveringByConfig[config.id] ? "获取中…" : "获取工具"}</button>
        </div>
        {#if discoveryErrorByConfig[config.id]}
          <p class="text-sm text-[var(--danger)]">{discoveryErrorByConfig[config.id]}</p>
        {:else if discoveredTools(config).length > 0}
          <div class="grid gap-2">
            {#each discoveredTools(config) as tool (tool.name)}
              <label class="flex items-start justify-between gap-3 rounded border border-[var(--color-border)] px-3 py-2">
                <span class="grid gap-1"><code class="text-xs">{tool.name}</code>{#if tool.description}<span class="text-xs text-[var(--color-text-muted)]">{tool.description}</span>{/if}</span>
                <span class="flex shrink-0 items-center gap-2 text-sm">公开<input type="checkbox" checked={isToolEnabled(config, tool.name)} onchange={(event) => setToolEnabled(index, tool.name, (event.currentTarget as HTMLInputElement).checked)} /></span>
              </label>
            {/each}
          </div>
        {:else}
          <p class="text-xs text-[var(--color-text-muted)]">尚未获取工具。保存后服务仍会自动发现并默认公开工具；如需逐项关闭，请先获取工具。</p>
        {/if}
      </div>
      <p class="text-xs text-[var(--color-text-muted)]">远端工具名将为 <code>{config.tool_prefix || "prefix"}__原始工具名</code>。已有旧版白名单配置会保持原有公开范围，直到你保存本页的开关。</p>
    </section>
  {/each}

  {#if validationError}<p class="text-sm text-[var(--danger)]">{validationError}</p>{/if}
  <div class="flex justify-end"><button type="button" class="rounded-md bg-[var(--color-accent)] px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50" disabled={saving} onclick={() => void save()}>{saving ? "保存中…" : "保存本地 MCP"}</button></div>
</div>
