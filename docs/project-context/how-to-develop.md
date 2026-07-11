# 如何开发

> 本文档描述 Coding Tools MCP Rust 的开发流程。

## 概述

本项目使用 MCP Probe Kit 工作流驱动开发。新功能必须先走规格流程，通过闸门后再写实现代码。

## 新功能开发流程

### 第一步：启动功能编排

调用 `start_feature` MCP 工具：

```json
{
  "feature_name": "my-feature",
  "description": "功能描述",
  "project_root": "e:/workspace/github/coding-tools-mcp-rust"
}
```

### 第二步：生成并校验规格

1. 调用 `add_feature` 生成规格模板
2. Agent 按模板填写 `docs/specs/<feature>/requirements.md`、`design.md`、`tasks.md`
3. 调用 `check_spec` 校验规格完整性
4. **未通过前不要写实现代码**

### 第三步：工作量估算

调用 `estimate` 获取故事点和时间区间。

### 第四步：按 tasks.md 实现

1. 每条任务先写证据块（读相关代码）
2. 实现后对照验收标准核验
3. 单文件不超过 500 行，超出需拆分

## Tauri 开发命令（工程创建后）

```bash
# 开发模式（热重载）
cargo tauri dev

# 构建发布版
cargo tauri build

# 仅构建 Rust 后端
cd src-tauri && cargo build

# 仅构建前端
pnpm dev
```

## Rust 后端开发约定

### Tauri Command 模式

```rust
#[tauri::command]
async fn list_workspaces(state: State<'_, AppState>) -> Result<Vec<WorkspaceProfile>, String> {
    state.workspace_store.list().map_err(|e| e.to_string())
}
```

### 状态机模式

Runtime 生命周期使用显式 enum，不用字符串状态：

```rust
enum RuntimeState {
    Stopped,
    Starting { since: Instant },
    Running { pid: u32, port: u16 },
    Stopping,
    Error { message: String },
}
```

## 参考旧版实现

开发 MCP 工具时，以 `old/docs/profile-v0.1.md` 为行为契约，以 `old/tests/compliance/` 为验收标准。不要猜测工具行为，对照规范和测试。

---
*返回索引: [../project-context.md](../project-context.md)*
