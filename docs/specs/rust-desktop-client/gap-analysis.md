# 新旧项目差距分析

对照 `old/` Python 实现与当前 Rust/Tauri 桌面客户端（2026-07-11）。

## 总结

| 维度 | 状态 |
|------|------|
| **桌面 MVP 核心** | ✅ 已对齐（Workspace、双 Runtime、隧道、认证、健康、日志） |
| **20 个 MCP 工具** | ✅ 名称与 Actions 白名单一致 |
| **工具行为** | ⚠️ 大部分对齐；`glob` 别名已补齐 |
| **安全深度** | ⚠️ 缺 Landlock、shell env 策略、深层 exec 扫描 |
| **MCP 传输层** | ⚠️ 缺 stdio、HTTP 加固、双认证并存 |
| **合规测试** | ⚠️ 13 项直接调用 vs 旧版 ~120+ 项 |
| **分发形态** | ❌ 无 PyPI CLI / Docker / install.sh（桌面内嵌，非目标） |

**Rust 领先旧版：** 内嵌 Runtime（无 Python 子进程）、Actions OAuth、frpc 自动拉起、全局 FRP 配置库、钥匙串、健康面板 UI、legacy 导入、隧道「未配置」模式。

---

## FR 验收对照（tasks 5.2）

| ID | 验收要点 | 状态 | 备注 |
|----|----------|------|------|
| FR-1 | Workspace 卡片首页 | ✅ | `/` + 侧栏；详情在 `/workspace/[id]` |
| FR-2 | CRUD + 重启恢复 | ✅ | `last_workspace_id` 自动跳转 |
| FR-3 | MCP 启停 + 端口占用提示 | ✅ | `runtime/supervisor.rs` |
| FR-4 | P0 工具 + 路径边界 | ✅ | 13 项合规测试通过 |
| FR-5 | FRP + Cloudflare 隧道 | ✅ | Rust 额外支持 frpc 自动管理 |
| FR-6 | OAuth / Bearer / NoAuth | ✅ | MCP + Actions 均支持 |
| FR-7 | 钥匙串存密钥 | ✅ | 配置文件无明文 |
| FR-8 | 健康检查 + hint | ✅ | 8 项检查 |
| FR-9 | MCP/隧道日志 | ✅ | `LogViewer` + frpc 日志 |
| FR-10 | 一键复制 | ✅ | Web Clipboard API（无 Tauri IPC，功能等价） |

---

## 工具层差距

### 已对齐

- 20 工具注册表、`full` / `read-only` / `compat-readonly-all` 三档
- Actions `ALLOWED_TOOLS`（14 工具，与旧版一致）
- `exec_command` argv 直启 + allowlist + `python -c` / shell 链拦截
- `apply_patch` 路径校验 + 大小限制
- `request_permissions` → `ELICITATION_UNSUPPORTED`
- `list_files` / `search_text` 的 `glob` 别名、`exclude_patterns`、`context_lines`

### 部分对齐

| 项 | 旧版 | 新版 |
|----|------|------|
| permission_mode 全部门禁 | safe/trusted/dangerous 多维度 | 主要拦 network + skip gates |
| exec 策略 | 破坏性命令、路径参数、heredoc 等 | allowlist + shell 链 + network regex |
| `search_text` | 优先 `rg`/`fd` 引擎 | WalkDir 纯 Rust |
| `list_files` | 优先 `fd` | WalkDir |
| MCP discovery GET `/mcp` | 完整 server card | 最小 JSON |
| 双端口默认 | MCP 28766 / Actions 8787 | ✅ 已修正 |

### 未实现（Phase 2+ 或 Out of Scope）

- Landlock 沙箱（`landlock_exec.py`）
- Shell env inherit（`core`/`all`/`none` + include/exclude/set）
- MCP stdio 传输（`--stdio`）
- MCP 同时 Bearer + OAuth
- HTTP 加固：Origin 策略、batch 限制、协议版本协商、trace 日志
- ngrok / Dev Tunnel 适配器
- 系统托盘、团队共享 profile
- 独立 PyPI / Docker 分发

---

## Actions 网关差距

| 项 | 旧版 | 新版 |
|----|------|------|
| 路由 `/health` `/openapi.json` `/privacy` `/actions/{tool}` | ✅ | ✅ |
| API key / none 认证 | ✅ | ✅ |
| OAuth | 501 未实现 | ✅ **领先** |
| 变异工具写锁 | `asyncio.Lock` | ✅ 已补齐 |
| Gateway 层 exec/patch 校验 | `policies.py` 重复校验 | 仅在 `call_tool` 内（符合架构约束） |

---

## 桌面 UI 差距

| 项 | 旧版 PySide6 | 新版 Svelte |
|----|-------------|-------------|
| MCP + Actions 双面板 | ✅ | ✅ |
| FRP / Cloudflare | ✅ | ✅ + 全局 FRP 页 |
| 工具档 + permission mode | ✅ | ✅ |
| `runtime_command` 自定义命令 UI | ✅ | ❌ 字段存在无表单（内嵌模式不需要） |
| 健康面板 | 模块有、UI 未接 | ✅ |
| 主题 | `theme.py` | ✅ design tokens |

---

## 测试差距

| 旧版套件 | 约计 | 新版 |
|----------|------|------|
| `test_mcp_contract.py` | 31 | ❌ HTTP 层未移植 |
| `test_tool_golden.py` | 8 | ❌ |
| `test_security.py` | 15 | ⚠️ ~7 项 |
| `test_e2e.py` / dogfood | 8+ | ❌ |
| `test_runtime_helpers.py` | 42 | ❌ |
| **合计** | **~120+** | **15**（含本轮 +2 glob） |

---

## 建议优先级

1. **P1** — 本机 `cargo tauri dev` 冒烟（MCP + Actions + 隧道 + GPT 联调）
2. **P2** — `test_tool_golden.py` 核心用例移植（read/patch/exec/git）
3. **P2** — HTTP MCP contract 测试（initialize、auth、tools/list）
4. **P3** — Landlock / shell env（Linux 专用，requirements 标 Phase 2）
5. **P3** — ngrok、系统托盘

---

## 结论

**桌面 MVP 相对 `old/apps/desktop-client` 已可替代使用**，并在 FRP 管理、Actions OAuth、健康 UI、密钥存储等方面优于旧版。

与完整 `old/` 单仓（含 CLI 包、Docker、全量合规）相比，主要差距在：**传输层加固**、**沙箱深度**、**合规测试覆盖率**、**独立 CLI 分发**——后三项对桌面 MVP 非阻塞。
