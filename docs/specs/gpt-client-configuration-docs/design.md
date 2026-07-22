# 设计文档：gpt-client-configuration-docs

## 概述

在 `README.md` 与 `README.en.md` 首屏加入 30 秒连接路径，并在 MCP Connector 小节提供从桌面端准备到 ChatGPT 验证和故障排查的逐步教程。第二张真实截图使用不改变内容的裁剪放大版本，确保表单字段在 GitHub 页面中可读。

**对应需求：** FR-1、FR-2、NFR-1、NFR-2、NFR-3

## 技术方案

### 技术选型

| 类别 | 选择 | 理由 | 关联需求 |
| --- | --- | --- | --- |
| 文档格式 | GitHub Markdown | 与现有 README 一致，可直接展示编号步骤、提示和图片 | FR-1、FR-2 |
| 图片引用 | 仓库相对路径 | 在 GitHub 和本地预览中都可用 | FR-2 |
| 验证方式 | 链接检查、敏感信息检查、diff 检查 | 文档改动无需运行应用构建 | NFR-2、NFR-3 |

### 信息流

```text
桌面端启动 MCP 与公网隧道
  → 复制公网 /mcp URL 和认证配置
  → ChatGPT 开启开发人员模式
  → 插件页面新建 MCP 插件
  → 保存并完成 OAuth 授权
  → 在新对话中调用 server_info / history_session_bootstrap 验证
```

## 数据模型

不涉及数据存储或模型变更。

## API 设计

不涉及 API 变更；README 只引用现有公网 MCP URL 和已有工具。

## 文件结构

```text
coding-tools-mcp-rust/
├── README.md
├── README.en.md
├── docs/images/gpt-config-1.png
├── docs/images/gpt-config-2.png
├── docs/images/gpt-config-2-detail.png
└── docs/specs/gpt-client-configuration-docs/
    ├── requirements.md
    ├── design.md
    └── tasks.md
```

## 设计决策

### 决策 1：把截图教程放在 MCP Connector 小节（关联需求：FR-1、FR-2）

**问题**：截图属于 ChatGPT 端配置，如果放入桌面端启动步骤，会打断五分钟快速开始并混淆 MCP 与 Actions。

**选项**：

1. 放在“五分钟开始使用”的启动步骤中。
2. 放在“ChatGPT 的两种接入方式”的 MCP Connector 小节中。

**决策**：选择选项 2。

**理由**：该位置已经解释公网 `/mcp` URL，能自然承接桌面端输出并继续到 GPT 端操作，同时不误导 Actions 用户。

### 决策 2：使用字段映射而非逐字绑定界面（关联需求：FR-2）

**问题**：ChatGPT 的菜单和按钮名称可能随版本或语言变化。

**决策**：保留截图中的当前名称，同时增加“界面名称可能变化”的提示，并用字段含义解释配置。

**理由**：既能让当前用户按图操作，也能降低未来 UI 小改版导致教程完全失效的风险。

## 测试策略

- 确认两张图片路径存在且中英文 README 相对链接正确。
- 检查教程覆盖服务启动、URL、开发人员模式、插件字段、OAuth 和验证。
- 检查首屏快速路径和故障排查表能够直接链接到详细步骤。
- 扫描新增文本及图片周边说明，确保没有秘密值或本机敏感路径。
- 运行 `git diff --check` 检查 Markdown 空白问题。

## 风险评估

| 风险 | 影响 | 缓解措施 |
| --- | --- | --- |
| ChatGPT UI 改版 | 中 | 标注入口名称可能随版本变化，步骤聚焦稳定概念 |
| 用户误用本地 URL | 中 | 明确必须使用公网 HTTPS `/mcp` URL |
| 认证字段混淆 | 高 | 明确认证方式必须与桌面端一致，Secret 不写入 README |

## 检查清单

- [x] 技术方案与现有 README 架构一致
- [x] 覆盖 requirements.md 的全部需求
- [x] 文件路径来自真实仓库
- [x] 不涉及数据模型和接口变更
- [x] 关键设计决策已记录
- [x] 测试策略可验证验收标准
