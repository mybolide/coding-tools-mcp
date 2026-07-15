# 设计文档：grep 与手动 macOS 发布

## 概述

本设计覆盖 FR-1 至 FR-4。`grep` 作为注册表级兼容别名接入 `search_text`，不新增搜索引擎；macOS 补齐按镜像路径清理 frpc 的平台实现，并通过只含 `workflow_dispatch` 的 GitHub Actions 工作流构建通用 DMG。

## 技术方案

| 类别 | 选择 | 理由 | 关联需求 |
|------|------|------|----------|
| 搜索实现 | `grep` 路由到 `file::search_text` | 单一实现、行为一致、跨平台 | FR-1, FR-2 |
| 工具 schema | `grep` 与 `search_text` 共用 schema | 避免契约漂移 | FR-1, FR-2 |
| macOS 进程管理 | libproc 枚举 PID，按规范化镜像路径终止进程树 | 与唯一 frpc 生命周期一致 | FR-3 |
| macOS 构建 | `universal-apple-darwin` + DMG | 一个包覆盖 Intel 和 Apple Silicon | FR-3 |
| 触发方式 | GitHub `workflow_dispatch` | 禁止自动消耗与自动发布 | FR-4 |

### 架构设计

```text
MCP tools/list / server_info / Actions OpenAPI
                    ↓
             tools::registry
              ↓            ↓
         search_text       grep
              └──────┬──────┘
                     ↓
             file::search_text

workflow_dispatch
        ↓
macos-14 + Rust 双 target + Node
        ↓
Tauri universal app/dmg
        ↓
Actions artifact ──可选──→ 已存在的 GitHub Release
```

## 数据模型

不涉及持久化数据变更。

## API 设计

| 工具 | 入参 | 出参 | 关联需求 |
|------|------|------|----------|
| `grep` | 与 `search_text` 相同：`query` 必填；`path`、`glob`、`include_globs`、`exclude_globs`、`regex`、`case_sensitive`、`context_lines`、`max_preview_bytes`、`max_results` 可选 | `query`、`matches[{path,line,column,preview,before,after}]`、`total_matches`、`truncated`、`warnings` | FR-1, FR-2 |

## 文件结构

```text
.github/workflows/macos-release.yml
docs/specs/grep-and-manual-macos-release/
src-tauri/src/tools/registry.rs
src-tauri/src/tools/dispatch.rs
src-tauri/src/platform/macos/mod.rs
src-tauri/src/platform/macos/process.rs
src-tauri/tests/call_tool_contract.rs
```

## 设计决策

### 决策 1：grep 采用别名而非独立实现

独立实现会重复目录遍历、忽略规则和错误处理。注册别名保留 Agent 熟悉的工具名，同时保证 `search_text` 兼容性与单一事实源。

### 决策 2：工作流只允许 workflow_dispatch

不使用 push、pull_request、tag、release 或 schedule。用户明确要求后，维护者再通过 `gh workflow run` 触发。

### 决策 3：先交付未签名通用包，签名能力保持可扩展

代码兼容和通用构建可以立即验证；Apple 签名与公证需要用户的开发者账号和私密凭据，不应在仓库中伪造。未签名包用于当前测试，正式广泛分发前再配置 GitHub Secrets。

## 测试策略

- 契约测试验证 core 工具清单、grep schema、调用结果及默认 cwd。
- Rust 全量测试和 Clippy 验证别名未破坏现有工具。
- macOS 工作流运行 `cargo test`、`npm run check` 和 Tauri universal DMG 构建。
- 工作流静态检查确认 `on` 下只有 `workflow_dispatch`。
- Windows 继续构建 NSIS 安装包。

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 工具数增加造成选择歧义 | 低 | 描述中明确 grep 是 search_text 兼容入口，schema 完全一致 |
| macOS libproc 链接或双架构编译失败 | 中 | 在 GitHub macOS runner 真实编译，不以 Windows 交叉检查代替 |
| 未签名 DMG 被 Gatekeeper 阻止 | 中 | Release 标注未签名测试包，后续配置 Apple 签名与公证 |
| 工作流误自动触发 | 高 | YAML 只声明 workflow_dispatch，并增加静态测试/人工复核 |
