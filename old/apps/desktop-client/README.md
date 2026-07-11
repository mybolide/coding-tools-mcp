# MCP 桌面客户端

这是 `coding-tools-mcp` 的 Python 桌面客户端 MVP，核心目标是让研发同学用一个中文界面完成：

- 管理多个 Workspace
- 在同一个 Workspace 下切换 `MCP` / `Actions` 两套入口
- 配置公网暴露地址，当前支持 FRP 和 Cloudflare
- 配置 MCP 的 OAuth / Bearer / NoAuth
- 配置 Actions 的 Bearer API Key、OpenAPI 地址和隐私政策地址
- 启动和停止本地 MCP / Actions 运行时
- 查看两套运行时的日志和当前入口地址

## 运行

```bash
python apps/desktop-client/main.py
```

## 依赖

- Python 3.11+
- PySide6
- psutil
- `uvx` 或 `coding-tools-mcp` 已在 PATH 中可用

## ChatGPT 接入

### MCP 页签

当认证方式选择 `oauth` 后，界面里会直接展示并支持复制：

- 连接地址
- OAuth 客户端 ID
- OAuth 客户端密钥
- 授权口令

如果你使用 FRP，只需要把 Workspace、本地端口、FRP 子域名和服务器域名配好，再启动运行时即可。

如果你使用 Cloudflare，有两种模式：

- 临时隧道：使用 `cloudflared tunnel --url`，启动后自动分配一个 `trycloudflare.com` 公网地址
- 固定域名：使用 `Tunnel Token` 启动命名隧道，并在界面里填写固定公网地址

### Actions 页签

Actions 页签用于给私有 GPT 配置自定义 Actions：

- Bearer API Key
- OpenAPI 地址
- 隐私政策地址
- 本地 Actions 监听端口
- 允许命令白名单
- 最大 Patch 字节数

在私有 GPT 中，选择 `API Key / Bearer`，再把桌面端里生成的 `OpenAPI 地址` 和 `隐私政策地址` 复制过去即可。

## 当前限制

- 当前只接通了 FRP 和 Cloudflare
- `Ngrok`、`Dev Tunnel` 还没有实现真实隧道启动能力
- Cloudflare 命名隧道模式依赖你提前在 Cloudflare 仪表盘里配置好 tunnel 和 hostname
- Cloudflare 命名隧道模式下，本地服务地址需要和 Cloudflare Tunnel 的 ingress 目标一致，通常是 `http://127.0.0.1:<本地端口>`
- Actions 运行时依赖 `coding-tools-actions` 或本地仓库里的 `coding_tools_actions` 模块可用
