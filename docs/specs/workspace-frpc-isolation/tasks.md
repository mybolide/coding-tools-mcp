# 任务清单：workspace-frpc-isolation

## 概述

按 TDD 将 FRP 生命周期改为工作区隔离，并增加端口及代理资源启动前冲突阻断。每个阶段只提交本任务文件，不处理用户已有工作区改动。

## 交付物清单（Scope-lock）

- **预计新建文件数**：4 个，包括 3 个规格文档和 1 个 Rust 资源校验模块。
- **预计修改文件数**：7 个业务源码文件。
- **预计新增/修改函数数**：约 18 个函数及相关测试。
- **交付物逐项列举**：
  1. `docs/specs/workspace-frpc-isolation/requirements.md`
  2. `docs/specs/workspace-frpc-isolation/design.md`
  3. `docs/specs/workspace-frpc-isolation/tasks.md`
  4. `src-tauri/src/workspace/resources.rs`
  5. `src-tauri/src/workspace/mod.rs`
  6. `src-tauri/src/commands/workspace.rs`
  7. `src-tauri/src/commands/runtime.rs`
  8. `src-tauri/src/commands/tunnel.rs`
  9. `src-tauri/src/tunnel/supervisor.rs`
  10. `src-tauri/src/tunnel/frp/mod.rs`
  11. `src-tauri/src/tunnel/frp/client.rs`
  12. Windows 0.1.19 MSI 与 NSIS 安装包。

---

## 任务列表

### 阶段 1：规格与影响面

- [x] 1.1 校验工作区 frpc 隔离规格，确认端口冲突、进程归属和停止顺序均可验收。
  - **证据块**：`src-tauri/src/tunnel/supervisor.rs:59` 当前只有一个 `frpc: Option<FrpcProcess>`；`src-tauri/src/commands/runtime.rs:55` 只检查实际监听 PID；`src-tauri/src/commands/workspace.rs:65` 只校验 FRP 子域名。
  - **涉及文件**：3 个规格文档，合计约 350 行。
  - _需求：FR-1、FR-2、FR-3、FR-4、FR-5_ ｜ _设计：全部章节_
- [x] 1.2 对待修改符号执行 GitNexus upstream impact，记录风险后再进入 RED。
  - **证据块**：`TunnelSupervisor::start/stop_internal/restart_frpc`、`spawn_frpc`、`start_runtime/start_tunnel/update_workspace` 是主要变更入口。
  - **涉及文件**：只读分析，无源码修改。
  - _需求：FR-1、FR-3、FR-5_ ｜ _设计：架构设计_

### 阶段 2：RED——资源冲突

- [x] 2.1 先增加端口、proxy name 和 subdomain 冲突失败测试，并运行得到目标 RED。
  - **证据块**：`WorkspaceProfile` 已在 `runtime.local_port`、`actions.local_port` 记录端口；现有测试只覆盖 subdomain。
  - **涉及文件**：新增 `src-tauri/src/workspace/resources.rs`（约 220 行），修改 `workspace/mod.rs`（约 5 行）。
  - _需求：FR-3、FR-4_ ｜ _设计：资源校验、测试策略_

### 阶段 3：GREEN——资源冲突

- [x] 3.1 实现共享资源声明校验并接入保存、运行时启动和隧道启动入口。
  - **证据块**：`runServiceToggle` 会把 Tauri 错误正文作为启动失败 Toast 展示，因此后端错误必须包含冲突工作区、服务和值。
  - **涉及文件**：`workspace/resources.rs`、`commands/workspace.rs`、`commands/runtime.rs`、`commands/tunnel.rs`，合计新增约 180 行。
  - _需求：FR-3、FR-4、NFR-4_ ｜ _设计：API 设计、决策 2_

### 阶段 4：RED——每工作区 frpc

- [ ] 4.1 增加工作区配置路径、proxy name、PID 归属和局部进程状态失败测试，并运行得到目标 RED。
  - **证据块**：`managed_frpc_config_path` 当前返回全局 `frpc-active.toml`；`stop_running_frpc_instances` 当前按所有已知镜像路径结束进程。
  - **涉及文件**：`tunnel/frp/client.rs`、`tunnel/frp/mod.rs`、`tunnel/supervisor.rs` 测试模块，新增约 180 行测试。
  - _需求：FR-1、FR-2、FR-5_ ｜ _设计：进程隔离、进程恢复_

### 阶段 5：GREEN——每工作区 frpc

- [ ] 5.1 将单一 frpc 状态替换为工作区进程映射，所有启动、停止、删除和 orphan 清理只操作目标工作区。
  - **证据块**：`restart_frpc` 当前聚合 `self.frp_routes.values()`，会把全部工作区写入一个配置并全局重启。
  - **涉及文件**：`tunnel/supervisor.rs` 当前 931 行；新增逻辑优先提取小型纯函数，业务文件净增预算 180 行，不继续堆叠无关职责。
  - _需求：FR-1、FR-2、FR-5_ ｜ _设计：决策 1、架构设计_
- [ ] 5.2 改造 frpc 工作区配置、PID、锁和有界释放重试，移除正常流程的全局镜像路径批量终止。
  - **证据块**：`tunnel/frp/client.rs:20` 仅有 600ms grace；`stop_running_frpc_instances` 会终止所有同路径 `frpc`。
  - **涉及文件**：`tunnel/frp/client.rs` 当前 686 行，新增路径/PID职责控制在约 170 行；`tunnel/frp/mod.rs` 修改约 30 行。
  - _需求：FR-4、FR-5、NFR-2、NFR-3_ ｜ _设计：决策 3、决策 4_

### 阶段 6：集成验证与交付

- [ ] 6.1 对照 FR-1 至 FR-5 运行专项和全量 Rust 测试、Clippy、Svelte check/build。
  - **证据块**：项目现有 Rust 测试内嵌在对应模块，前端错误展示复用 `src/lib/runtime/service.ts`。
  - **涉及文件**：仅测试和构建产物。
  - _需求：FR-1、FR-2、FR-3、FR-4、FR-5_ ｜ _设计：测试策略_
- [ ] 6.2 运行 GitNexus detect-changes，升级 0.1.19 并构建 Windows MSI/NSIS，核验版本、大小和 SHA-256。
  - **证据块**：当前版本 0.1.18，上一轮 Windows Tauri 打包已通过。
  - **涉及文件**：版本清单与构建产物；版本文件实际修改数量在实施时核对。
  - _需求：NFR-3_ ｜ _设计：测试策略_

---

## 检查点

- [x] 阶段 1 完成后：`check_spec` 通过；GitNexus 对 Rust 符号返回 UNKNOWN，已按高风险人工调用链评估并告知用户。
- [x] 阶段 2 完成后：资源冲突测试因缺少实现而产生 E0432 目标 RED，并创建只含本任务测试的 checkpoint commit。
- [x] 阶段 3 完成后：资源冲突专项测试 8 项 GREEN，并创建最小实现 checkpoint commit。
- [ ] 阶段 4 完成后：工作区 frpc 隔离测试 RED，并创建第二个测试 checkpoint commit。
- [ ] 阶段 5 完成后：隔离专项测试 GREEN，A 工作区操作不改变 B 状态。
- [ ] 阶段 6 完成后：全量门禁通过，安装包哈希完成。

---

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---|---|---|---|
| FR-1 | 决策 1、架构设计 | 1.2、4.1、5.1、6.1 | 未开始 |
| FR-2 | 决策 1、架构设计 | 4.1、5.1、6.1 | 未开始 |
| FR-3 | 决策 2、资源校验 | 2.1、3.1、6.1 | 未开始 |
| FR-4 | 资源校验、proxy name | 2.1、3.1、5.2、6.1 | 未开始 |
| FR-5 | 决策 3、决策 4 | 1.2、4.1、5.1、5.2、6.1 | 未开始 |

---

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|---|---|---:|---|
| `src-tauri/src/workspace/resources.rs` | 新建 | 220 | 资源声明、冲突错误和单元测试 |
| `src-tauri/src/workspace/mod.rs` | 修改 | 5 | 导出资源校验 API |
| `src-tauri/src/commands/workspace.rs` | 修改 | 80 | 保存时共享校验，移除重复实现 |
| `src-tauri/src/commands/runtime.rs` | 修改 | 50 | MCP/Actions 启动前校验 |
| `src-tauri/src/commands/tunnel.rs` | 修改 | 60 | start/restart/test 启动前校验 |
| `src-tauri/src/tunnel/supervisor.rs` | 修改 | 260 | 工作区进程映射与局部生命周期 |
| `src-tauri/src/tunnel/frp/mod.rs` | 修改 | 50 | 稳定唯一代理名 |
| `src-tauri/src/tunnel/frp/client.rs` | 修改 | 260 | 工作区配置、PID、锁、停止与重试 |

## 检查清单

- [x] 交付物清单已填
- [x] 每条任务标题具体且可验收
- [x] 每条任务含证据块
- [x] 每条任务标注涉及文件与行数预算
- [x] 任务按 TDD 阶段拆分
- [x] 每条任务回链到 FR 与设计章节
- [x] 需求覆盖矩阵无遗漏
- [x] 阶段 6 对照验收标准核验
- [x] 全文无占位符
