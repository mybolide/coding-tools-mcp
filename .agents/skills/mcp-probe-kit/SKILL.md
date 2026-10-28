---
name: mcp-probe-kit
description: >-
  在已配置 mcp-probe-kit 的项目中，于新功能、Bug、UI、重构或提交前读取；区分独立能力与完整交付编排，汇总当前对话构造完整参数，并在不确定首工具时提供 workflow 兜底。完整新功能由 start_feature 选择 flat 或 parent-child Spec；Skill 不承担中央意图识别，start_* 只组合当前场景实际需要的能力。
mcp-probe-kit-version: "4.1.1"
---

# MCP 调用时机 — mcp-probe-kit

> 本 Skill 负责：**什么情况直接调用独立能力，什么情况使用完整交付编排，以及调用前如何构造完整参数**。不是中央意图识别器。
> 由 mcp-probe-kit 自动安装；支持 MCP 的 Agent 客户端可从 `.agents/skills/` 加载。

## 总规则

1. **先判断目标**：明确单项能力直接调用对应工具；需要从分析到验证完整交付时才调用 `start_*`
2. **独立能力不是必须被编排**：`code_insight`、`fix_bug`、`gentest`、`code_review`、Memory、History Session 等均可直接调用
3. **新对话默认先 `history_session_bootstrap`**（History 开启时）：传 `initial_user_input`；无宿主 key 时服务器铸 handle，后续必须回传 `session_key`（漏传开新档、不合并）；每轮结束前 `history_session_checkpoint`；跨会话用 search/read。关闭：`MCP_HISTORY_SESSION=0|false|off`
4. **只有拿不准该调用哪个工具时**才调用 `workflow`。`workflow` 是兜底选择指南，不做自然语言意图识别；默认 `scenario=auto` 不会根据 `intent` 猜 `firstTool`。Agent 阅读指南和 tool descriptions 后自行判断，缺关键事实时再澄清用户
5. `start_*` 只组合当前场景实际需要的能力；按返回的 Delegated Plan 逐步执行，不要额外塞入无关工具
6. 在写代码或改文件前，先完成当前目标真正需要的理解、规格或根因步骤

## 执行通道与自动降级

### 首选：原生 MCP

如果当前 Agent 可以直接调用 `workflow`、`start_feature`、`start_bugfix`、`start_ui`、`plan_heartbeat`、`resume_plan`、`converge` 等工具，优先使用原生 MCP。

### 降级：项目 CLI

如果 MCP 面板已连接，但当前 Agent 会话看不到上述 MCP 工具，不要放弃当前目标所需的 mcp-probe-kit 能力，也不要要求用户安装。直接通过终端调用项目内版本锁定启动器：

Windows（PowerShell / CMD，优先使用不受脚本执行策略影响的 CMD 启动器）：

```powershell
& ./.mcp-probe-kit/bin/probe.cmd exec <tool> --stdin
```

macOS / Linux：

```bash
./.mcp-probe-kit/bin/probe exec <tool> --stdin
```

CLI 返回 JSON；读取 `structuredContent`、`content` 和 `isError`，继续执行与原生 MCP 相同的工具链。

CLI 降级通道不会继承 IDE `mcp.json` 里的 `env`。若需 Memory 等能力，在项目根复制 `.mcp-probe-kit/local.env.example` 为 `.mcp-probe-kit/local.env` 并填写 `MEMORY_*`；已有 shell 环境变量优先。History Session 默认开启；可用 `MCP_HISTORY_SESSION=0|false|off` 关闭，并用 `MCP_HISTORY_MAX_LINES` / `MCP_HISTORY_MAX_BYTES` 调整软轮转阈值。

### 启动器缺失时自修复

如果 `.mcp-probe-kit/bin/probe.*` 不存在，由 Agent 自行执行：

Windows：

```powershell
npx.cmd --yes mcp-probe-kit@4.1.1 install-agent --project-root .
```

macOS / Linux：

```bash
npx --yes mcp-probe-kit@4.1.1 install-agent --project-root .
```

安装后立即改用项目内启动器。不得使用 `@latest` 或 `@next`，不得全局安装，也不得修改用户项目的 `package.json`。Skill、CLI 启动器和 MCP 必须保持同一精确版本 `4.1.1`。

### 未发布精确版本与本地入口

`runtime.json` 的 `packageSpec` 即使尚未出现在 npm registry，也必须与 MCP / Skill / AGENTS.md 保持同一精确版本 `4.1.1`。禁止把启动器改成 `@latest` 或 `@next`，也禁止为了“能 npx 成功”而改钉已发布的旧版本。

启动器按以下顺序解析 CLI 入口，失败则立即退出并打印路径，不得长时间重试错误版本：

1. `MCP_PROBE_LOCAL_ENTRY`（指向本机 `build/index.js`）
2. 源码仓库检出：项目根 `build/index.js`（仅当 `package.json` 的 name 为 `mcp-probe-kit` 且 version 与 pin 一致）
3. `.mcp-probe-kit/local-verify/node_modules/mcp-probe-kit/build/index.js`
4. `node_modules/mcp-probe-kit/build/index.js`
5. `npx --yes mcp-probe-kit@4.1.1`（`NPM_CONFIG_FETCH_RETRIES=0`）

若自修复 `install-agent` 遇到 ETARGET，改用本地入口，例如：

```powershell
node ./build/index.js install-agent --project-root .
```

### GitNexus 托管运行时

GitNexus 不打包进 mcp-probe-kit 主包，也不做全局安装。优先使用用户显式配置或系统已有的兼容 CLI；否则使用 mcp-probe-kit 管理的精确版本 Sidecar。

如果工具结果包含 `managed_install_required`，Agent 不要让用户手动安装，自动执行一次：

Windows：

```powershell
& ./.mcp-probe-kit/bin/probe.cmd doctor gitnexus --install
```

macOS / Linux：

```bash
./.mcp-probe-kit/bin/probe doctor gitnexus --install
```

安装完成后重试原工具。安装失败或超时则保留降级结果继续，不得阻塞主工作流。可通过 `MCP_GITNEXUS_MODE=system|managed|off` 控制策略。

---

## History Session（默认开启）读写规则

档案目录：`docs/history-session/`。默认开启；关闭：`MCP_HISTORY_SESSION=0|false|off`。与 Memory/Qdrant **分离**。
档案是**项目本地交接材料**，应写入仓库 `.gitignore`（忽略整个 `docs/history-session/`），**不要提交**到 Git。

### 何时写

1. **新对话开始、回答用户第一问之前**：调用 `history_session_bootstrap`  
   - 传 `initial_user_input=<用户首条原话>`  
   - 无宿主 session 时**第一次**可省略 `session_key`：服务器会铸 `hs_<uuid>` 并开新档（不会写入共享 `project-active`）  
   - **禁止**自行编造 UUID；同一对话后续轮次必须回传上次 bootstrap 的 `session_key`，漏传会再开一本而不是合并  
   - 需要刻意开新档时用 `mode=fresh`
2. **每轮任务结束、给出最终回复之前**：调用 `history_session_checkpoint`  
   - 必须回传 bootstrap（或上轮轮转）返回的 `session_key` + `expected_path`  
   - 必须带本轮 `raw_user_input`（服务端读不到未作为参数传入的聊天原文）  
   - 超 `MCP_HISTORY_MAX_LINES` / `MCP_HISTORY_MAX_BYTES` 会软轮转；以返回的新 `expected_path` 为准  
   - 必须按下方「检查点最低填写清单」写满，禁止只写空数组交差

### 何时读

1. **bootstrap 的有界 `state` / `continuity`**：先读完即可开工；**不要**一上来全量加载历史  
2. **需要精确旧结论 / 旧用户原话 / 旧决策**：`history_session_search` → `history_session_read`（按 `next_cursor` 分页）  
3. **编号缺口或派生状态异常**：`history_session_validate`（必要时 `repair=true`）

### 检查点最低填写清单（合格标准）

每轮 `history_session_checkpoint` **至少**包含：

| 字段 | 要求 |
|---|---|
| `raw_user_input` | **必填**：本轮用户原话（逐字） |
| `user_intent` | **必填**：一句话意图摘要 |
| `findings` | 本轮关键事实/现象；无则写 `["本轮无新发现"]` |
| `decisions` | 本轮关键决定；无则写 `["本轮无新决定"]` |
| `files_changed` | 改动路径列表；无改文件写 `["本轮未改文件"]` |
| `tests` | 跑过的命令与结果；未跑写 `["本轮未跑测试"]` |
| `runtime_state` | 分支/HEAD/服务状态等关键运行事实；无可写 `["无额外运行状态"]` |
| `remaining_issues` | 未解决问题；无则 `[]` |
| `next_actions` | **必填至少 1 条**：下一会话应做什么 |
| `notes` | 可选；证据路径、截图、报告引用 |

说明：空数组 `[]` 只允许用在「确实没有」的 `remaining_issues`；其他列表字段不要留空糊弄。目标是像旧 ChatGPT 厚档案那样**信息可交接**，但结构落在检查点 JSON，而不是把全量摘要堆进 Markdown 文首。

### 与 Plan / Memory 的边界

| 层 | 用途 | 典型工具 |
|---|---|---|
| History Session | 本项目会话交接、用户原话、检查点 | `history_session_*` |
| Delegated Plan | 可恢复步骤与证据收敛 | `plan_heartbeat` / `resume_plan` / `converge` |
| Memory | 跨仓库可复用经验（需 MEMORY_*） | `search_memory` / `memorize_asset` |

用户只说“继续”且存在未完成 Plan → **先** `resume_plan`；History bootstrap 用于会话档案交接，不替代 Plan 恢复。

---

## 参数构造纪律

- 用户只说“继续 / 开始 / 往下做”且存在最近的 Delegated Plan 或已知 plan_id 时，直接调用 `resume_plan` 恢复检查点并从 nextStepId 继续；`mustContinue=true` 时禁止只汇报恢复结果，必须立即执行 nextStep/nextTool 并逐步 heartbeat；plan_id 丢失时只传 project_root，由工具自动选择最近的 active/blocked Plan；只有不存在可恢复 Plan 时，才结合当前对话、已有 Spec 和用户已确认决定下一工具。不要先调用 `workflow` 做意图识别。
- 先判断当前目标是一个明确的单项能力，还是需要完整交付；单项能力直接调用对应工具，完整交付才使用 `start_*`。
- 完整新功能交付调用 `start_feature`，并传 `description=<完整范围摘要>`、`spec_layout=auto` 和明确的 `project_root`；让编排器决定 flat 或 parent-child。
- 跨模块、多阶段、大版本或架构升级不得直接调用 `add_feature`；只有布局和 `subspecs` 已明确时，才按 `start_feature` 返回的 plan 调用它。
- 工具参数必须表达当前任务事实，不要只复制用户最后一条消息；当前项目代码和已落盘 Spec 优先于历史记忆。
- 只有需要持续状态、跨会话恢复或正式交付的 Delegated Plan 才要求 `plan_heartbeat`；单次只读分析不强制创建 Plan。
- 拿到托管 Delegated Plan 后首次调用 `plan_heartbeat` 时附完整 plan；每完成、跳过或阻塞步骤后更新检查点。
- History Session 默认开启：新对话先 `history_session_bootstrap`（传 `initial_user_input`；无宿主 session 可不传 `session_key`，落到项目当前档）；每轮最终回复前 `history_session_checkpoint`（回传 `session_key`/`expected_path`，必须带 `raw_user_input`）。检查点须按 Skill「最低填写清单」写满 findings/decisions/files_changed/tests/runtime_state/next_actions，禁止空数组交差。精确旧上下文用 `history_session_search` → `history_session_read`，不要把全量历史塞进 bootstrap。

---

## 工具选择速查（由 Agent 判断）

| 用户说什么 / 什么情况 | 第一个 MCP |
|----------------------|------------|
| 完整交付新功能、功能增强或跨模块能力 | `start_feature` |
| 完整修复 Bug，并完成回归、审查和收敛 | `start_bugfix` |
| 只做 Bug 根因分析或使用 SRC-8 方法 | `fix_bug` |
| 架构评估、架构设计、数据所有权、迁移回滚或架构漂移 | `architecture` |
| 完整交付页面、组件或 UI 交互 | `start_ui` |
| 只查 UI 模式或生成设计系统 | `ui_search / ui_design_system` |
| 不熟代码、找入口、调用链、依赖或影响面 | `code_insight` |
| 只生成测试策略、测试设计或候选用例 | `gentest` |
| 只审查指定代码、真实 diff 或 PR | `code_review` |
| 新成员上手、熟悉仓库和开发上下文 | `start_onboard` |
| 产品方案、PRD、目标用户、范围或原型方向 | `start_product` |
| 需要有界多轮自主迭代并逐轮留证 | `start_ralph` |
| 缺 AGENTS.md、项目上下文或图谱索引 | `init_project_context` |
| 全新空仓库需要初始化项目结构 | `init_project` |
| 写 commit message | `gencommit` |
| 重构、整理代码或制定重构步骤 | `refactor` |
| 估算工时、故事点、排期或风险 | `estimate` |
| 校验已有规格是否完整 | `check_spec` |
| 查询历史踩坑、已保存方案或可复用经验 | `search_memory` |
| 需求本身不清楚，缺关键事实，需要向用户提问 | `ask_user / interview` |
| 工作报告、周报或 Git 工作汇总 | `git_work_report` |
| 用户只说继续/开始/往下做且可能存在未完成 Plan | `resume_plan` |
| 新对话开始需要会话交接，或跨会话找回历史结论/用户原话 | `history_session_bootstrap` |

---

## 全工具：何时调用

### 完整交付编排 `start_*`（按需使用）

| MCP | 何时调用 |
|-----|----------|
| `start_feature` | 需要从需求、规格、实施、测试、审查到收敛完成**完整新功能交付**时使用；先把当前对话确认的完整范围汇总到 description，默认 `spec_layout=auto`，复杂多模块需求使用 parent-child；仅做规格、影响分析或测试时可直接调用对应能力 |
| `start_bugfix` | 需要从现象、SRC-8 真因、修复、回归、审查到收敛完成**完整 Bug 交付**时使用；只做根因分析时直接调用 `fix_bug` |
| `start_ui` | 需要从视觉方向、页面结构、实现、桌面/移动验收到正式收敛完成**完整 UI 交付**时使用；只查模式或生成设计系统时直接调用 UI 能力 |
| `start_onboard` | **新成员 / 新仓库**快速建立心智模型 |
| `start_product` | 从 0 做**产品方案**（PRD、原型思路） |
| `start_ralph` | 需要**有界多轮迭代**、每轮 Heartbeat/测试/Diff 证据和最终 Converge 的完整长任务时；不用于后台无人值守循环 |

### 可选首工具路由

| MCP | 何时调用 |
|-----|----------|
| `workflow` | Agent 阅读 Skill 和工具 description 后仍**不确定该调用哪个工具**时使用；`auto` 只返回选择指南，不做自然语言意图识别、不替 Agent 猜 firstTool。Agent 已明确场景时可显式传 scenario 获取该场景的确定性流程说明 |

### 项目与规格

| MCP | 何时调用 |
|-----|----------|
| `init_project_context` | 没有 **AGENTS.md**、`docs/project-context/`、图谱索引；大改前缺上下文 |
| `init_project` | **空目录**需要初始化项目结构 |
| `add_feature` | 仅在规格布局已确定时生成 `docs/specs/<feature>/`；复杂需求不得把它当首个入口，通常由 `start_feature` 的 plan 触发 |
| `check_spec` | 规格写完后、**写实现代码前**；或 Bug 修完要过规格闸门 |
| `estimate` | 需要**故事点 / 工时 / 风险**评估（通常在 `add_feature` 之后） |

### 代码分析（可直接调，不必等 start_*）

| MCP | 何时调用 |
|-----|----------|
| `code_insight` | 读不懂代码、找入口、看**调用链 / 影响面**；大重构前；`mode=impact` 评估改动范围 |
| `architecture` | 需要评估或设计模块边界、依赖方向、数据所有权、公共契约、迁移回滚或实施漂移时直接调用；支持 `assess|design|validate|drift`，完整功能、Bug 和重构流程只按需组合它 |
| `fix_bug` | 需要独立执行 **SRC-8 根因分析与修复方法**时直接调用；完整 Bug 交付中由 `start_bugfix` 编排或展开同一方法核心 |
| `gentest` | 需要**补测试 / 回归用例**（Bug 修复后、功能完成后） |
| `code_review` | 用户要审查指定代码、真实 Git diff，或核验 Plan 声明范围、测试、公共契约、架构漂移与当前 revision 是否一致 |
| `refactor` | 需要**分步重构计划**；范围大时先 `code_insight` |

### Git

| MCP | 何时调用 |
|-----|----------|
| `gencommit` | 变更完成，需要**规范 commit message** |
| `git_work_report` | 需要基于 git 历史的**工作报告 / 周报** |

### UI 独立能力（可直接调用，也可由 `start_ui` 组合）

| MCP | 何时调用 |
|-----|----------|
| `ui_design_system` | 需要**设计 token / 组件规范** |
| `ui_search` | 需要搜 **UI/UX 模板、模式** |
| `sync_ui_data` | UI 内嵌数据过期，需要**同步缓存** |

### 记忆（需 MEMORY 已配置）

| MCP | 何时调用 |
|-----|----------|
| `search_memory` | 主动查**历史经验**；默认只返回 active，审计失效记录时显式 `include_inactive=true`，并结合 ranking 解释核对证据与适用边界 |
| `read_memory_asset` | `search_memory` 命中后需要**读全文** |
| `memorize_asset` | 托管交付流程在 **converge passed=true** 后沉淀 MemoryCandidate；用户明确进行独立记忆管理时也可直接调用。沉淀跨项目知识：**禁止** source_project / source_path / file_path；summary 写关键词+结论，content 抽象化。默认拒绝同身份冲突，确认替代时用 `conflict_policy=supersede`，确需并行结论时显式 `allow_parallel` |
| `update_memory_asset` | 修正已有记忆、撤回错误结论或建立 supersede 关系；历史关系不可清除，retracted/负面结论必须保留 evidence |
| `delete_memory_asset` | 硬删除未关联的错误/重复/无价值资产（需 `confirm: true`）；参与 supersede 链的资产只能用 update_memory_asset 撤回 |
| `scan_and_extract_patterns` | 从代码库**批量提取**可复用模式并建议沉淀 |

### 会话历史档案（默认开启；MCP_HISTORY_SESSION=0|false|off 关闭）

| MCP | 何时调用 |
|-----|----------|
| `history_session_bootstrap` | 新对话开始时**先 bootstrap**（可传 initial_user_input；无宿主 session 可不传 session_key，落到项目当前档）；返回有界状态与 checkpoint_policy；档案在 docs/history-session/（应 gitignore，勿提交） |
| `history_session_checkpoint` | 每轮最终回复前写入检查点；必须回传 session_key/expected_path + raw_user_input/user_intent；按 Skill 最低清单填 findings/decisions/files_changed/tests/runtime_state/next_actions，禁止空数组交差；超行数/字节会软轮转 |
| `history_session_validate` | 编号缺口、重复 key 或派生状态损坏时校验；`repair=true` 可重建 state/manifest |
| `history_session_search` | 跨会话需要定位旧档时按关键词搜索；再配合 history_session_read 取原文 |
| `history_session_read` | `history_session_search` 命中后按 number/path **分页读取**无损 Markdown 原文 |

### 长任务状态、恢复与正式收敛（按需）

| MCP | 何时调用 |
|-----|----------|
| `plan_heartbeat` | 执行需要持续状态、跨会话恢复或正式交付的 Delegated Plan 时记录步骤、证据、作用域、产物、候选、验收结果、运行证据和 revision；首次调用附完整 plan，单次只读能力不强制使用 |
| `resume_plan` | 会话中断、重启、切换 Agent 或用户只说继续时恢复下一可执行步骤；已知 plan_id 时精确恢复，未知时省略 plan_id 自动选择最近 active/blocked Plan；found=true 且 mustContinue=true 后必须立即执行 nextStep，禁止只汇报恢复结果后停止 |
| `converge` | 托管交付实现与验证完成后，按 Plan 自己声明的 requiredEvidenceKinds（例如需求、测试、审查）、qualityGates、步骤和未决项进行收敛；通过后才允许该流程正式沉淀记忆。单次只读分析和独立记忆管理不强制进入收敛 |

### 交互

| MCP | 何时调用 |
|-----|----------|
| `ask_user` | 目标模糊、缺关键信息，需要**向用户提问** |
| `interview` | 需要结构化**需求访谈** |

---

## 常见链路（只是调用顺序参考）

**新功能**：`start_feature → plan_heartbeat → add_feature → check_spec（通过）→ 写代码 → gentest → code_review → converge（通过）→ memorize_asset（可选）→ gencommit`

**完整修 Bug**：`start_bugfix（编排并展开 fix_bug / SRC-8）→ plan_heartbeat → 改代码 → gentest → 跑测试 → code_review → converge（通过）→ memorize_asset（成功或负面记忆）`

**只做根因分析**：`fix_bug；不要求先 start_bugfix，不要求建立完整 Plan`

**只理解代码**：`code_insight；若后续转为完整交付，再进入对应 start_*`

**只设计测试**：`gentest；测试候选生成后仍需由 Agent 真实落盘和执行`

**独立架构工作**：`architecture assess → architecture design（按需）→ validate / drift；不要求先 start_*`

**大重构**：`code_insight（impact）→ refactor → plan_heartbeat → gentest → code_review → converge`

**会话中断后继续**：`resume_plan → 执行 nextStepId → plan_heartbeat → 最终 converge`

**会话历史交接**：`history_session_bootstrap → …工作… → history_session_checkpoint；跨会话 search → read`

---

## 不要

- 有对应 MCP 却**直接大段写实现**
- 把 `workflow` 当作所有任务的强制入口
- 把 `start_*` 当作所有原子能力的上级，导致单项分析也被强制套入完整流程
- 把用户的“继续 / 开始 / 往下做”交给 `workflow` 做意图识别，或原样当作 `start_feature.description`
- 大型跨模块需求绕过 `start_feature` 直接手写单体 Spec
- `check_spec` **未通过**就写功能代码
- 长流程执行步骤后**不** `plan_heartbeat`，导致中断后无法恢复
- 托管交付流程在 `converge` 未通过时就把候选经验正式写入 `memorize_asset`
- `delete_memory_asset` 不带 `confirm: true`
- History 开启时跳过 `history_session_bootstrap` / 最终回复前不 `history_session_checkpoint`
- 自行编造 `session_key`（只回传服务器铸出的 handle；漏传会开新档而不是合并）
- 把 History Session 当成 Memory/Qdrant；跨仓库可复用经验仍走 `search_memory` / `memorize_asset`
- 忽略 bootstrap/轮转后返回的新 `expected_path`，继续用旧路径 checkpoint
- `history_session_checkpoint` 只填空数组或省略 `raw_user_input`/`user_intent`/`next_actions`（不满足最低填写清单）

---

*mcp-probe-kit 按版本自动同步（当前 `4.1.1`）。路径：`.agents/skills/mcp-probe-kit/SKILL.md`*

