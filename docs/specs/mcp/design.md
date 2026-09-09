# 设计文档：本地 MCP 配置入口与自动挂载

## 概述

本设计覆盖 FR-1、FR-2 和全部非功能需求。前端只保留通用 stdio 草稿；运行时不作改动，仅以现有启动顺序作为自动挂载的验证对象。

## 技术方案

删除仅服务于机器特定预填的 UI 函数和按钮。保留现有的通用新增、保存、发现与逐项工具开关逻辑；不接触已通过运行时测试的上游初始化、监听器和隧道代码。

## 架构设计

```text
UpstreamMcpForm
  └─ 新增 stdio MCP（空白草稿）
       └─ 保存到 WorkspaceProfile.runtime.upstream_mcps

start_mcp_service
  └─ UpstreamMcpManager::start(已启用配置)
       └─ initialize → notifications/initialized → tools/list
            └─ start_prepared_mcp
                 └─ MCP listener + 公网隧道共用同一工具目录
```

## 文件结构

```text
src/lib/components/UpstreamMcpForm.svelte  # 删除机器特定预设和按钮
README.md                                  # 改为通用 stdio 配置说明
docs/specs/mcp/                            # 本功能规格
```

## 设计决策

### 决策 1：删除预设而不改动配置模型

**关联需求:** FR-1

删除机器特定预设和唯一的调用按钮。通用 `blankConfig` 已提供同等新增能力；配置模型、持久化格式和发现逻辑不动，从而不影响已保存的上游配置。

### 决策 2：将自动挂载定义为 MCP 服务启动责任

**关联需求:** FR-2

`start_mcp_service` 已在创建监听器前调用 `UpstreamMcpManager::start`，成功后才进入 `start_prepared_mcp` 和隧道启动。公网地址只是同一监听器的可达地址，不启动另一套工具目录。

## 测试策略

- `npm run check` 验证 Svelte 类型、模板语法及未引用符号。
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` 回归上游初始化与运行时测试。
- 文本检查确认 UI 不再包含机器特定示例按钮或预设；保留通用新增入口。

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 误删通用新增能力 | 中 | 只删除预设函数及对应按钮，保留 `blankConfig`。 |
| 修改启动链路导致服务不可用 | 高 | 本次不改运行时；以现有调用顺序和回归测试验证。 |
