# 需求文档：history-session-archive

## 功能概述

为 ChatGPT 网页版新增跨会话开发状态归档能力。用户在新聊天中添加现有插件并输入“恢复会话”后，ChatGPT 调用 `history_session_bootstrap`，服务端依据工具调用元数据 `_meta["openai/session"]` 识别当前聊天，读取 `docs/history-session/` 中的历史并返回累计摘要、逐会话摘要和最新完整 handoff。功能支持 Windows、macOS 和 Linux，复用现有 Streamable HTTP `/mcp` 与隧道，不引入 OpenAI SDK，不改变任何现有工具的输入、输出或业务语义。

## 历史经验与坑（来自记忆库）

- **可复用经验**: MCP 与 Actions 必须继续使用统一工具注册表和 `call_tool` 分发入口；工具名称、Schema、注册与分发需要由契约测试共同约束。
- **必须规避的坑**: 不允许出现工具注册名与实际分发名不一致；不在前后端重复触发生命周期；不把会话状态放入仅进程内有效的缓存；不使用标题、首条消息或时间戳推测会话身份。

---

## 范围边界

- **In Scope**:
  - 新增 `history_session_bootstrap`、`history_session_checkpoint`、`history_session_validate` 三个 MCP 工具。
  - 使用 ChatGPT `_meta["openai/session"]` 作为首选稳定会话标识，并允许显式 `session_key` 供测试和兼容客户端使用。
  - 在当前 MCP Runtime 已绑定的工作区内读写 `docs/history-session/`。
  - 支持数字 Markdown、`README.md`、`index.json`、跨进程文件锁、原子替换、索引重建、结构化错误和敏感信息过滤。
  - 支持 Windows、macOS、Linux 的路径、锁和原子替换语义。
- **Out of Scope**:
  - 修改或包裹现有编码工具的执行行为。
  - 引入 OpenAI SDK、Responses API 或 Chat Completions API。
  - 读取 ChatGPT 完整聊天转录、浏览器 DOM 或调用 ChatGPT 私有接口。
  - 自动提交 Git、执行 Shell、删除历史文件或在工作区外写入。
  - 对任意非结构化历史调用外部模型生成摘要。

---

## 需求列表

### FR-1: 新会话恢复与编号

**优先级:** Must
**用户故事:** 作为 ChatGPT 网页版开发者，我想在新聊天中输入“恢复会话”后恢复完整开发交接，以便继续上一会话而不重复排查。

#### 验收标准（EARS）

1. WHEN ChatGPT 调用 `history_session_bootstrap` 且 `_meta["openai/session"]` 存在 THEN 系统 SHALL 使用该值作为当前 `session_key`，无需用户复制会话 ID。
2. WHEN 当前 `session_key` 首次出现 THEN 系统 SHALL 在锁内扫描历史、校验编号并创建下一个纯数字 Markdown 文件。
3. WHEN 同一 `session_key` 重复调用 THEN 系统 SHALL 返回原编号文件且不得重复创建。
4. WHEN `_meta["openai/session"]` 与显式 `session_key` 同时存在 THEN 系统 SHALL 使用宿主元数据并返回来源 `platform_conversation_id`。
5. IF 两种会话标识均不存在 THEN 系统 SHALL 返回 `SESSION_ID_UNAVAILABLE`，不得生成不可复用的临时 ID。

### FR-2: 全历史摘要和最新 handoff

**优先级:** Must
**用户故事:** 作为恢复会话的开发者，我想获得所有阶段摘要和最近一次完整交接，以便保留早期决定并重点恢复最新状态。

#### 验收标准（EARS）

1. WHEN bootstrap 成功 THEN 系统 SHALL 按数字升序解析全部历史文件并返回 `session_summaries`。
2. WHEN bootstrap 成功 THEN 系统 SHALL 返回跨全部既有会话合并的 `all_history_summary`。
3. WHEN存在当前会话之前的历史 THEN 系统 SHALL 在 `latest_handoff` 返回最大既有编号文件的完整 UTF-8 内容。
4. WHEN 历史总量超过完整注入阈值 THEN 系统 SHALL 返回摘要加最新全文，并将 `full_history_included` 设为 `false`。
5. WHEN 返回历史 THEN 系统 SHALL 同时返回读取模式、总字节数和 SHA-256 摘要。

### FR-3: 幂等检查点

**优先级:** Must
**用户故事:** 作为持续开发的用户，我想把每轮压缩交接写入当前会话文件，以便下一聊天恢复准确状态。

#### 验收标准（EARS）

1. WHEN `history_session_checkpoint` 收到新的 `session_key + turn_id` THEN 系统 SHALL 将结构化检查点写入当前编号文件。
2. WHEN相同 `turn_id` 以相同内容重试 THEN 系统 SHALL 不重复追加并返回 `duplicate_ignored=true`。
3. WHEN相同 `turn_id` 内容变化 THEN 系统 SHALL 原位更新对应检查点块而不是新增重复块。
4. WHEN写入成功 THEN 系统 SHALL 返回最终文件 SHA-256、会话编号和相对路径。
5. IF `session_key` 未经 bootstrap 建立映射 THEN 系统 SHALL 返回 `SESSION_NOT_BOOTSTRAPPED`。

### FR-4: 历史完整性校验与安全修复

**优先级:** Must
**用户故事:** 作为维护者，我想校验和恢复历史索引，以便发现编号缺口、重复映射和损坏文件而不丢失历史。

#### 验收标准（EARS）

1. WHEN `history_session_validate` 运行 THEN 系统 SHALL 报告编号、缺失编号、非法文件、空文件、重复 session key、最新编号和最新路径。
2. WHEN发现 `1.md`、`3.md` THEN 系统 SHALL 报告缺失编号 2 且不得创建或覆盖 `2.md`。
3. WHEN `index.json` 缺失或损坏且 `repair=true` THEN 系统 SHALL 从 Markdown 元数据重建索引。
4. WHEN `repair=false` THEN 系统 SHALL 保持只读。
5. IF 发现无法确定的冲突 THEN 系统 SHALL 返回结构化告警，不得删除或改名历史文件。

### FR-5: 跨平台文件安全

**优先级:** Must
**用户故事:** 作为 Windows、macOS 或 Linux 用户，我想安全地在项目内保存历史，以便插件在各桌面平台行为一致。

#### 验收标准（EARS）

1. WHEN解析 `history_dir` THEN 系统 SHALL 使用平台路径 API 并限制最终路径位于当前工作区内。
2. IF `history_dir` 为绝对路径、包含父级穿越或经符号链接逃逸 THEN 系统 SHALL 返回 `PATH_OUTSIDE_WORKSPACE`。
3. WHEN写入 Markdown 或索引 THEN 系统 SHALL 获取跨进程独占锁并使用同目录临时文件加原子替换。
4. IF 无法获得锁或原子替换失败 THEN 系统 SHALL 返回可恢复的结构化错误，不得留下半写文件。
5. WHEN创建文本文件 THEN 系统 SHALL 使用 UTF-8。

### FR-6: 敏感信息过滤

**优先级:** Must
**用户故事:** 作为开发者，我想在归档前自动清理密钥，以便历史文件可以安全留在仓库中。

#### 验收标准（EARS）

1. WHEN checkpoint 字段包含疑似 API Key、Token、Cookie、Bearer、密码或私钥 THEN 系统 SHALL 将敏感值替换为 `[REDACTED]`。
2. WHEN发生脱敏 THEN 系统 SHALL 在返回值 `warnings` 中报告脱敏，不得回显原值。
3. WHEN内容仅包含普通技术文本 THEN 系统 SHALL 保持原意和字段结构。

### FR-7: 增量集成和工具契约

**优先级:** Must
**用户故事:** 作为现有工具用户，我想在获得历史能力的同时保持所有原有工具行为不变，以便升级不会造成回归。

#### 验收标准（EARS）

1. WHEN客户端调用 `tools/list` THEN 系统 SHALL 在现有工具之外返回三个历史工具及完整 JSON Schema。
2. WHEN调用历史工具 THEN MCP 与 Actions SHALL 继续通过唯一 `call_tool` 入口执行。
3. WHEN调用任一既有工具 THEN 系统 SHALL 保持原输入、输出、权限和执行路径不变。
4. WHEN构建依赖解析 THEN 系统 SHALL 不包含 OpenAI SDK。

### FR-8: ChatGPT 持久化工作流提示

**优先级:** Must
**用户故事:** 作为 ChatGPT 网页版用户，我希望插件明确告诉模型何时恢复和何时写入检查点，以便不需要每轮人工提醒。

#### 验收标准（EARS）

1. WHEN ChatGPT 初始化 MCP 连接 THEN 系统 SHALL 在 `initialize.result.instructions` 中说明恢复时使用 bootstrap、需要持久化交接时使用 checkpoint，不得在工具元数据中注入“每次最终回复前必须调用”一类全局行为控制。
2. WHEN `history_session_bootstrap` 成功 THEN 系统 SHALL 返回结构化 `assistant_instructions`、`required_next_actions` 和 `checkpoint_policy`，说明 checkpoint 能力可用但不会后台自动执行。
3. WHEN客户端调用 `tools/list` THEN bootstrap 与 checkpoint 描述 SHALL 使用纯能力说明；强制每轮 checkpoint 的规则只保留在用户主动复制到会话中的提示词里。
4. WHEN模型未执行 checkpoint THEN 服务端 SHALL 不宣称已自动持久化；第一版不修改或拦截现有工具，也不具备脱离模型调用的后台自动写入能力。
5. WHEN用户打开任一工作区的 MCP 配置 THEN 页面 SHALL 展示完整会话恢复提示词和一键复制按钮，并提供复制成功或失败反馈。

### FR-9: ChatGPT 工具目录升级提示

**优先级:** Must
**用户故事:** 作为插件用户，我希望升级服务端后能明确知道如何让 ChatGPT 重新读取工具 Schema，以便不再误以为修改版本号就会自动刷新。

#### 验收标准（EARS）

1. WHEN MCP 服务端版本升级但工具通知通道未实现 THEN 系统 SHALL 不得声明 `capabilities.tools.listChanged=true`。
2. WHEN用户查看工作区的 ChatGPT 会话提示区域 THEN 页面 SHALL 明确说明 ChatGPT 不会依据服务端版本号自动刷新工具。
3. WHEN用户需要加载新工具目录 THEN 页面 SHALL 提供 ChatGPT 连接器设置入口，并要求重新配置连接后新开会话。
4. WHEN未来实现 `notifications/tools/list_changed` 的可达通知通道 THEN 系统 MAY 将 `listChanged` 改为 `true`，但必须有协议级集成测试证明通知能够到达客户端。

---

## 非功能需求

- **NFR-1（兼容性）**: 支持 Windows 10+、macOS 12+、Linux x86_64；相同测试向量在三平台产生等价 JSON 结果。
- **NFR-2（安全）**: 所有历史路径限定在当前工作区；不执行 Shell、不删除历史、不自动提交 Git；服务器端不信任模型输入。
- **NFR-3（一致性）**: 同一进程和跨进程并发 bootstrap 不得分配重复编号；写入失败后原文件保持完整。
- **NFR-4（性能）**: 100 个、总计不超过 10 MiB 的历史文件 bootstrap 在本地 SSD 上应在 2 秒内完成。
- **NFR-5（可维护性）**: 历史模块按模型、Markdown、存储和用例拆分，单文件不超过 500 行；新增核心逻辑测试覆盖率不低于 80%。

---

## 依赖关系

- [OpenAI Apps SDK 官方变更记录](https://developers.openai.com/apps-sdk/changelog)（2026-01-15）：ChatGPT 工具调用会携带 `_meta["openai/session"]`，该匿名 conversation id 可用于关联同一 ChatGPT 会话内的请求。
- [OpenAI tunnel-client 连接器文档](https://github.com/openai/tunnel-client/blob/master/docs/connectors.md)：连接器侧 MCP 端点为 POST JSON-RPC；通知只会在进行中的流式请求里回传，普通无状态 JSON 响应不构成长连接通知通道。
- [OpenAI tunnel-client 用户指南](https://github.com/openai/tunnel-client/blob/master/docs/end-user-guide.md)：ChatGPT 连接器配置入口为 `https://chatgpt.com/#settings/Connectors`。
- 现有 `src-tauri/src/mcp/server.rs`：读取 MCP `tools/call` 的 `_meta`。
- 现有 `src-tauri/src/tools/registry.rs`：注册工具及 Schema。
- 现有 `src-tauri/src/tools/dispatch.rs`：唯一执行入口。
- 现有 `Workspace`：工作区路径边界。
- 新增跨平台文件锁依赖 `fs2`；Windows 原子替换复用 `windows` crate 的文件系统 API。
