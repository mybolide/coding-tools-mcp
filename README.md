<p align="center">
  <img src="src-tauri/icons/128x128.png" width="96" alt="Coding Tools MCP 图标">
</p>

<h1 align="center">Coding Tools MCP</h1>

<p align="center">
  把本地项目变成 AI 可直接开发、能够跨会话延续上下文的持久工作区。
</p>

<p align="center">
  <a href="https://github.com/mybolide/coding-tools-mcp/releases/latest"><img src="https://img.shields.io/github/v/release/mybolide/coding-tools-mcp?label=Release" alt="Latest release"></a>
  <img src="https://img.shields.io/badge/Windows-x64-0078D4?logo=windows" alt="Windows x64">
  <img src="https://img.shields.io/badge/macOS-Apple%20Silicon-000000?logo=apple" alt="macOS Apple Silicon">
  <a href="https://www.apache.org/licenses/LICENSE-2.0"><img src="https://img.shields.io/badge/license-Apache--2.0-blue" alt="Apache-2.0"></a>
</p>

<p align="center">
  <a href="README.md">中文</a> · <a href="README.en.md">English</a> · <a href="https://github.com/mybolide/coding-tools-mcp/releases/latest">下载最新版</a>
</p>

Coding Tools MCP 是一个 Rust + Tauri 2 桌面应用。选择项目目录并启动服务后，AI Agent 就能通过 MCP 读取文件、修改代码、运行命令和测试、查看 Git 状态，并把关键进度保存为项目内的历史会话。它更接近“AI 打开一个会记住开发进度的 IDE 工作区”；普通开发工具不要求先创建 Task，历史会话则负责在新对话中恢复上下文。

![Coding Tools MCP 工作区总览](docs/images/workspace-overview.png)

*一个桌面端同时管理工作区、MCP 服务、连接信息与会话恢复提示词。*

## 30 秒看懂怎么用

```text
下载安装桌面端
  → 添加项目目录
  → 启动 MCP 和公网隧道
  → 复制“公网 MCP 地址”
  → ChatGPT 开启开发人员模式
  → 新建 MCP 插件并粘贴地址
  → 完成授权，在新对话中开始开发
```

第一次使用只需要记住两件事：**桌面端负责把项目变成 MCP 工作区，ChatGPT 负责通过公网 `/mcp` 地址连接它。**

- [查看完整安装和桌面端启动步骤](#五分钟开始使用)
- [直接查看 ChatGPT 插件配置](#mcp-connector)

## 五分钟开始使用

### 1. 安装桌面客户端

打开 [Releases](https://github.com/mybolide/coding-tools-mcp/releases/latest) 并下载对应安装包：

| 系统 | 安装包 |
| --- | --- |
| Windows 10/11 x64 | `Coding.Tools.MCP_*_x64-setup.exe` |
| macOS Apple Silicon | `Coding Tools MCP_*_aarch64.dmg` |

macOS 安装包目前未签名。如果系统阻止首次打开，请在“系统设置 → 隐私与安全性”中确认打开。

### 2. 添加项目工作区

1. 点击左侧的“添加工作区”。
2. 选择项目根目录。
3. 设置工作区名称、MCP 端口和认证方式。
4. 保存后，工作区会长期保留在左侧列表中。

### 3. 配置公网隧道

如果 AI 客户端不在本机，需要把本地 MCP 暴露为 HTTPS 地址：

- 在“软件管理”中安装或识别 `frpc` / `cloudflared`。
- 在“FRP 配置”中保存服务器、端口和 Token，或在工作区选择 Cloudflare。
- 每个工作区填写独立子域名。应用会统一管理 FRP 进程和多条代理线路。

![FRP 配置页面](docs/images/frp-configuration.png)

*FRP 服务器配置集中保存，各工作区只需选择配置并填写自己的子域名。*

如果还没有可用的 FRPS 服务端，可以参考：[FRPS 服务端安装教程（微信公众号）](https://mp.weixin.qq.com/s/kmpQhHsvmHlaLfj4rw3A0Q)。安装完成后，把服务端地址、端口和 Token 填入客户端的“FRP 配置”即可。

### 4. 启动 MCP

进入工作区并点击 MCP 的“启动”。客户端会显示：

- 本地 MCP 地址，例如 `http://127.0.0.1:28766/mcp`；
- 公网 HTTPS MCP 地址；
- ChatGPT 连接所需的认证信息；
- 实时日志和健康检查结果。

![MCP 本地、公网与 ChatGPT 连接信息](docs/images/workspace-connection.png)

启动后可以直接检查本地与公网端点、OAuth 元数据和 MCP 受保护资源：

![MCP 健康检查结果](docs/images/health-check.png)

*健康检查会逐项显示连接和认证元数据是否可用。*

遇到连接问题时，无需离开桌面端即可查看最近的 MCP 请求日志：

![MCP 运行日志](docs/images/runtime-logs.png)

*日志可快速确认工具列表、历史初始化和检查点调用是否真正到达服务端。*

### 5. 连接 AI 客户端

支持 MCP 的客户端使用界面中的公网 MCP URL。使用 OAuth 时，客户端会通过服务端元数据进入授权流程；授权口令、Client ID 和 Secret 均可在桌面端集中生成和管理。当前版本使用预配置 OAuth 客户端，创建 ChatGPT 插件时应选择静态/手动 OAuth 凭据，不需要选择 CIMD。

首次连接建议先调用历史初始化，再检查工作区：

```text
history_session_bootstrap
server_info
get_default_cwd
git_status
check_exec_environment
```

这样 Agent 不需要依赖聊天上下文猜测当前项目、工作目录和执行能力。

## ChatGPT 的两种接入方式

| 方式 | 适合场景 | 在客户端中使用什么 |
| --- | --- | --- |
| MCP Connector | ChatGPT 直接使用文件、命令和 Git 工具 | 工作区的公网 `/mcp` 地址 |
| GPT Actions | 在自定义 GPT 中导入 OpenAPI 工具 | Actions 面板中的 `/openapi.json` 地址 |

### MCP Connector

配置前请先确认：

1. 工作区的 MCP 服务和公网隧道均处于运行状态。
2. “健康检查”中的公网 MCP 检查通过；如果使用 OAuth，再确认 OAuth 受保护资源和授权元数据检查通过。
3. 从桌面端“GPT 配置”卡片复制“公网 MCP 地址”；如果使用 OAuth，同时准备 OAuth Client ID、OAuth Client Secret 和授权口令。

> ChatGPT 必须使用公网 HTTPS `/mcp` 地址，不能使用 `http://127.0.0.1:28766/mcp` 之类的本地地址。ChatGPT 的菜单名称可能随版本和语言设置略有变化。

#### 1. 开启 ChatGPT 开发人员模式

打开 ChatGPT 设置，进入“账户安全与登录”，开启“开发人员模式”。该开关允许添加未经验证的 MCP 连接器。

![在 ChatGPT 中开启开发人员模式](docs/images/gpt-config-1.png)

*开发人员模式具有较高权限，只应连接你自己部署或明确可信的 MCP 服务。*

#### 2. 创建 MCP 插件

在 ChatGPT 左侧进入“插件”，点击右上角的 `+` 新建插件，然后选择 MCP（测试版）并填写：

| ChatGPT 字段 | 填写内容 |
| --- | --- |
| 名称 | 自定义一个容易识别的名称，例如 `Coding Tools MCP` |
| 描述 | 简要说明它连接的项目或用途 |
| 连接 | 粘贴桌面端“GPT 配置”中的公网 MCP 地址，URL 应以 `/mcp` 结尾 |
| 身份验证 | 与桌面端保持一致；截图以 OAuth 为例 |

![在 ChatGPT 中新建 MCP 插件并填写连接信息](docs/images/gpt-config-2-detail.png)

使用 OAuth 时，展开“高级 OAuth 设置”，选择静态/手动 OAuth 凭据并填写桌面端提供的 Client ID 和 Client Secret，不需要选择 CIMD。保存或连接后，ChatGPT 会打开授权页面；输入桌面端“GPT 配置”卡片中的授权口令完成首次授权。

> Client Secret、授权口令和 Bearer Token 都属于敏感信息，不要粘贴到对话、Issue 或公开截图中。若桌面端使用 Bearer 或不启用认证，请在 ChatGPT 中选择当前界面提供的对应认证方式。

#### 3. 验证连接

创建一个启用了该插件的新对话，并发送：

```text
请使用 Coding Tools MCP 调用 server_info、get_default_cwd 和 git_status，
告诉我当前连接的工作区、默认目录和 Git 状态。
```

如果能够返回当前项目的信息，说明“桌面端 → 公网隧道 → OAuth → ChatGPT → MCP 工具”链路已经打通。首次正式开发时，再调用 `history_session_bootstrap` 初始化或恢复项目历史。

如果 ChatGPT 仍显示旧的工具列表，请断开并重新连接插件，或创建一个新对话后再次验证。

#### 常见问题

| 现象 | 优先检查 |
| --- | --- |
| ChatGPT 无法连接 | 是否使用公网 HTTPS `/mcp` 地址，而不是 `127.0.0.1`；桌面端公网 MCP 健康检查是否通过 |
| OAuth 授权失败 | Client ID、Client Secret 和授权口令是否来自同一个工作区；OAuth 元数据检查是否通过 |
| 看不到新增工具 | 断开并重新连接插件，然后创建一个新对话 |
| 工具调用失败 | 打开桌面端“日志”和“健康检查”，确认请求是否到达 MCP 服务 |

### 将本地 MCP 一并暴露

除了本应用内置的开发工具，工作区还可以受控地代理管理员明确配置的 MCP。支持本机进程的 **stdio**，以及本机或可达服务的 **Streamable HTTP**。ChatGPT 或云端 Codex 始终只连接本应用原有的公网 `/mcp` 地址，桌面端再把调用转发给已保存的上游。

在工作区的 **MCP → 配置 → 本地 MCP** 中点击 **“+ 添加”**：

1. 选择 **快速创建**，再选择 `stdio` 或 `Streamable HTTP` 传输。
2. 或选择 **从 JSON 导入**，粘贴单个配置、配置数组，或常见的 `mcpServers` 对象；导入只生成草稿，须审阅后保存。
3. `stdio` 填写命令、参数和环境变量；`Streamable HTTP` 填写端点 URL 和可选请求头。
4. 勾选该本地 MCP 的“启用”开关，并点击“获取工具”。
5. 默认所有已发现工具都会公开；按需要逐项关闭高权限工具。
6. 保存。若 MCP 已在运行，桌面端会重新加载；否则在下次启动 MCP 时生效。

Streamable HTTP 导入示例：

```json
{
  "mcpServers": {
    "local-service": {
      "type": "streamableHttp",
      "url": "http://127.0.0.1:3000/mcp",
      "headers": {
        "Authorization": "Bearer your-token"
      },
      "tool_prefix": "local-service"
    }
  }
}
```

远端客户端看到的工具会带工作区配置的前缀，例如 `local__tool_name`。旧版手工白名单会保持原有公开范围，直到在新版界面保存逐项开关。

> 安全边界：环境变量仅注入本机 stdio 进程；HTTP 请求头仅发送到其保存的端点。两者都不会出现在 `/mcp` 工具目录、工具响应或普通请求日志中。自动公开会让远端获得该服务提供的所有本机能力；请在保存前逐项关闭不希望暴露的工具，且不要配置不受信任的 MCP 或包含敏感凭据的环境变量。

### GPT Actions

1. 启动工作区的 Actions 服务。
2. 复制 Actions 面板中的 OpenAPI URL。
3. 在 GPT 编辑器的 Actions 页面导入该 URL。
4. 根据桌面端配置选择 None、API Key 或 OAuth。

MCP 和 Actions 可以为同一个工作区同时运行，也可以分别使用不同端口和子域名。

## 为什么需要它

- **面向真实开发**：文件、命令、Git、测试和长时间运行的进程都在同一个 Workspace 中。
- **跨会话持续开发**：新对话先获得有界的当前状态，需要精确旧上下文时按关键词定位并读取原始档案，无需反复向 AI 解释项目背景和当前进度。
- **进度可追溯**：每轮任务完成后可保存结构化检查点，决策、修改、测试结果和下一步都留在项目目录中。
- **多工作区管理**：一个桌面客户端可以保存多个项目，并管理各自的 MCP、Actions 和公网地址。
- **连接 ChatGPT 更直接**：内置 Streamable HTTP、OAuth、Bearer Token、OpenAPI、FRP 和 Cloudflare 隧道。
- **默认工具面保持简单**：稳定的核心工具默认可用，高级 Harness 能力按需开启。

## 让项目记住每次对话

普通聊天记录适合回看交流内容，但不适合作为长期开发交接。Coding Tools MCP 将会话进度写入当前项目的 `docs/history-session/`，让上下文跟随项目，而不是困在某一个聊天窗口里。

![ChatGPT 新会话启动提示词](docs/images/history-session-prompt.png)

*复制完整提示词到新会话，即可初始化或恢复历史；每轮任务完成后再保存检查点。*

它提供五个互相配合的历史工具：

| 工具 | 作用 |
| --- | --- |
| `history_session_bootstrap` | 新对话开始时初始化或恢复项目会话；保存逐字的 `initial_user_input`，返回稳定的 `session_key`、`current_path` 和有界当前状态，不返回全量历史 |
| `history_session_checkpoint` | 每轮任务完成后按 bootstrap 返回的稳定目标追加结构化进度，并保存逐字的 `raw_user_input`；目标不一致时拒绝写入，避免串到其他历史文件 |
| `history_session_validate` | 检查历史编号、文件和会话映射；必要时重建派生索引，不删除已有历史 |
| `history_session_search` | 按确定性关键词搜索长期 Markdown 档案，返回有界的命中位置和短片段 |
| `history_session_read` | 按编号或搜索结果位置，无损、UTF-8 安全地分页读取一份原始 Markdown 档案；默认每页 `32 KiB`，最多 `64 KiB`，根据 `next_cursor` 继续读取 |

典型效果：

```text
对话 1：分析项目 → 修改代码 → 运行测试 → 保存检查点
                                      ↓
对话 2：读取有界当前状态 → 搜索并精读需要的旧档案 → 从上次进度继续 → 保存新检查点
```

历史档案使用可读的 Markdown 格式，可以随项目备份或纳入 Git，也方便开发者直接审阅和修订。`memory/state.json` 是有界当前状态投影，`memory/manifest.json` 只保存位置、哈希与关键词，不复制正文；Markdown 才是长期、无损的事实来源。首次输入和每轮输入必须由 ChatGPT 作为 `initial_user_input`、`raw_user_input` 工具参数传入，服务端无法读取未传入的远程聊天文本。检查点采用幂等追加，同一 `turn_id` 内容变化时保留 revision 与 supersedes 证据，并要求返回 `ok=true` 且会话目标一致后才确认保存成功。

> 历史持久化由 AI 调用 MCP 工具完成，并非桌面端在后台录制聊天内容。若客户端未触发工具调用，服务端无法凭空感知新的对话或任务进度。

## Agent 可以做什么

默认 `core` profile 提供一组稳定、可组合的开发工具：

| 类别 | 主要工具 |
| --- | --- |
| 文件读取 | `read_file`、`list_dir`、`list_files`、`search_text`、`grep_text`、`view_image` |
| 文件修改 | `apply_patch` |
| 命令执行 | `exec_command`、`write_stdin`、`read_output`、`kill_session` |
| Git | `git_status`、`git_diff`、`git_log`、`git_show`、`git_blame` |
| 环境 | `server_info`、`check_exec_environment`、`get_default_cwd`、`set_default_cwd` |
| 历史会话 | `history_session_bootstrap`、`history_session_checkpoint`、`history_session_validate`、`history_session_search`、`history_session_read` |

典型开发过程：

```text
打开 Workspace
  → 理解项目和 Git 状态
  → 搜索并读取代码
  → 事务化应用 Patch
  → 运行命令和测试
  → 检查 diff 并提交
```

高级 profile 还保留项目状态、操作记录等 Harness 能力，但普通文件修改和命令执行不要求先创建 Task。

## 权限与恢复模型

项目采用 Workspace-first 权限模型：

- Workspace 内普通文件可以读取、创建、修改、删除和执行。
- Workspace 外允许完整只读：`read_file`、`list_dir`、`list_files`、`search_text`、`view_image`。
- Workspace 外写入、删除和执行会被阻止。
- `.git` 和 `.github` 不能被普通文件工具、Patch 或解释器命令破坏。
- Patch 在单次操作内进行预检和失败恢复；长期恢复统一使用 Git，不创建全量 Workspace Snapshot。

> Windows 子进程目前仍是 `policy_only` 执行边界，返回中的 `sandbox_enforced: false` 是真实状态。静态命令策略不能等同于完整的操作系统文件系统沙箱。

## 本地开发

环境要求：Node.js 20+、Rust stable，以及当前系统的 [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/)。

```bash
npm install
npm run desktop
```

常用验证命令：

```bash
npm run check
npm run build
cd src-tauri && cargo test
cd src-tauri && cargo clippy --all-targets -- -D warnings
```

Windows 也可以双击 `dev-desktop.cmd`。不要只用 `npm run dev` 验证桌面应用，它只启动 Vite，不会启动 Tauri 外壳。

## 项目结构

| 路径 | 作用 |
| --- | --- |
| `src-tauri/src/tools/` | 文件、Patch、Exec、Git 等共享工具内核 |
| `src-tauri/src/mcp/` | MCP Streamable HTTP 服务 |
| `src-tauri/src/actions/` | ChatGPT Actions OpenAPI 网关 |
| `src-tauri/src/tunnel/` | FRP / Cloudflare 隧道和进程管理 |
| `src/` | SvelteKit 桌面界面 |
| `old/` | Python 参考实现和兼容性基线 |

## 致谢
感谢 [Linux.do](https://linux.do/) 社区对项目推广与反馈的支持。

## License

[Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0)
