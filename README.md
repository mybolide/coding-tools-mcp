# Coding Tools MCP Desktop

Coding Tools MCP Desktop 是一个基于 Rust + Tauri 2 的本地 AI Coding
Workspace。它把项目目录、文件读写、命令执行和 Git 能力通过 MCP 暴露给
ChatGPT、Codex 以及其他 MCP 客户端。

项目的 Rust 实现以 `old/` 中的 Python 版本为行为参考，核心目标是：

```text
打开 Workspace
    ↓
读取代码
    ↓
修改文件
    ↓
运行命令和测试
    ↓
查看 Git 状态和差异
```

## 当前能力

- Rust + Tauri 2 桌面客户端
- MCP Streamable HTTP 服务
- ChatGPT Actions OpenAPI 网关
- OAuth、Bearer Token 和无认证模式
- Workspace 默认目录和子目录切换
- 文件读取、目录浏览、文件搜索和图片查看
- 原子化 Patch、Patch 预检和结构化变更结果
- Workspace 内命令执行、TTY、stdin、超时和输出分页
- Git status、diff、log、show、blame
- FRP 和 Cloudflare 隧道管理
- 全局 FRP 配置和 Workspace 配置

## 默认工具集

默认使用 `core` profile，只暴露与 Python 参考实现一致的核心工具，避免
Agent 一开始面对过多实验性工具：

| 类别 | 工具 |
| --- | --- |
| 文件读取 | `read_file`、`list_dir`、`list_files`、`search_text`、`view_image` |
| 文件修改 | `apply_patch`、`patch_check` |
| 命令执行 | `exec_command`、`write_stdin`、`read_output`、`kill_session` |
| Git | `git_status`、`git_diff`、`git_log`、`git_show`、`git_blame` |
| 环境 | `server_info`、`check_exec_environment`、`get_default_cwd`、`set_default_cwd` |
| 权限 | `request_permissions` |

高级 Harness、任务状态和操作记录工具仍保留在代码中，但不属于默认工具
面；需要时使用显式 `advanced` profile。

## 权限边界

- Workspace 内普通文件：允许读取、创建、修改、删除和执行。
- Workspace 外：`read_file`、`list_dir`、`list_files`、`search_text`、
  `view_image` 允许只读访问。
- `.git` 和 `.github`：普通文件工具和 Patch 禁止写入。
- Git 恢复统一使用本地 Git，不使用 Workspace 快照或工具级 Undo。
- `exec_command` 的 `filesystem_scope` 默认且目前只接受 `workspace`。

注意：Windows 子进程的操作系统级文件系统隔离仍在开发中。当前返回
`sandbox_enforced: false` 是真实状态，不能把静态命令检查当成完整沙箱。

## 环境要求

- Rust stable（2021 edition）
- Node.js 20+
- 当前操作系统对应的
  [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

## 开发

首次安装前端依赖：

```bash
npm install
```

启动桌面开发环境：

```bash
npm run desktop
```

Windows 也可以运行：

```text
dev-desktop.cmd
```

其他常用命令：

```bash
npm run check
npm run build
cd src-tauri && cargo test
cd src-tauri && cargo clippy --all-targets -- -D warnings
```

不要只运行 `npm run dev` 来验证桌面应用；它只会启动 Vite，不会启动
Tauri 外壳。

默认端口：

- MCP：`28766`
- Actions：`8787`
- Vite：`1420`

## 目录结构

| 路径 | 作用 |
| --- | --- |
| `src-tauri/src/tools/` | 共享工具内核和 `call_tool` |
| `src-tauri/src/mcp/` | MCP HTTP 服务 |
| `src-tauri/src/actions/` | Actions OpenAPI 网关 |
| `src-tauri/src/tunnel/` | FRP / Cloudflare 隧道管理 |
| `src-tauri/src/settings/` | 全局设置和 FRP 配置 |
| `src/` | SvelteKit 界面 |
| `old/` | Python 参考实现和兼容测试 |

## ChatGPT Actions

1. 在桌面端启动目标 Workspace 的 Actions 服务。
2. 配置 FRP 或 Cloudflare 隧道。
3. 在 Workspace 的 Actions 认证页面复制 OpenAPI 地址。
4. 在 GPT 编辑器中导入 OpenAPI。
5. 按需要选择 None、API Key 或 OAuth 认证。

## 旧版本迁移

首次启动且新的 profile 存储为空时，会尝试从下面的位置导入旧配置：

```text
~/.coding-tools-mcp-desktop/profiles.json
```

旧的 `secrets.json` 会迁移到操作系统密钥环。

## License

Apache-2.0
