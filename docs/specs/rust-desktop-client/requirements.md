# 需求文档：rust-desktop-client

## 功能概述

用 Rust/Tauri 2 重构 Coding Tools MCP 桌面客户端。目标是单二进制跨平台桌面应用，内嵌 MCP 核心（不再依赖外部 Python 子进程），提供 Workspace-first 管理界面、本地 HTTP `/mcp` 端点、FRP/Cloudflare 公网隧道、OAuth/Bearer/NoAuth 认证、健康检查与日志查看。参考 `old/` 目录中的 Python 实现，以 `old/docs/profile-v0.1.md` 和 `old/tests/compliance/` 为行为契约。

## 历史经验与坑

- **可复用经验**: 旧版 `old/docs/specs/mcp-desktop-client/` 已定义 Workspace-first 产品模型和 FRP/OAuth 需求，可直接继承 FR 定义
- **必须规避的坑**: 旧版 PySide6 客户端通过 psutil 启发式猜 PID 管理外部 Python 进程，导致重启后状态不一致、Cloudflare 隧道靠 regex 扒日志超时。新版必须内嵌 MCP 核心并使用显式状态机

## 术语定义

- **Workspace**: 用户选定的本地项目目录，是所有 MCP 操作的边界
- **MCP Runtime**: 内嵌在 Tauri 后端的 MCP 服务器，监听本地 HTTP 端口
- **Tunnel**: 将本地 MCP 端口暴露到公网的机制（FRP 或 Cloudflare）
- **WorkspaceProfile**: 一个 Workspace 的完整配置（路径、端口、隧道、认证）

---

## 范围边界

**In Scope（Phase 1 MVP）**
- Tauri 2 桌面壳 + Svelte 前端
- Workspace CRUD 与持久化
- 内嵌 MCP 核心（P0 工具集）
- Runtime 状态机（启动/停止/状态查询）
- FRP 配置生成 + Cloudflare 隧道管理
- OAuth / Bearer / NoAuth 认证配置
- 系统钥匙串密钥存储
- 健康检查（本地 /mcp、公网 endpoint、OAuth 元数据）
- 日志查看
- P0 合规测试移植

**Out of Scope（Phase 1 不做）**
- Actions 网关（GPT Actions，Phase 2）
- ngrok / Dev Tunnel 适配器（Phase 2）
- 全部 17 个 MCP 工具（Phase 1 仅 P0 子集）
- 系统托盘、自动恢复（Phase 3）
- 团队共享 profile（Phase 3）
- Landlock 沙箱（Linux 特有，Phase 2）

---

## 需求列表

### FR-1: Workspace-first 首页

**优先级:** Must
**用户故事:** 作为开发者，我想要以 Workspace 卡片为首页，以便快速看到各项目的运行状态和连接地址。

#### 验收标准（EARS）

1. WHEN 用户打开应用 THEN 系统 SHALL 展示 Workspace 卡片列表
2. WHEN 卡片渲染 THEN 每张卡片 SHALL 显示名称、路径、隧道类型、运行状态、endpoint 摘要
3. WHEN 用户点击卡片 THEN 系统 SHALL 进入该 Workspace 的详情控制面板

### FR-2: Workspace 管理

**优先级:** Must
**用户故事:** 作为开发者，我想要添加、编辑、删除和切换 Workspace，以便管理多个项目。

#### 验收标准（EARS）

1. WHEN 用户点击添加 THEN 系统 SHALL 弹出目录选择器
2. WHEN 用户选择目录 THEN 系统 SHALL 创建 WorkspaceProfile 并持久化
3. WHEN 用户编辑名称或配置 THEN 系统 SHALL 保存变更
4. WHEN 用户删除 Workspace THEN 系统 SHALL 停止关联 runtime 并移除配置
5. WHEN 应用重启 THEN 系统 SHALL 恢复上次选中的 Workspace

### FR-3: 内嵌 MCP Runtime 生命周期

**优先级:** Must
**用户故事:** 作为开发者，我想要一键启动/停止内嵌 MCP 服务，以便无需管理外部 Python 进程。

#### 验收标准（EARS）

1. WHEN 用户点击启动 THEN 系统 SHALL 在配置的本地端口启动内嵌 MCP HTTP server
2. WHEN 启动成功 THEN 系统 SHALL 显示运行状态和监听端口
3. WHEN 用户点击停止 THEN 系统 SHALL 关闭 HTTP server 并释放端口
4. IF 端口被占用 THEN 系统 SHALL 显示占用进程信息并拒绝启动
5. WHEN 启动或停止完成 THEN 系统 SHALL 在 500ms 内更新 UI 状态

### FR-4: P0 MCP 工具实现

**优先级:** Must
**用户故事:** 作为 MCP 客户端用户，我想要通过 `/mcp` 端点调用核心编码工具，以便完成日常开发任务。

#### 验收标准（EARS）

1. WHEN 客户端调用 `tools/list` THEN 系统 SHALL 返回 P0 工具列表（read_file, list_dir, list_files, search_text, apply_patch, exec_command, git_status, git_diff, server_info, get_default_cwd, set_default_cwd）
2. WHEN 客户端调用任一 P0 工具 THEN 系统 SHALL 返回符合 `old/docs/profile-v0.1.md` 定义的结果 envelope
3. WHEN 路径包含 `..` 或越出 workspace THEN 系统 SHALL 拒绝并返回错误
4. WHEN 移植的合规测试运行 THEN P0 工具相关测试 SHALL 全部 PASS

### FR-5: 公网隧道暴露

**优先级:** Must
**用户故事:** 作为开发者，我想要将本地 MCP 暴露到公网，以便远程 MCP 客户端（如 ChatGPT）连接。

#### 验收标准（EARS）

1. WHEN 用户选择 FRP 方式 THEN 系统 SHALL 生成 FRP proxy 配置片段并展示公网 URL
2. WHEN 用户选择 Cloudflare 临时隧道 THEN 系统 SHALL 启动 cloudflared 并解析公网 URL
3. WHEN 用户选择 Cloudflare 固定域名 THEN 系统 SHALL 使用 Tunnel Token 启动命名隧道
4. IF cloudflared 未安装 THEN 系统 SHALL 提示安装方式
5. IF 隧道启动超时（15 秒） THEN 系统 SHALL 显示错误日志并回退到 Error 状态

### FR-6: 认证模式配置

**优先级:** Must
**用户故事:** 作为开发者，我想要配置 OAuth、Bearer 或 NoAuth，以便控制远程访问安全。

#### 验收标准（EARS）

1. WHEN 用户选择 OAuth THEN 系统 SHALL 生成 client_id、client_secret、password 并持久化到钥匙串
2. WHEN 用户选择 Bearer THEN 系统 SHALL 支持 token 生成、保存和复制
3. WHEN 用户选择 NoAuth THEN 系统 SHALL 以无认证模式启动（仅建议本地使用）
4. WHEN 认证模式切换 THEN UI SHALL 动态展示对应配置项

### FR-7: 密钥安全存储

**优先级:** Must
**用户故事:** 作为开发者，我想要密钥存储在系统钥匙串中，以便避免明文泄露。

#### 验收标准（EARS）

1. WHEN 保存 OAuth/Bearer 密钥 THEN 系统 SHALL 写入系统钥匙串（Windows Credential Manager / macOS Keychain / Linux Secret Service）
2. WHEN 配置文件持久化 THEN 系统 SHALL 不包含任何密钥明文
3. WHEN 应用重启 THEN 系统 SHALL 从钥匙串恢复密钥

### FR-8: 健康检查与诊断

**优先级:** Should
**用户故事:** 作为开发者，我想要查看连接健康状态，以便快速定位连接问题。

#### 验收标准（EARS）

1. WHEN 用户查看健康面板 THEN 系统 SHALL 检查本地 `/mcp`、公网 `/mcp`、OAuth 元数据端点
2. WHEN 检查完成 THEN 每项 SHALL 显示 pass/fail 状态和 HTTP 详情
3. IF 检查失败 THEN 系统 SHALL 显示建议修复动作

### FR-9: 日志查看

**优先级:** Should
**用户故事:** 作为开发者，我想要查看 MCP 和隧道日志，以便排查启动问题。

#### 验收标准（EARS）

1. WHEN runtime 运行中 THEN 系统 SHALL 提供 MCP server 日志流
2. WHEN 隧道运行中 THEN 系统 SHALL 提供隧道进程日志
3. WHEN 用户切换 Workspace THEN 系统 SHALL 切换到对应日志

### FR-10: 一键复制连接信息

**优先级:** Should
**用户故事:** 作为开发者，我想要一键复制 MCP endpoint 和认证信息，以便快速配置远程客户端。

#### 验收标准（EARS）

1. WHEN 用户点击复制 MCP 地址 THEN 系统 SHALL 将完整 endpoint URL 写入剪贴板
2. WHEN OAuth 模式 THEN 系统 SHALL 支持复制 client_id、client_secret、授权口令
3. WHEN FRP 模式 THEN 系统 SHALL 支持复制 FRP proxy 配置片段

---

## 非功能需求

- **NFR-1（性能）**: 应用冷启动到 UI 可交互 < 3 秒；MCP 启动到端口监听 < 5 秒
- **NFR-2（安全）**: 密钥不得出现在日志、配置文件或 IPC 明文传输中；workspace 路径边界不可绕过
- **NFR-3（兼容性）**: 支持 Windows 10+、macOS 12+、Linux（x86_64）；MCP 协议版本 2025-06-18
- **NFR-4（可维护性）**: 单文件不超过 500 行；Rust 模块按职责拆分

---

## 依赖关系

- `old/docs/profile-v0.1.md` — MCP 工具行为契约
- `old/tests/compliance/` — 合规测试基线
- `old/apps/desktop-client/` — UI 交互和配置模型参考
- 外部依赖：cloudflared CLI（Cloudflare 隧道）、FRP 服务端（FRP 模式）
- Rust crate：tauri, tokio, axum, rmcp, git2, keyring, serde

---

## 检查清单

- [x] 已消化记忆库的历史经验，并逐条规避历史坑
- [x] 需求覆盖核心场景与边界场景
- [x] 每条需求有唯一 ID（FR-1 至 FR-10）
- [x] 验收标准使用 EARS 格式且可测
- [x] 已标注优先级（MoSCoW）
- [x] 范围边界（In/Out of Scope）明确
- [x] 非功能需求明确、尽量可量化
- [x] 依赖关系完整
