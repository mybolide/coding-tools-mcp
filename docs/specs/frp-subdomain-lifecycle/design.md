# 设计文档：frp-subdomain-lifecycle

## 概述

通过 `TunnelSupervisor` 内存中的 route 归属记录实现子域名线路生命周期管理。配置修改前先取得旧工作区配置，确认新配置的子域名唯一，再原子替换当前工作区 route；工作区删除走显式 `drop_workspace`，普通状态刷新不触发线路删除。

**对应需求：** FR-1、FR-2、FR-3、NFR-1、NFR-2、NFR-3。

## 技术方案

### 架构设计

```text
工作区保存/删除
        ↓
归属校验（workspace_id + service + old config）
        ↓
更新 TunnelSupervisor.frp_routes
        ↓
停止应用控制的唯一 frpc
        ↓
聚合剩余 route 生成配置并启动 frpc
```

- route key 使用 `(workspace_id, TunnelServiceKind)`，不使用 subdomain 作为删除主键。
- 新 route 与已有 route 的 proxy name/subdomain 冲突时拒绝操作。
- 删除旧 route 只允许命中当前 workspace/service 的旧配置；未命中时不做服务端删除操作。
- `RuntimeSupervisor::refresh` 只负责本地运行状态确认，不负责普通状态下的 route 回收。

## 数据模型

不新增持久化数据。复用 `FrpRoute { profile, kind }`，以旧 `WorkspaceProfile` 保存配置快照，确保修改前的子域名可用于精确归属校验。

## API 设计

不新增对外 MCP API。复用以下内部接口：

| 方法/函数 | 作用 |
|---|---|
| `TunnelSupervisor::start` | 新增或替换当前工作区线路并重建聚合 frpc |
| `TunnelSupervisor::stop` | 显式停止当前工作区线路 |
| `TunnelSupervisor::drop_workspace` | 删除工作区时回收其 MCP/Actions 线路 |
| `RuntimeSupervisor::refresh` | 仅在确认本地 runtime 连续失效后触发当前线路清理 |

## 文件结构

```text
src-tauri/src/tunnel/supervisor.rs       修改 route 生命周期和唯一性校验
src-tauri/src/runtime/supervisor.rs      保持状态刷新不误删线路
src-tauri/src/commands/workspace.rs      核对工作区删除前后的显式清理流程
src-tauri/tests/                         增加多工作区和子域名冲突回归测试
```

## 设计决策

### 决策 1：以工作区和服务归属删除，而不是按 subdomain 删除（FR-1、FR-2、NFR-1）

**决策：** 删除键使用 `(workspace_id, service)`，subdomain 只用于冲突校验和生成配置。

**理由：** subdomain 可能来自旧配置、服务端残留或其他工作区；仅按名字删除有误删风险。

### 决策 2：配置修改采用暂存旧 route、验证新 route、失败恢复（FR-1、NFR-2）

**决策：** 复用现有 `start` 的 previous route/session 恢复机制。

**理由：** 新配置无效或 frpc 重启失败时，保留旧线路比先删除再尝试恢复更安全。

## 测试策略

- 单元测试：相同 subdomain 冲突、不同 workspace 同名拒绝、不同 service 的合法组合。
- route 生命周期测试：A 启动后 B 启动，配置包含 A/B；A 修改后只替换 A；删除 B 后只保留 A。
- 回归测试：状态刷新、端口单次抖动和运行时异常不会删除其他 workspace route。
- 发布前：Rust 测试、Clippy、前端检查、Windows 打包。

## 风险评估

| 风险 | 影响 | 缓解措施 |
|---|---|---|
| 服务端已有同名代理 | 高 | 保存/启动前拒绝冲突，不做模糊删除 |
| frpc 重启失败 | 高 | 保留旧 route，失败后尝试恢复旧聚合配置 |
| 配置修改与状态刷新并发 | 高 | route 变更只走 TunnelSupervisor 锁，refresh 不做普通清理 |
| 工作区删除流程遗漏线路 | 中 | 删除命令统一调用 `drop_workspace`，补 MCP/Actions 双服务测试 |
