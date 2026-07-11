# GPT Actions Gateway

这个网关为 `coding-tools-mcp` 增加了一条给私有 GPT 使用的 REST/OpenAPI 入口，同时保留原有 `/mcp` 主链路不变。

## 设计目标

- 不改现有 MCP runtime 主逻辑
- 通过 `stdio` 复用现有工具能力
- 用独立的 Bearer API Key 给 GPT Actions 做鉴权
- 先提供安全收敛后的读、改、diff、测试闭环

## 主要入口

- `GET /health`
- `GET /openapi.json`
- `GET /privacy`
- `POST /actions/{tool_name}`

默认暴露的工具：

- `server_info`
- `check_exec_environment`
- `read_file`
- `list_dir`
- `list_files`
- `search_text`
- `apply_patch`
- `exec_command`
- `read_output`
- `git_status`
- `git_diff`
- `git_log`
- `git_show`
- `git_blame`

## 本地运行

先安装依赖：

```bash
python -m pip install -e ".[actions,dev]"
```

设置环境变量：

```bash
set ACTIONS_API_KEY=replace-with-a-long-random-token
set ACTIONS_WORKSPACE=E:\path\to\target-repo
set ACTIONS_PUBLIC_BASE_URL=https://actions.example.com
```

启动服务：

```bash
coding-tools-actions
```

## Docker Compose

使用独立的 Compose 文件：

```bash
docker compose -f docker-compose.actions.yml up --build
```

需要提供：

- `ACTIONS_API_KEY`
- `TARGET_REPOSITORY_PATH`
- 可选：`ACTIONS_PUBLIC_BASE_URL`

## GPT 配置

在私有 GPT 的 Actions 配置页中：

- 身份验证选择 `API Key`
- 认证类型选择 `Bearer`
- Key 填写与 `ACTIONS_API_KEY` 相同的值
- OpenAPI Schema URL：

```text
https://actions.example.com/openapi.json
```

- 隐私政策：

```text
https://actions.example.com/privacy
```

## 安全约束

- `apply_patch` 和 `exec_command` 属于可写操作
- `exec_command` 只允许白名单命令
- 禁止 shell 链式执行、重定向、内联脚本
- 禁止由 GPT 自带环境变量覆盖
- `set_default_cwd`、`write_stdin`、`kill_session`、`request_permissions`、`view_image` 不对 Actions 暴露
