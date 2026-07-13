# 项目图谱洞察

更新时间：2026-07-13

## 分析状态

- GitNexus 索引已刷新：5,338 个节点、10,793 条边、300 条执行流。
- 当前索引对本仓库 Rust 符号的查询仍不稳定，`start_runtime`、`RuntimeSupervisor`、`call_tool` 等目标未能可靠解析。
- 以下结构结论以当前源码和项目文档为准，GitNexus 仅作为辅助索引。

## 项目定位

这是一个 Rust + Tauri 2 + Svelte 的桌面客户端，将 Coding Tools MCP 能力以内嵌 HTTP 服务形式暴露，并同时提供 ChatGPT Actions OpenAPI 网关。每个工作区可以独立运行 MCP 服务、Actions 服务和 FRP/Cloudflare 隧道。

## 主执行链路

```text
Svelte 页面
  → src/lib/api/* 的 Tauri invoke
  → src-tauri/src/commands/*
  → AppState
      ├─ DataStore：工作区、设置和密钥数据
      └─ RuntimeSupervisor：MCP/Actions 生命周期
            ├─ mcp::spawn_listener：/mcp
            └─ actions::spawn_listener：/openapi.json、/actions/{tool}
  → tunnel supervisor：FRP / Cloudflare 公网隧道
```

## 核心模块

### 应用入口与状态

- `src-tauri/src/lib.rs` 注册 Tauri 插件、`AppState` 和全部 IPC commands。
- `src-tauri/src/app_state.rs` 以两个 `Mutex` 管理 `DataStore` 与 `RuntimeSupervisor`。
- `src-tauri/src/commands/mod.rs` 聚合工作区、运行时、隧道、密钥、软件和设置命令。

### 数据与工作区

- `src-tauri/src/data/store.rs` 负责统一数据文件的读取、迁移、保存及工作区 CRUD。
- `src-tauri/src/data/model.rs` 的 `AppData` 聚合 FRP 配置、代理、下载配置、工作区和 secrets。
- `src-tauri/src/workspace/` 定义工作区模型、旧版导入和兼容层。

### 运行时

- `src-tauri/src/runtime/supervisor.rs` 以 `(workspace_id, ServiceKind)` 管理 MCP/Actions 状态。
- 生命周期为 `Stopped → Starting → Running/ Error → Stopping`。
- `src-tauri/src/commands/runtime.rs` 负责端口占用检查、启动/停止、隧道联动及公网 URL 回写。
- `src-tauri/src/runtime/port.rs` 和 `src-tauri/src/platform/windows/net.rs` 提供端口与进程检测。

### HTTP 服务

- `src-tauri/src/mcp/listener.rs` 启动 MCP Streamable HTTP 服务，并接入 Bearer/OAuth/无认证。
- `src-tauri/src/actions/listener.rs` 生成 OpenAPI 文档，暴露 Actions 执行端点和 OAuth 端点。
- 两个 listener 都复用 `src-tauri/src/tools/` 的工具内核和策略配置。

### 隧道

- `src-tauri/src/tunnel/supervisor.rs` 统一管理隧道生命周期。
- `src-tauri/src/tunnel/frp/` 负责 FRP 配置与客户端。
- `src-tauri/src/tunnel/cloudflare.rs` 负责 cloudflared 进程和公网 URL 处理。

### 前端

- `src/routes/+layout.svelte` 加载工作区、刷新 MCP/Actions 状态并承载全局导航和 Toast。
- `src/routes/workspace/[id]/+page.svelte` 是核心工作区页面，管理两个服务、认证、策略、隧道、日志和健康检查。
- `src/lib/api/` 封装 Tauri IPC；`src/lib/components/` 提供配置表单和状态面板；`src/lib/stores/` 管理前端共享状态。

## 当前工作区观察

- 当前有 52 个已修改文件，另有若干新增文件，改动集中在 OAuth、运行时、隧道、数据存储及 UI。
- Rust `cargo check` 通过，但有 16 个 unused/dead-code 警告，表明旧的 `settings::store`、`workspace::store` 和 `secret::keyring_store` 抽象尚未完全清理或接回。
- 当前 `Cargo.toml` 已移除 `keyring` 依赖，但 `DataStore` 将 `shared_secrets`、`workspace_secrets` 和 `app_secrets` 写入 `data/profiles.json`。这与文档中“系统钥匙串存储密钥”的设计目标不一致，应在发布前明确这是临时迁移方案还是需要恢复 OS keyring。
- `docs/project-context/architecture.md` 仍描述“尚无 Rust 源码或 Tauri 工程”，已经落后于实际仓库，需要后续同步。

## 验证结果

- `rtk cargo check --manifest-path src-tauri/Cargo.toml`：通过，16 个警告。
- `rtk cargo test --manifest-path src-tauri/Cargo.toml --no-run`：通过，测试目标可编译。
- `rtk npm run check`：通过，0 错误、0 警告。
- `rtk npm run build`：通过，SvelteKit/Vite 生产构建成功。

## 建议优先级

1. 先决定 secrets 的最终存储边界：恢复系统钥匙串，或明确加密文件方案并补迁移/权限测试。
2. 清理未使用的兼容层，避免 `DataStore` 与 `SecretStore` 两套 API 继续并存。
3. 将 `docs/project-context/architecture.md`、`how-to-test.md` 与实际 Tauri 工程同步。
4. 补充 MCP/Actions/tunnel 的运行时集成测试，尤其是端口冲突、停止等待、OAuth 回调和隧道自动启动失败场景。

---
*来源：当前源码、README、项目上下文文档、Git 状态与构建验证；GitNexus 索引作为辅助。*
