# 任务清单：本地 MCP Streamable HTTP 与 JSON 导入

## 交付物清单

- **预计新建文件数:** 3 个规格文档。
- **预计修改文件数:** 5 个生产文件。
- **预计新增或修改函数数:** 约 16 个。
- **交付物:** Rust 配置模型、上游客户机、Svelte 类型与表单、README、三份规格文档。

## 任务列表

### 阶段 1：配置模型与契约

- [ ] 1.1 扩展上游配置并校验两种传输的持久化边界。
  - **证据块:** `model.rs` 目前将非 `stdio` 配置拒绝；`type` 已是带默认值的持久化字段。
  - **涉及文件:** `src-tauri/src/workspace/model.rs`，预计 100 行；`src/lib/types.ts`，预计 15 行。
  - _需求: FR-1、FR-3_ ｜ _设计: 数据模型、决策 1_

### 阶段 2：运行时转发

- [ ] 2.1 抽象上游客户机并实现 Streamable HTTP 的初始化、session、目录和工具调用。
  - **证据块:** `upstream.rs` 当前 `UpstreamMcp::start` 只创建 `tokio::process::Command`，并统一执行 `initialize`、`tools/list` 和 `tools/call`。
  - **涉及文件:** `src-tauri/src/mcp/upstream.rs`，预计 300 行；超过 500 行时按 `stdio` 和 `streamable_http` 私有结构拆分。
  - _需求: FR-1、FR-2_ ｜ _设计: 架构设计、决策 1、决策 2_

- [ ] 2.2 添加模型与本地 HTTP fixture 回归测试，验证默认公开、关闭项、session 与工具转发。
  - **证据块:** `upstream.rs` 已有 Node stdio fixture，`model.rs` 已有上游配置单元测试。
  - **涉及文件:** `src-tauri/src/mcp/upstream.rs`、`src-tauri/src/workspace/model.rs`，预计 180 行。
  - _需求: FR-1、FR-2、NFR-1、NFR-3_ ｜ _设计: 测试策略_

### 阶段 3：配置体验与文档

- [ ] 3.1 将添加入口改为菜单，实现快速创建、JSON 粘贴导入及输入错误回显。
  - **证据块:** `UpstreamMcpForm.svelte` 当前只有直接新增 stdio 按钮，且全部配置均显示 command、args、env。
  - **涉及文件:** `src/lib/components/UpstreamMcpForm.svelte`，预计 240 行。
  - _需求: FR-1、FR-3_ ｜ _设计: 决策 3_

- [ ] 3.2 更新本地 MCP 文档，说明两种传输、JSON 结构和请求头安全边界。
  - **证据块:** README 当前只说明 stdio 配置流程。
  - **涉及文件:** `README.md`，预计 70 行。
  - _需求: FR-1、FR-3、NFR-2_ ｜ _设计: 风险评估_

### 阶段 4：集成验证

- [ ] 4.1 运行模型与 HTTP fixture、全库测试、Svelte 检查、生产打包和代码审查。
  - **证据块:** 仓库已有 Rust lib 测试和 `npm run check`、`npm run desktop:build` 脚本。
  - **涉及文件:** 上述全部产物。
  - _需求: FR-1、FR-2、FR-3_ ｜ _设计: 测试策略_

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---------|----------|----------|------|
| FR-1 | 数据模型、决策 1、决策 2 | 1.1、2.1、2.2、3.1、4.1 | 未开始 |
| FR-2 | 架构设计、决策 2 | 2.1、2.2、4.1 | 未开始 |
| FR-3 | 决策 3 | 1.1、3.1、3.2、4.1 | 未开始 |

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|------|------|----------|------|
| `src-tauri/src/workspace/model.rs` | 修改 | 100 | HTTP 配置与验证。 |
| `src-tauri/src/mcp/upstream.rs` | 修改 | 480 | 双传输客户端与 fixture 测试。 |
| `src/lib/types.ts` | 修改 | 15 | 前端类型。 |
| `src/lib/components/UpstreamMcpForm.svelte` | 修改 | 240 | 传输字段与 JSON 导入。 |
| `README.md` | 修改 | 70 | 配置说明。 |
| `docs/specs/mcp-streamable/*.md` | 新建 | 230 | 规格。 |
