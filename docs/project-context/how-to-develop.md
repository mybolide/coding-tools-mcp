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

## 安装包版本与发布规则（硬性）

凡是包含功能或缺陷修复、且需要构建、安装、交付或发布桌面安装包的变更，**必须先递增应用版本，再构建**。不得用旧版本号覆盖安装包，也不得只改 DMG、NSIS 等产物文件名。

### 版本递增

- 默认递增 patch 版本，例如 `0.1.7` → `0.1.8`。
- 新功能或不兼容变更按语义化版本递增 minor 或 major。
- 仅文档、索引或不产生安装包的维护变更可不递增版本。

### 必须同步的版本源

每次递增时，以下位置必须保持为同一版本：

1. `package.json`
2. `package-lock.json` 的根 `version` 和根包 `version`
3. `src-tauri/Cargo.toml`
4. `src-tauri/Cargo.lock` 中 `coding-tools-mcp-desktop` 包的 `version`
5. `src-tauri/tauri.conf.json`

不要修改依赖自身恰好相同的版本号；只更新本项目包的版本字段。

### 构建门禁

构建前必须：

1. 搜索上述版本源，确认没有旧的项目版本残留。
2. 运行 `npm run check`、`cargo check`；修复类变更还需运行相关 Rust 测试。
3. 提交版本升级与功能/修复代码，再从该提交构建。

构建后必须：

1. 校验 App 内部版本（macOS 为 `CFBundleShortVersionString`）。
2. 校验安装包文件名包含当前版本，例如 `Coding Tools MCP_<version>_aarch64.dmg`。
3. 校验已安装应用显示的版本与安装包一致；不得把旧包误报为新包。
4. 清理同一构建目录中旧版本的安装包和临时构建日志，但保留当前版本产物。

若移动了 macOS 源码目录，Rust/Tauri 的 `target` 缓存会保留旧绝对路径；首次在新目录构建前必须执行 `cd src-tauri && cargo clean`，再重新构建。

macOS GitHub Actions 仍仅允许在用户明确要求后通过 `workflow_dispatch` 手动触发，不因版本提交自动触发。

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
