# 需求文档：GitHub README 展示

## 功能概述

面向首次接触 Coding Tools MCP 的 ChatGPT、Codex 和其他 MCP 客户端用户，重写中英文 GitHub README，并用真实桌面客户端截图说明从安装、创建工作区到启动公网 MCP/Actions 的完整路径。

## 历史经验与边界

- README 只描述已经发布、可验证的能力，不伪造用户反馈、运行状态或安全保证。
- 截图来自真实客户端，隐藏本机路径、口令、Token、Client Secret 等敏感信息。
- macOS 工作流仍保持手动触发，README 不宣称自动发布。

## 术语定义

- **Workspace**：用户选择并授权给 AI Coding Agent 操作的项目目录。
- **MCP**：向支持 MCP 的 AI 客户端暴露文件、命令、Git 等工具的 Streamable HTTP 服务。
- **Actions**：供 ChatGPT GPT Actions 导入的 OpenAPI 网关。

## 范围边界

**In Scope**

- 重写 `README.md` 与 `README.en.md`。
- 新增真实桌面客户端截图并在两个 README 中复用。
- 提供 Windows、macOS 下载入口、五分钟快速开始、ChatGPT 接入和安全边界。

**Out of Scope**

- 修改桌面客户端功能或 UI。
- 伪造示例 Workspace、用户评价或运行结果。
- 新增自动触发 macOS 发布流程。

## 需求列表

### FR-1：首屏说明产品价值

**优先级：Must**

作为首次访问仓库的用户，我想立即理解项目解决什么问题、支持哪些客户端和系统，以便决定是否下载。

1. WHEN 用户打开 README THEN 页面 SHALL 在首屏展示产品定位、平台、Release 和语言入口。
2. IF 某项能力尚未实现 THEN README SHALL 不把它写成已支持能力。

### FR-2：用真实截图呈现核心流程

**优先级：Must**

作为准备安装的用户，我想看到真实界面，以便理解工作区、服务和隧道的关系。

1. WHEN 用户浏览产品预览 THEN README SHALL 展示至少两张真实客户端截图。
2. IF 截图含本机路径或密钥 THEN 交付前 SHALL 遮挡或避免暴露敏感内容。

### FR-3：提供可执行的快速开始

**优先级：Must**

作为新用户，我想按顺序完成安装、创建工作区、启动服务和复制连接地址，以便在五分钟内完成首次连接。

1. WHEN 用户阅读快速开始 THEN README SHALL 给出按顺序编号的操作步骤。
2. IF 用户使用 ChatGPT THEN README SHALL 区分 MCP Connector 与 GPT Actions 两种接入方式。

### FR-4：准确说明能力和安全边界

**优先级：Must**

作为开发者，我想知道 Agent 能做什么、不能做什么，以便评估是否适合我的项目。

1. WHEN 用户查看能力说明 THEN README SHALL 列出核心工具类别、多工作区和公网隧道能力。
2. WHEN 用户查看安全说明 THEN README SHALL 明确 Workspace 内权限、Workspace 外只读、`.git`/`.github` 保护以及 Windows `policy_only` 现状。

### FR-5：保持中英文内容一致

**优先级：Should**

作为中文或英文用户，我想获得同等完整的信息，以便使用自己熟悉的语言完成配置。

1. WHEN 任一 README 更新核心流程 THEN 另一语言版本 SHALL 包含相同的章节和事实。

## 非功能需求

- **NFR-1（可读性）**：安装和首次连接步骤应在三分钟内可扫读完成。
- **NFR-2（安全）**：仓库图片不得包含真实密钥、授权口令或用户隐私数据。
- **NFR-3（兼容性）**：图片使用 GitHub 可直接渲染的 PNG，链接使用仓库相对路径。

## 依赖关系

- GitHub Release v0.1.6 的 Windows x64 和 macOS Universal 安装包。
- 当前 Tauri 客户端真实界面与已发布功能。
