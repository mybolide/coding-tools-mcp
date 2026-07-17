# 设计文档：history-session-archive

## 概述

本功能作为 `tools/history/` 独立领域模块加入现有 MCP 工具内核。MCP 层只负责把 ChatGPT `_meta["openai/session"]` 作为内部参数传入历史工具；工具注册表负责暴露三个新工具；历史模块负责解析、校验、加锁、原子写入、脱敏和摘要。既有工具分支、Schema 和结果保持不变。

**对应需求:** FR-1, FR-2, FR-3, FR-4, FR-5, FR-6, FR-7, FR-8, NFR-1, NFR-2, NFR-3, NFR-4, NFR-5

---

## 技术方案

### 技术选型

| 类别 | 选择 | 理由 | 关联需求 |
|------|------|------|----------|
| 服务协议 | 现有 MCP 2025-06-18 Streamable HTTP | ChatGPT 网页版已通过现有 `/mcp` 连接，无需 OpenAI SDK | FR-1, FR-7 |
| 序列化 | `serde`、`serde_json` | 与现有工具结果和 `index.json` 一致 | FR-1, FR-4 |
| 内容哈希 | 现有 `sha2` | 返回审计哈希且无需新增依赖 | FR-2, FR-3 |
| 跨进程锁 | `fs2::FileExt` | Windows、macOS、Linux 提供统一文件锁 | FR-5, NFR-3 |
| 原子替换 | Unix `rename`；Windows `MoveFileExW` | 保证同目录替换并覆盖既有索引/Markdown | FR-5, NFR-3 |
| 摘要 | 固定 Markdown 章节解析 | 无外部模型、结果确定、可测试 | FR-2, FR-7 |

### 架构设计

```text
ChatGPT tools/call
  -> mcp/server.rs 提取 params._meta["openai/session"]
  -> 仅为 history_session_* 注入内部 _host_session_key
  -> tools/dispatch.rs 唯一分发入口
  -> tools/history/mod.rs 用例编排
       -> storage.rs 路径、锁、扫描、索引、原子写入
       -> markdown.rs 解析、渲染、检查点更新、脱敏
       -> model.rs 输入、索引和扫描模型
  -> wrap_mcp_tool_result 保持现有结果 envelope
```

历史工具不读取客户端传入的任意 `workspace_root` 作为权限根。当前 MCP Runtime 的 `Workspace` 是唯一可信边界；兼容字段 `workspace_root` 存在时必须与已绑定工作区规范路径相同。

---

## 数据模型

### index.json

```json
{
  "version": 1,
  "latest_number": 4,
  "sessions": {
    "anonymous-chat-session": {
      "number": 4,
      "path": "docs/history-session/4.md",
      "created_at": "2026-07-17T11:00:00+08:00",
      "updated_at": "2026-07-17T11:05:00+08:00"
    }
  }
}
```

`index.json` 是加速和幂等映射，不是唯一事实源。Markdown 顶部的 `Session key`、编号和时间字段用于重建索引。

### 会话 Markdown

```markdown
# 会话 N：标题

**Session key:** anonymous-chat-session
**Created:** 2026-07-17T11:00:00+08:00
**Updated:** 2026-07-17T11:05:00+08:00
**Status:** active

## 用户核心目标
## 已确认事实
## 已完成修改
## 关键设计决定
## 测试结果
## 当前运行状态
## 剩余问题
## 下一步
## 本轮检查点
### turn-0001
```

### 错误模型

历史模块使用现有 `WorkspaceError::ToolDetails` 返回 `code`、`message`、`category`、`retryable` 和 `details`。主要代码包括 `SESSION_ID_UNAVAILABLE`、`SESSION_NOT_BOOTSTRAPPED`、`HISTORY_SEQUENCE_CONFLICT`、`HISTORY_LOCK_FAILED`、`HISTORY_INDEX_CONFLICT`、`PATH_OUTSIDE_WORKSPACE` 和 `HISTORY_WRITE_FAILED`。

---

## API 设计

| 方法/函数 | 路径/签名 | 入参/出参 | 关联需求 |
|------|------|------|----------|
| MCP tool | `history_session_bootstrap` | 可选 `session_key`、`title`、`workspace_root`、`history_dir`、`create_if_missing`；返回编号、摘要、handoff、哈希和告警 | FR-1, FR-2 |
| MCP tool | `history_session_checkpoint` | 可选 `session_key`、必填 `turn_id` 及结构化交接字段；返回路径、幂等状态和内容哈希 | FR-3, FR-6 |
| MCP tool | `history_session_validate` | 可选 `workspace_root`、`history_dir`、`repair`；返回完整性报告 | FR-4, FR-5 |
| 内部函数 | `history::bootstrap(ctx, args)` | 解析会话身份并在锁内恢复或创建会话 | FR-1, FR-2 |
| 内部函数 | `history::checkpoint(ctx, args)` | 在锁内更新指定 turn 块和顶部累积章节 | FR-3, FR-6 |
| 内部函数 | `history::validate(ctx, args)` | 只读扫描或安全重建索引 | FR-4, FR-5 |

会话身份优先级固定为 `_host_session_key`、显式 `session_key`、结构化错误。`_host_session_key` 不出现在公开 Schema 中。

---

## 文件结构

```text
src-tauri/
  Cargo.toml                              # 增加 fs2 和 Windows 文件系统 feature
  src/mcp/server.rs                       # 为历史工具传递 openai/session
  src/tools/mod.rs                        # 导出 history 模块
  src/tools/registry.rs                   # 注册工具、Schema 和 annotations
  src/tools/dispatch.rs                   # 新增三个分发分支
  src/tools/history/mod.rs                # 用例编排与结果组装
  src/tools/history/model.rs              # 索引及扫描数据模型
  src/tools/history/storage.rs            # 路径、锁、扫描、索引和原子替换
  src/tools/history/markdown.rs           # Markdown 解析、渲染、更新和脱敏
```

测试与实现放在相应模块的 `#[cfg(test)]` 中，MCP 元数据集成测试放在 `mcp/server.rs`。

---

## 设计决策

### 决策 1: 使用 ChatGPT 宿主会话元数据（关联需求: FR-1）

**问题**: 如何稳定识别同一网页聊天而不依赖标题或时间。
**选项**: 模型传参、浏览器 DOM、`_meta["openai/session"]`。
**决策**: MCP 层读取 `_meta["openai/session"]` 并以内部参数传递；显式参数只用于测试和兼容客户端。[OpenAI Apps SDK 官方变更记录](https://developers.openai.com/apps-sdk/changelog)已明确：自 2026-01-15 起，ChatGPT 工具调用包含该匿名 conversation id，可用于关联同一 ChatGPT 会话内的请求。

### 决策 2: 结构化 Markdown 代替外部 LLM 摘要（关联需求: FR-2, FR-7）

**问题**: 如何在不引入 OpenAI SDK 的前提下生成稳定摘要。
**选项**: 外部 LLM、全文注入、固定章节提取。
**决策**: checkpoint 本身保存结构化交接；bootstrap 从固定章节确定性提取摘要，并完整返回最新 handoff。

### 决策 3: 文件是事实源，索引可重建（关联需求: FR-4）

**问题**: 索引损坏时如何恢复且不删除历史。
**选项**: 仅信索引、仅扫描、索引加 Markdown 元数据。
**决策**: 每次 bootstrap 在锁内扫描数字文件并交叉校验索引；repair 仅重建索引。

### 决策 4: 保持现有工具行为不变（关联需求: FR-7）

**问题**: 是否用 bootstrap 门禁包裹所有现有工具。
**选项**: 修改统一分发门禁、仅新增历史工具。
**决策**: 仅增加三个分发分支，不拦截或改变任何现有工具。ChatGPT 通过明确的工具描述在用户说“恢复会话”时调用 bootstrap。

### 决策 5: 用初始化 instructions 和工具结果双层提示持久化（关联需求: FR-8）

**问题**: 仅暴露 checkpoint 工具时，ChatGPT 是否会在每轮结束前稳定主动调用。
**选项**: 依赖用户每轮提醒、修改旧工具增加强制门禁、MCP 初始化 instructions 加 bootstrap 结果指令。
**决策**: 使用后者。OpenAI 官方文档说明 ChatGPT 会把 MCP 初始化 `instructions` 用作跨工具工作流的服务器级指导，并会检查工具名称、描述和 Schema 来决定调用时机。bootstrap 成功结果再返回同一规则，形成连接级和会话级双层提示；不修改旧工具，不声称存在后台自动执行。

工作区详情页在现有 MCP「GPT 配置」卡片底部增加提示词区块，而不是增加独立嵌套卡片。区块展示完整提示词、简短使用说明和一键复制按钮；复制过程包含禁用、成功、失败及 `aria-live` 反馈。模板内容跨工作区相同，但入口按工作区展示，便于用户在配置当前工作区连接器时直接使用。

---

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| Windows 覆盖式 rename 与 Unix 不同 | 高 | 使用 `MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)` |
| 多个 ChatGPT 会话并发分配相同编号 | 高 | 目录级跨进程独占锁覆盖扫描、分配和写索引全过程 |
| 模型输入包含密钥 | 高 | 所有自由文本和字符串数组在渲染前统一脱敏，并测试不回显原值 |
| 历史文件被手工修改导致索引漂移 | 中 | bootstrap 全量扫描并报告冲突；validate repair 重建索引 |
| 历史增长造成上下文过大 | 中 | 返回全部会话摘要加最新全文，不默认返回全部原文 |
| 工具名注册与分发不一致 | 中 | Schema、tools/list、分发和 `_meta` 注入集成测试 |
