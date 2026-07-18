# 任务清单：history-session-archive

## 概述

以纯增量方式实现 ChatGPT 网页版会话历史归档。严格执行 RED-GREEN-REFACTOR，现有工具行为不变。

> **二元禁令**：交付物中零容忍模板占位标记、待办标记和省略实现。动手前读取证据文件并执行 GitNexus 影响分析；单文件超过 500 行必须拆分。

---

## 交付物清单（Scope-lock）

- 预计实现文件数: 9
- 预计任务数: 8

---

## 任务列表

### 阶段 1: 契约与 RED 测试

- [x] 1.1 扩展历史工具契约测试，覆盖 tools/list、Schema 和 ChatGPT 会话元数据注入
  - 证据块: `src-tauri/src/mcp/server.rs:8`、`src-tauri/src/tools/registry.rs:3`
  - 文件: `src-tauri/src/mcp/server.rs`、`src-tauri/src/tools/registry.rs`（各预算新增不超过 180 行）
  - _需求: FR-1, FR-7_ · _设计: API 设计、决策 1_
- [x] 1.2 新建历史领域 RED 测试，覆盖编号、摘要、幂等、校验、脱敏、路径和并发
  - 证据块: `src-tauri/src/tools/workspace.rs:87`、`docs/project-context/how-to-test.md:1`
  - 文件: `src-tauri/src/tools/history/mod.rs`、`storage.rs`、`markdown.rs`（每文件测试与实现合计不超过 500 行）
  - _需求: FR-1, FR-2, FR-3, FR-4, FR-5, FR-6_ · _设计: 数据模型、架构设计_
- [x] 1.3 新增持久化提示 RED 测试，覆盖初始化 instructions、工具描述和 bootstrap 结构化指令
  - 证据块: `src-tauri/src/mcp/server.rs:34`、`src-tauri/src/tools/registry.rs:29`、`src-tauri/src/tools/history/mod.rs:14`
  - 文件: `src-tauri/src/mcp/server.rs`、`src-tauri/tests/history_session.rs`
  - _需求: FR-8_ · _设计: 决策 5_

---

### 阶段 2: 核心实现

- [x] 2.1 新建历史模型和 Markdown 处理，确定性解析摘要、更新 turn 块并脱敏
  - 证据块: `docs/specs/history-session-archive/design.md:34`
  - 文件: `src-tauri/src/tools/history/model.rs`、`src-tauri/src/tools/history/markdown.rs`（各预算不超过 350 行）
  - _需求: FR-2, FR-3, FR-6_ · _设计: 数据模型_
- [x] 2.2 新建跨平台存储层，实现工作区边界、文件锁、扫描、索引和原子替换
  - 证据块: `src-tauri/src/tools/workspace.rs:87`、`src-tauri/Cargo.toml:1`
  - 文件: `src-tauri/src/tools/history/storage.rs`、`src-tauri/Cargo.toml`（存储文件预算不超过 480 行）
  - _需求: FR-4, FR-5, NFR-1, NFR-3_ · _设计: 技术选型、决策 3_
- [x] 2.3 实现 bootstrap、checkpoint、validate 用例编排和结构化结果
  - 证据块: `src-tauri/src/tools/dispatch.rs:38`、`src-tauri/src/tools/workspace.rs:15`
  - 文件: `src-tauri/src/tools/history/mod.rs`（预算不超过 480 行）
  - _需求: FR-1, FR-2, FR-3, FR-4_ · _设计: API 设计_
- [x] 2.4 注册三个工具并接入唯一分发入口，保持原工具分支不变
  - 证据块: `src-tauri/src/tools/mod.rs:1`、`src-tauri/src/tools/registry.rs:3`、`src-tauri/src/tools/dispatch.rs:38`
  - 文件: `src-tauri/src/tools/mod.rs`、`registry.rs`、`dispatch.rs`（合计新增不超过 260 行）
  - _需求: FR-7_ · _设计: 决策 4_
- [x] 2.5 注入 ChatGPT openai/session，仅影响 history_session 工具参数
  - 证据块: `src-tauri/src/mcp/server.rs:48`
  - 文件: `src-tauri/src/mcp/server.rs`（预算新增不超过 80 行）
  - _需求: FR-1, FR-7_ · _设计: 决策 1_
- [x] 2.6 增加服务器级与会话级持久化工作流提示，不修改现有工具执行路径
  - 证据块: OpenAI Apps SDK `instructions` 与工具描述契约
  - 文件: `src-tauri/src/mcp/server.rs`、`src-tauri/src/tools/registry.rs`、`src-tauri/src/tools/history/mod.rs`
  - _需求: FR-8_ · _设计: 决策 5_
- [x] 2.7 在每个工作区的 MCP GPT 配置区增加会话恢复提示词与一键复制反馈
  - 证据块: `src/lib/components/GptQuickCopy.svelte`
  - 文件: `src/lib/components/ChatGptSessionPrompt.svelte`、`src/lib/components/GptQuickCopy.svelte`
  - _需求: FR-8_ · _设计: 决策 5_

---

### 阶段 3: 集成验证

- [x] 3.1 运行格式化、历史模块测试、全量 Rust 测试和 Clippy，并核对现有工具契约无回归
  - 证据块: `docs/project-context/how-to-test.md:18`、`src-tauri/src/tools/registry.rs:180`、`src-tauri/src/tools/dispatch.rs:38`
- 验收点: FR-1 至 FR-9、NFR-1 至 NFR-5；原需求与工具刷新回归场景
  - _需求: FR-1, FR-2, FR-3, FR-4, FR-5, FR-6, FR-7, FR-8, FR-9_
- [x] 3.2 修正 ChatGPT 工具目录刷新声明与 checkpoint 路由兼容性
  - 将 `listChanged` 与实际通知能力保持一致
  - checkpoint 描述改为纯能力说明，`turn_id` 缺失时稳定生成
  - 工作区页面增加重新配置连接并新开会话的升级提示
  - _需求: FR-8, FR-9_

---

## 检查点

- [x] 阶段 1 完成后：新增测试因缺少历史模块或行为而按预期 RED，非测试配置错误
- [x] 阶段 2 完成后：同一相关测试目标转为 GREEN，新增模块单文件均不超过 500 行
- [x] 阶段 3 完成后：本次变更文件 `rustfmt --check`、全量 `cargo test`、`cargo clippy --all-targets -- -D warnings` 通过；全仓 `cargo fmt --check` 仍受既有未格式化文件影响

---

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---------|----------|----------|------|
| FR-1 | API 设计、决策 1 | 1.1, 1.2, 2.3, 2.5, 3.1 | 已完成 |
| FR-2 | 数据模型、决策 2 | 1.2, 2.1, 2.3, 3.1 | 已完成 |
| FR-3 | 数据模型、API 设计 | 1.2, 2.1, 2.3, 3.1 | 已完成 |
| FR-4 | 数据模型、决策 3 | 1.2, 2.2, 2.3, 3.1 | 已完成 |
| FR-5 | 技术选型、风险评估 | 1.2, 2.2, 3.1 | 已完成 |
| FR-6 | 数据模型、风险评估 | 1.2, 2.1, 3.1 | 已完成 |
| FR-7 | 架构设计、决策 4 | 1.1, 2.4, 2.5, 3.1 | 已完成 |
| FR-8 | 决策 5 | 1.3, 2.6, 2.7, 3.1 | 已完成 |
| FR-9 | 决策 6 | 3.2 | 已完成 |

---

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|------|------|----------|------|
| `src-tauri/Cargo.toml` | 修改 | 新增不超过 5 | 文件锁和 Windows 原子替换 feature |
| `src-tauri/src/mcp/server.rs` | 修改 | 新增不超过 180 | 会话元数据注入和集成测试 |
| `src-tauri/src/tools/mod.rs` | 修改 | 新增 1 | 导出 history 模块 |
| `src-tauri/src/tools/registry.rs` | 修改 | 新增不超过 180 | 工具定义、Profile、Schema |
| `src-tauri/src/tools/dispatch.rs` | 修改 | 新增不超过 20 | 三个历史分发分支 |
| `src-tauri/src/tools/history/mod.rs` | 新建 | 不超过 480 | 用例编排与测试 |
| `src-tauri/src/tools/history/model.rs` | 新建 | 不超过 220 | 数据模型 |
| `src-tauri/src/tools/history/storage.rs` | 新建 | 不超过 480 | 锁、扫描、索引、原子写入 |
| `src-tauri/src/tools/history/markdown.rs` | 新建 | 不超过 350 | Markdown 和脱敏 |

---

## 交付前自检

- [x] 无占位符、TODO 或省略实现
- [x] 9 个实现文件全部完成且没有范围外重构
- [x] 每个文件不超过 500 行、每条任务回链 FR
- [ ] 新增核心逻辑覆盖率不低于 80%
- [x] 未引入 OpenAI SDK，未改变现有工具行为
