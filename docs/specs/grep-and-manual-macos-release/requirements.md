# 需求文档：grep 与手动 macOS 发布

## 功能概述

为 Coding Tools MCP 增加 Agent 熟悉的 `grep` 工具入口，并复用现有 `search_text` 搜索内核；同时建立 macOS 通用安装包的 GitHub 手动构建流程，使 Windows 与 macOS 用户使用同一版本功能，且任何 macOS 构建都必须由用户明确要求后手动触发。

## 历史经验与坑

- **可复用经验**：现有 `search_text` 已提供内置正则、glob、上下文和截断能力，应复用同一实现与 schema。
- **必须规避的坑**：不能复制第二套搜索逻辑；不能配置 push、tag、release 或 schedule 自动触发 macOS 打包；不能只验证前端而忽略 macOS Rust 平台代码和 frpc 生命周期。

## 术语定义

- **grep**：对 `search_text` 的 MCP 兼容名称，输入和输出契约保持一致。
- **手动发布**：仅由 GitHub Actions `workflow_dispatch` 触发的构建，不响应仓库事件。
- **通用 macOS 包**：同时包含 Apple Silicon 与 Intel 架构的 `universal-apple-darwin` 应用包。

## 范围边界

**In Scope**

- 在 core、read-only、advanced 工具配置中暴露 `grep`。
- `grep` 支持 query、path、glob、正则、大小写、上下文和结果上限。
- MCP tools/list、server_info 与 Actions OpenAPI 使用同一工具注册表。
- macOS 能清理本应用路径下遗留的 frpc 进程。
- 新增仅手动触发的 macOS 通用包构建与可选 Release 上传流程。
- 验证 Windows 构建、Rust 测试、前端检查和 GitHub macOS 构建。

**Out of Scope**

- 不调用或捆绑系统 grep、ripgrep。
- 不删除 `search_text`，避免破坏现有客户端。
- 不配置自动发布、自动触发或定时构建。
- 不在本次申请或生成 Apple Developer 签名证书。

## 需求列表

### FR-1：暴露 grep MCP 工具

**优先级：Must**
**用户故事：** 作为 Coding Agent，我想直接调用 `grep`，以便使用熟悉的搜索工具名完成代码检索。

#### 验收标准

1. WHEN 客户端调用 tools/list THEN 系统 SHALL 在所有适用工具配置中返回 `grep` 及完整输入 schema。
2. WHEN 客户端调用 `grep` THEN 系统 SHALL 复用 `search_text` 的工作区边界、忽略规则、匹配和截断行为。
3. IF 正则无效或路径不可读 THEN 系统 SHALL 返回与 `search_text` 一致的结构化错误。

### FR-2：保持三类工具清单一致

**优先级：Must**
**用户故事：** 作为集成维护者，我想让 MCP、server_info 和 Actions OpenAPI 使用同一注册表，以便避免能力声明与实际执行不一致。

#### 验收标准

1. WHEN 生成 MCP tools/list、server_info 或 Actions OpenAPI THEN 系统 SHALL 均包含当前配置暴露的 `grep`。
2. IF 工具配置为只读 THEN 系统 SHALL 将 `grep` 标记为只读且非破坏性。

### FR-3：兼容 macOS 运行时

**优先级：Must**
**用户故事：** 作为 macOS 用户，我想运行 MCP、Actions 和唯一 frpc 隧道，以便获得与 Windows 相同的主要开发能力。

#### 验收标准

1. WHEN macOS 应用重启或重建 frpc THEN 系统 SHALL 按完整可执行文件路径清理本应用遗留的 frpc。
2. WHEN 构建 macOS 版本 THEN 系统 SHALL 编译 Rust/Tauri 前后端并生成通用架构安装包。
3. IF 未配置 Apple 签名与公证凭据 THEN 系统 SHALL 仍可生成未签名测试包，并明确其分发限制。

### FR-4：仅手动触发 macOS 打包

**优先级：Must**
**用户故事：** 作为发布负责人，我想只在明确要求时触发 macOS 打包，以便控制 GitHub Actions 消耗和发布节奏。

#### 验收标准

1. WHEN 查看 macOS 工作流触发器 THEN 系统 SHALL 只包含 `workflow_dispatch`。
2. WHEN 用户手动提供 Release tag THEN 工作流 SHALL 构建通用 DMG、上传 Actions artifact，并将产物上传到已存在的 Release。
3. IF 未提供 Release tag THEN 工作流 SHALL 只上传 Actions artifact，不创建或修改 Release。

## 非功能需求

- **NFR-1（性能）**：`grep` 不增加第二次目录遍历，相同参数性能与 `search_text` 一致。
- **NFR-2（安全）**：搜索继续遵守 Workspace 只读边界和忽略规则；Release 上传使用最小 `contents: write` 权限。
- **NFR-3（兼容性）**：Windows x64 与 macOS universal-apple-darwin 均可构建；grep 不依赖外部命令。

## 依赖关系

- Rust 工具注册表与文件搜索内核。
- Tauri 2、Node 20、Rust stable 和 GitHub macOS runner。
- macOS Release 正式分发后续依赖 Apple Developer 签名与公证凭据。
