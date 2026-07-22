# 需求文档：gpt-client-configuration-docs

## 功能概述

为首次使用 Coding Tools MCP 的 ChatGPT 用户补充可按图完成的 GPT 端配置教程，将桌面端启动、公网 MCP 地址、ChatGPT 开发人员模式、插件创建、OAuth 授权和连接验证串成完整流程。

## 历史经验与坑

- **可复用经验**：沿用双语 README 的真实截图模式，把教程放在五分钟快速开始之后的客户端接入章节。
- **必须规避的坑**：截图和文字不得展示 Token、Client Secret、授权口令或本机敏感路径；不得把 MCP Connector 与 GPT Actions 的配置字段混为一谈。

## 术语定义

- **开发人员模式**：ChatGPT 账户设置中允许添加未经验证 MCP 连接器的开关。
- **MCP 插件**：ChatGPT 插件页面中使用公网 `/mcp` URL 创建的 MCP 连接。

## 范围边界

**In Scope**

- 在中英文 README 中加入两张 GPT 配置截图和对应说明。
- 说明开启开发人员模式、创建 MCP 插件、填写字段、完成授权和验证连接的步骤。
- 明确配置所需信息来自桌面端工作区的 MCP 连接区域。
- 在 README 首屏提供 30 秒连接路径，并加入常见连接问题的快速排查入口。

**Out of Scope**

- 修改桌面应用、MCP 协议或认证实现。
- 编写 ChatGPT GPT Actions 的逐屏截图教程。
- 承诺 ChatGPT 界面文案永久不变。

## 需求列表

### FR-1：说明 ChatGPT MCP 配置前置条件

**优先级：** Must
**用户故事：** 作为首次使用者，我想知道配置 ChatGPT 前需要启动哪些服务并复制哪些信息，以便避免在不可访问的地址上排查问题。

#### 验收标准

1. WHEN 用户阅读 MCP Connector 教程 THEN README SHALL 要求 MCP 服务和公网隧道处于运行状态。
2. WHEN 用户准备配置连接 THEN README SHALL 指明从桌面端复制公网 `/mcp` 地址及所选认证信息。
3. IF 用户只有 `127.0.0.1` 本地地址 THEN README SHALL 说明 ChatGPT 无法直接访问该地址。

### FR-2：提供按图完成的 ChatGPT 配置和验证步骤

**优先级：** Must
**用户故事：** 作为 ChatGPT 用户，我想按截图完成开发人员模式和 MCP 插件配置，以便确认 AI 能调用当前工作区的工具。

#### 验收标准

1. WHEN 用户阅读教程 THEN README SHALL 按顺序展示开发人员模式和新建插件截图。
2. WHEN 用户填写插件表单 THEN README SHALL 解释名称、描述、连接 URL 和身份验证字段。
3. WHEN 插件保存并授权完成 THEN README SHALL 给出可执行的连接验证方法。
4. IF 连接或工具发现失败 THEN README SHALL 给出公网 URL、OAuth、工具缓存和日志的优先排查项。

## 非功能需求

- **NFR-1（可读性）**：教程应在一个 README 小节内完成，步骤不依赖源码知识。
- **NFR-2（安全）**：文档不得要求公开 OAuth Secret、授权口令或 Token。
- **NFR-3（兼容性）**：使用“名称可能随 ChatGPT 版本变化”的提示降低界面更新造成的歧义。

## 依赖关系

- `README.md` 与 `README.en.md` 现有快速开始和 ChatGPT 接入章节。
- `docs/images/gpt-config-1.png` 与 `docs/images/gpt-config-2.png`。

## 检查清单

- [x] 已消化 README 截图展示经验并规避敏感信息风险
- [x] 需求覆盖前置条件、配置和验证场景
- [x] 每条需求有唯一 ID
- [x] 验收标准可人工核验
- [x] 已标注优先级
- [x] 范围边界明确
- [x] 非功能需求明确
- [x] 依赖关系完整
