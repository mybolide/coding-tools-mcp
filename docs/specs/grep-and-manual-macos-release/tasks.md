# 任务清单：grep 与手动 macOS 发布

## 概述

按工具契约、macOS 平台能力、手动发布和交付验证四个阶段完成 FR-1 至 FR-4。

## 交付物清单（Scope-lock）

- **预计新建文件数**：4 个规格/工作流文件。
- **预计修改文件数**：5 个 Rust 源码或测试文件。
- **预计新增/修改函数数**：约 5 个。
- **交付物逐项列举**：
  1. `docs/specs/grep-and-manual-macos-release/requirements.md`
  2. `docs/specs/grep-and-manual-macos-release/design.md`
  3. `docs/specs/grep-and-manual-macos-release/tasks.md`
  4. `.github/workflows/macos-release.yml`
  5. `src-tauri/src/tools/registry.rs`
  6. `src-tauri/src/tools/dispatch.rs`
  7. `src-tauri/src/platform/macos/mod.rs`
  8. `src-tauri/src/platform/macos/process.rs`
  9. `src-tauri/tests/call_tool_contract.rs`

## 任务列表

### 阶段 1：冻结工具契约

- [ ] 1.1 确认 search_text 能力并确定 grep 仅作为兼容别名
  - **证据块**：`src-tauri/src/tools/file.rs:205-303` 已实现 query、路径、glob、正则、大小写、上下文和截断；`src-tauri/src/tools/registry.rs:562-578` 已定义 schema。
  - **涉及文件**：规格文档，约 200 行。
  - _需求：FR-1, FR-2_ ｜ _设计：技术方案、决策 1_

### 阶段 2：核心实现

- [ ] 2.1 在统一注册表和调度层暴露 grep，共用 search_text schema 与实现
  - **证据块**：`src-tauri/src/tools/dispatch.rs:107-140` 统一调度工具；`src-tauri/src/tools/registry.rs` 统一生成 MCP 与 Actions 工具声明。
  - **涉及文件**：`registry.rs` 约 10 行，`dispatch.rs` 约 4 行。
  - _需求：FR-1, FR-2_ ｜ _设计：API 设计、决策 1_

- [ ] 2.2 补齐 macOS 按镜像路径清理 frpc，保持唯一进程生命周期
  - **证据块**：`src-tauri/src/platform/mod.rs:29-31` 默认实现不清理；`macos/process.rs` 已具备 PID 枚举、镜像路径读取和进程树终止。
  - **涉及文件**：`macos/mod.rs` 约 5 行，`macos/process.rs` 约 45 行。
  - _需求：FR-3_ ｜ _设计：macOS 进程管理_

- [ ] 2.3 新增仅 workflow_dispatch 的 macOS 通用 DMG 工作流
  - **证据块**：`.github/workflows/ci.yml` 仅在 Ubuntu 验证，当前没有 macOS 构建产物。
  - **涉及文件**：`.github/workflows/macos-release.yml` 约 70 行。
  - _需求：FR-3, FR-4_ ｜ _设计：macOS 构建、决策 2 与决策 3_

### 阶段 3：集成测试

- [ ] 3.1 增加 grep 工具清单、schema、执行和默认 cwd 契约测试
  - **证据块**：`src-tauri/tests/call_tool_contract.rs:156` 已以 CORE_TOOLS 校验 MCP 清单，`339-360` 已覆盖 search_text glob。
  - **涉及文件**：`call_tool_contract.rs` 约 45 行。
  - _需求：FR-1, FR-2_ ｜ _设计：测试策略_

- [ ] 3.2 执行 Windows 全量回归并在 GitHub 手动验证 macOS 通用构建
  - **证据块**：当前 Windows 基线为 Rust 109 tests、Clippy 零问题、Svelte 0 错误 0 警告、NSIS 0.1.5 成功。
  - **涉及文件**：不新增源码；产出测试与 GitHub run 记录。
  - _需求：FR-3, FR-4_ ｜ _设计：测试策略_

### 阶段 4：提交与发布

- [ ] 4.1 审查差异、提交并推送当前版本
  - **证据块**：提交前执行 GitNexus 变更检测、代码审查和 gencommit。
  - **涉及文件**：本清单锁定的交付物及此前已验证的 0.1.5 修复。
  - _需求：FR-1, FR-2, FR-3, FR-4_ ｜ _设计：全部章节_

- [ ] 4.2 上传 Windows 安装包并按用户本次明确要求手动触发 macOS 打包
  - **证据块**：Windows NSIS 路径为 `src-tauri/target/release/bundle/nsis/Coding Tools MCP_0.1.5_x64-setup.exe`。
  - **涉及文件**：GitHub Release 资产与 Actions artifact。
  - _需求：FR-3, FR-4_ ｜ _设计：手动发布流程_

## 检查点

- [ ] 阶段 1 完成后：规格通过 check_spec，grep 不复制搜索实现。
- [ ] 阶段 2 完成后：工具注册表一致，macOS 工作流不存在自动触发器。
- [ ] 阶段 3 完成后：Windows 全量测试通过，GitHub macOS 构建成功。
- [ ] 阶段 4 完成后：提交已推送，Windows 与 macOS 产物可从 GitHub 获取。

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---------|----------|----------|------|
| FR-1 | API 设计、决策 1 | 1.1, 2.1, 3.1 | 未开始 |
| FR-2 | 架构设计 | 2.1, 3.1 | 未开始 |
| FR-3 | macOS 进程管理与构建 | 2.2, 2.3, 3.2, 4.2 | 未开始 |
| FR-4 | 决策 2 | 2.3, 3.2, 4.2 | 未开始 |

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|------|------|----------|------|
| `src-tauri/src/tools/registry.rs` | 修改 | 10 | 注册 grep 并复用 schema |
| `src-tauri/src/tools/dispatch.rs` | 修改 | 4 | 调度与默认 cwd |
| `src-tauri/src/platform/macos/mod.rs` | 修改 | 5 | 接入进程路径清理 |
| `src-tauri/src/platform/macos/process.rs` | 修改 | 45 | 精确终止本应用 frpc |
| `src-tauri/tests/call_tool_contract.rs` | 修改 | 45 | grep 契约回归 |
| `.github/workflows/macos-release.yml` | 新建 | 70 | 手动通用 macOS 构建 |
