<p align="center">
  <img src="src-tauri/icons/128x128.png" width="96" alt="Coding Tools MCP 图标">
</p>

<h1 align="center">Coding Tools MCP</h1>

<p align="center">
  把本地项目变成 ChatGPT、Codex 和其他 AI Coding Agent 可直接使用的开发工作区。
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

Coding Tools MCP 是一个 Rust + Tauri 2 桌面应用。选择项目目录并启动服务后，AI Agent 就能通过 MCP 读取文件、修改代码、运行命令和测试、查看 Git 状态。它更接近“AI 打开一个 IDE 工作区”，而不是要求 Agent 先创建 Task 或 Session 的工作流系统。

![Coding Tools MCP 工作区总览](docs/images/workspace-overview.png)

## 为什么需要它

- **面向真实开发**：文件、命令、Git、测试和长时间运行的进程都在同一个 Workspace 中。
- **不依赖聊天生命周期**：切换对话不会让项目目录和运行环境消失。
- **多工作区管理**：一个桌面客户端可以保存多个项目，并管理各自的 MCP、Actions 和公网地址。
- **连接 ChatGPT 更直接**：内置 Streamable HTTP、OAuth、Bearer Token、OpenAPI、FRP 和 Cloudflare 隧道。
- **默认工具面保持简单**：稳定的核心工具默认可用，高级 Harness 能力按需开启。

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

如果还没有可用的 FRPS 服务端，可以参考：[FRPS 服务端安装教程（微信公众号）](https://mp.weixin.qq.com/s/kmpQhHsvmHlaLfj4rw3A0Q)。安装完成后，把服务端地址、端口和 Token 填入客户端的“FRP 配置”即可。

### 4. 启动 MCP

进入工作区并点击 MCP 的“启动”。客户端会显示：

- 本地 MCP 地址，例如 `http://127.0.0.1:28766/mcp`；
- 公网 HTTPS MCP 地址；
- ChatGPT 连接所需的认证信息；
- 实时日志和健康检查结果。

![MCP 本地、公网与 ChatGPT 连接信息](docs/images/workspace-connection.png)

### 5. 连接 AI 客户端

支持 MCP 的客户端使用界面中的公网 MCP URL。使用 OAuth 时，客户端会通过服务端元数据进入授权流程；授权口令、Client ID 和 Secret 均可在桌面端集中生成和管理。

首次连接建议让 Agent 依次调用：

```text
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

1. 启动工作区的 MCP 服务和公网隧道。
2. 在 ChatGPT 的连接器/MCP 设置中创建连接。
3. 粘贴客户端显示的公网 MCP 地址。
4. 按桌面端选择的 None、Bearer 或 OAuth 方式完成认证。

### GPT Actions

1. 启动工作区的 Actions 服务。
2. 复制 Actions 面板中的 OpenAPI URL。
3. 在 GPT 编辑器的 Actions 页面导入该 URL。
4. 根据桌面端配置选择 None、API Key 或 OAuth。

MCP 和 Actions 可以为同一个工作区同时运行，也可以分别使用不同端口和子域名。

## Agent 可以做什么

默认 `core` profile 提供一组稳定、可组合的开发工具：

| 类别 | 主要工具 |
| --- | --- |
| 文件读取 | `read_file`、`list_dir`、`list_files`、`search_text`、`grep`、`view_image` |
| 文件修改 | `apply_patch`、`patch_check` |
| 命令执行 | `exec_command`、`write_stdin`、`read_output`、`kill_session` |
| Git | `git_status`、`git_diff`、`git_log`、`git_show`、`git_blame` |
| 环境 | `server_info`、`check_exec_environment`、`get_default_cwd`、`set_default_cwd` |

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

## License

[Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0)
