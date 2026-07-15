# 任务清单：frp-subdomain-lifecycle

## 概述

实现多工作区 FRP 子域名生命周期管理，确保子域名修改和工作区删除只影响明确归属的线路。

## 交付物清单（Scope-lock）

- **预计新建文件数**：0 个业务源码文件，新增 3 个规格文档。
- **预计修改文件数**：2 个业务源码文件，必要时 1 个测试文件。
- **预计新增/修改函数数**：约 4 个函数及相关测试。
- **交付物逐项列举**：
  1. `src-tauri/src/tunnel/supervisor.rs` 的 route 唯一性与生命周期逻辑。
  2. `src-tauri/src/runtime/supervisor.rs` 的状态刷新保护回归。
  3. Rust 多工作区/子域名回归测试。
  4. Windows Release 安装包。

## 任务列表

### 阶段 1：准备工作

- [x] 1.1 核对工作区保存、删除和 TunnelSupervisor route 调用链。
  - **证据块**：`src-tauri/src/tunnel/supervisor.rs` 使用 `(workspace_id, TunnelServiceKind)` 作为 `frp_routes` key；`src-tauri/src/runtime/supervisor.rs` 的 `refresh` 曾触发 orphan cleanup。
  - **涉及文件**：`src-tauri/src/tunnel/supervisor.rs`（约 450 行）、`src-tauri/src/runtime/supervisor.rs`（约 580 行）。
  - _需求：FR-1、FR-2、FR-3_ ｜ _设计：技术方案_

### 阶段 2：核心实现

- [x] 2.1 实现子域名冲突校验，禁止新增或修改为其他活动工作区已使用的 proxy name/subdomain。
  - **证据块**：现有 `validate_frp_route_compatibility` 校验连接参数，但 route 集合需补充代理身份冲突校验。
  - **涉及文件**：`src-tauri/src/tunnel/supervisor.rs`（约 450 行）。
  - _需求：FR-1、FR-3、NFR-1_ ｜ _设计：决策 1_
- [x] 2.2 复用暂存/恢复机制处理子域名修改和工作区删除，确保只删除当前 workspace/service route。
  - **证据块**：现有 `start` 已暂存 `previous_route`，`stop_internal` 按 `(workspace_id, kind)` 删除 route。
  - **涉及文件**：`src-tauri/src/tunnel/supervisor.rs`、`src-tauri/src/runtime/supervisor.rs`（合计约 1030 行；不拆分，保持最小改动）。
  - _需求：FR-1、FR-2、NFR-2_ ｜ _设计：架构设计_

### 阶段 3：集成测试与打包

- [x] 3.1 增加并运行多工作区子域名生命周期回归测试，确认 A/B 同时在线、A 修改、B 删除和状态刷新均不误删。
  - **证据块**：现有 `src-tauri/src/tunnel/frp/mod.rs` 已有多 proxy 配置测试；需补生命周期/冲突断言。
  - **涉及文件**：`src-tauri/src/tunnel/supervisor.rs` 或 `src-tauri/tests/`。
  - _需求：FR-1、FR-2、FR-3_ ｜ _设计：测试策略_
- [x] 3.2 运行 Rust/前端门禁并构建 Windows Release 安装包，记录版本和产物路径。
  - **证据块**：当前项目使用 Tauri 2，版本配置位于 `src-tauri/tauri.conf.json`。
  - **涉及文件**：构建产物目录，不修改业务文件。
  - _需求：NFR-3_ ｜ _设计：测试策略_

## 检查点

- [x] 阶段 1 完成后：确认 route key、删除入口和现有恢复机制。
- [x] 阶段 2 完成后：冲突配置被拒绝，合法多工作区 route 保持聚合。
- [x] 阶段 3 完成后：测试通过，安装包生成且版本号可见。

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---|---|---|---|
| FR-1 | 决策 1、决策 2 | 2.1、2.2、3.1 | 已完成 |
| FR-2 | 架构设计 | 2.2、3.1 | 已完成 |
| FR-3 | 技术方案、测试策略 | 2.1、3.1 | 已完成 |

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|---|---|---:|---|
| `src-tauri/src/tunnel/supervisor.rs` | 修改 | 60 | 子域名冲突和 route 生命周期 |
| `src-tauri/src/runtime/supervisor.rs` | 修改 | 20 | 保持状态刷新安全 |
| `src-tauri/tests/` | 修改 | 100 | 生命周期回归测试 |
| `src-tauri/tauri.conf.json` | 读取 | 0 | 获取版本并打包 |
