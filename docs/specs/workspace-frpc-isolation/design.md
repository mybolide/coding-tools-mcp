# 设计文档：workspace-frpc-isolation

## 概述

以工作区 ID 作为 `frpc` 生命周期边界，把 `TunnelSupervisor` 的单一 `Option<FrpcProcess>` 改为工作区进程映射。资源冲突校验抽成共享纯函数，由工作区保存、运行时启动和隧道启动共同调用；后端错误继续通过现有 Svelte Toast 展示。

**对应需求：** FR-1、FR-2、FR-3、FR-4、FR-5、NFR-1、NFR-2、NFR-3、NFR-4。

---

## 技术方案

### 技术选型

| 类别 | 选择 | 理由 | 关联需求 |
|---|---|---|---|
| 进程隔离 | `HashMap<workspace_id, FrpcProcess>` | 直接表达工作区所有权，避免全局进程重启 | FR-1、FR-5 |
| 资源校验 | Rust 纯函数扫描 `WorkspaceProfile` | 所有入口复用，易于单元测试且无需新增存储 | FR-3、FR-4 |
| 进程恢复 | 工作区 PID 文件 + 镜像路径核验 | 应用重启后可回收自己的孤儿进程，避免误杀 | FR-5 |
| 用户提示 | 复用 Tauri 错误与现有 Toast | 后端硬门禁直接显示，无需复制校验逻辑到页面 | FR-3、NFR-4 |

### 架构设计

```text
点击启动 MCP / Actions / Tunnel
              ↓
Workspace resource validator
  ├─ local_port 冲突
  ├─ proxy name 冲突
  └─ HTTP subdomain 冲突
              ↓ 通过
RuntimeSupervisor 启动本地监听器
              ↓
TunnelSupervisor[workspace_id]
  ├─ routes: MCP / Actions
  ├─ frpc child + pid
  ├─ 独立 frpc.toml / frpc.pid
  └─ 当前工作区失败恢复与有界重试
```

停止顺序：先停止当前服务对应隧道 route，再关闭本地监听器；`TunnelSupervisor` 只重建当前工作区 `frpc`。另一个服务仍有 route 时保留该代理，无 route 时删除 PID 记录并停止进程。

---

## 数据模型

`WorkspaceProfile.runtime.local_port` 与 `WorkspaceProfile.actions.local_port` 已持久化，不新增业务数据字段。新增运行时文件：

| 实体/字段 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `frpc_processes` | `HashMap<String, FrpcProcess>` | key 为工作区 ID | 进程内 Child/PID 所有权 |
| `frpc/<workspace_id>/frpc.toml` | 文件 | 每工作区唯一 | 当前工作区 MCP/Actions 聚合配置 |
| `frpc/<workspace_id>/frpc.pid` | 文件 | PID + frpc 镜像路径 | 应用重启后的安全回收记录 |
| `WorkspaceResourceClaim` | 值对象 | 工作区、服务、资源类型和值 | 冲突错误构造和测试 |

工作区目录名仅接受 ASCII 字母、数字、`-`、`_`，其他字符转为 `_`，防止路径穿越。

---

## API 设计

| 方法/函数 | 签名/职责 | 入参 | 出参 | 关联需求 |
|---|---|---|---|---|
| `validate_workspace_resources` | 校验 candidate 保存后的全部资源声明 | profiles、candidate | `AppResult<()>` | FR-3、FR-4 |
| `validate_service_start` | 启动前校验目标工作区/服务 | profiles、workspace_id、service | `AppResult<()>` | FR-3、FR-4 |
| `restart_workspace_frpc` | 仅重建一个工作区进程 | workspace_id、settings | `AppResult<()>` | FR-1、FR-2、FR-5 |
| `spawn_frpc` | 使用工作区独立配置启动 | workspace_id、routes、settings | `FrpcHandle` | FR-1、FR-5 |
| `stop_recorded_frpc_instance` | 核验并停止 PID 文件指向的进程 | workspace_id、frpc_path | 停止数量 | FR-5 |

Tauri command 名称保持不变，避免改变前端调用契约。

---

## 文件结构

```text
docs/specs/workspace-frpc-isolation/
├── requirements.md
├── design.md
└── tasks.md
src-tauri/src/workspace/
├── mod.rs
└── resources.rs                 新增资源声明和冲突校验
src-tauri/src/commands/
├── runtime.rs                   启动前端口校验
├── tunnel.rs                    隧道启动前资源校验
└── workspace.rs                 保存时复用资源校验
src-tauri/src/tunnel/
├── supervisor.rs                工作区 frpc 进程映射与局部回滚
└── frp/
    ├── mod.rs                   稳定 proxy name
    └── client.rs                工作区配置/PID/锁与启动停止
```

---

## 设计决策

### 决策 1：每工作区一个 frpc（关联需求：FR-1、FR-2）

**问题：** 单一聚合进程使任一工作区线路变更都重启全部代理。

**选项：**

1. 保留单进程并增加 FRPS 释放重试。
2. 每条服务一个进程。
3. 每工作区一个进程，工作区内双服务共享。

**决策：** 选择选项 3。

**理由：** 工作区间完全隔离，同时避免 MCP/Actions 各占一个进程造成不必要的资源开销。

### 决策 2：配置声明重复即阻止启动（关联需求：FR-3）

**问题：** 两个停止状态的工作区可以暂时共享端口，但之后并行启动会产生歧义。

**决策：** 所有持久化端口声明必须唯一；旧数据存在重复时，启动目标服务即返回占用工作区与服务。

**理由：** 用户要求点击启动前明确告知，且唯一声明让状态恢复、自动启动和跨平台行为保持确定。

### 决策 3：按 PID 和镜像路径双重确认（关联需求：FR-5、NFR-2）

**问题：** 按 `frpc.exe` 路径批量结束会杀死其他工作区；只信 PID 文件又可能遇到 PID 复用。

**决策：** 工作区 PID 文件记录 PID 和镜像路径，停止前同时核验当前进程镜像；不匹配时删除陈旧记录并拒绝误杀。

### 决策 4：有界处理 FRPS 释放延迟（关联需求：FR-5）

**问题：** Windows 强制结束旧 `frpc` 后，FRPS 可能在约 30 秒内继续占用 proxy name。

**决策：** 仅对 `proxy already exists` 使用最长约 35 秒的退避重试，并始终持有当前工作区操作锁；其他认证或配置错误立即返回。

---

## 测试策略

- 单元测试：跨工作区 MCP/MCP、MCP/Actions 端口重复；自身更新；旧数据重复；冲突错误内容。
- FRP 配置测试：同名工作区生成不同稳定 proxy name；同工作区 MCP/Actions 名称不同。
- Supervisor 测试：A/B 启动状态互不影响；停止 A 不触及 B；A 保留 Actions 时配置只含 A Actions。
- PID 测试：工作区路径隔离、陈旧 PID、镜像不匹配不终止。
- 回归门禁：Rust 全量测试、Clippy `-D warnings`、Svelte check/build、Windows 安装包。

---

## 风险评估

| 风险 | 影响 | 缓解措施 |
|---|---|---|
| 旧版本遗留的单一 frpc 没有工作区 PID 文件 | 高 | 首次迁移仅识别旧全局 PID/配置并安全清理一次，之后禁止全局路径批杀 |
| FRPS 释放代理超过重试窗口 | 中 | 返回明确错误，保留当前工作区旧 route 状态，不影响其他工作区 |
| 旧配置存在重复端口 | 中 | 保存不相关字段不强制迁移；启动时指出具体冲突双方 |
| 两个桌面实例并发管理同一工作区 | 高 | 操作锁改为工作区范围并覆盖停止、等待、启动全过程 |
| 多进程增加资源占用 | 低 | 工作区内 MCP/Actions 共享，停止无 route 的实例 |

## 检查清单

- [x] 技术方案与现有架构一致
- [x] 全部 FR 均被覆盖
- [x] 文件结构使用真实路径
- [x] 数据模型与接口契约清晰
- [x] 关键设计决策已记录并关联需求
- [x] 测试策略可验证验收标准
