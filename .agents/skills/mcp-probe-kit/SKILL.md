---
name: mcp-probe-kit
description: >-
  将用户意图路由到 mcp-probe-kit MCP 工具（start_feature、start_bugfix、code_insight、workflow、gencommit 等）。在已配置 MCP 且准备写代码前读取；仅说明调哪个 MCP，不是项目研发流程本身。Routes intent to mcp-probe-kit MCP tools; read before coding when MCP is configured.
mcp-probe-kit-version: "3.6.11"
---

# MCP 调用时机 — mcp-probe-kit

> 本 Skill 只回答一件事：**什么情况 → 调哪个 MCP**。不是开发流程剧本。
> 由 mcp-probe-kit 自动安装；支持 MCP 的 Agent 客户端可从 `.agents/skills/` 加载。

## 总规则

1. **先查下表**，有对应 MCP 就先调，再写代码 / 改文件
2. **拿不准** → `workflow`：`{ "intent": "<用户原话>" }`
3. `start_*` 会列出后续该调的 MCP；按返回逐步调用即可

---

## 意图速查（第一个该调的 MCP）

| 用户说什么 / 什么情况 | 第一个 MCP |
|----------------------|------------|
| 新功能、加模块、做需求 | `start_feature` |
| Bug、报错、异常、排查、不生效 | `start_bugfix` |
| 页面、组件、样式、UI、交互 | `start_ui` |
| 不熟代码、架构、调用链、影响面 | `code_insight` |
| 新项目上手、熟悉仓库 | `start_onboard` |
| 产品方案、PRD、原型 | `start_product` |
| 长周期自主迭代（Ralph） | `start_ralph` |
| 缺 AGENTS.md / 项目上下文 | `init_project_context` |
| 全新空仓库脚手架 | `init_project` |
| 写 commit message | `gencommit` |
| 代码评审、安全检查 | `code_review` |
| 重构、整理代码 | `refactor（大改前先 code_insight）` |
| 估算工时、排期 | `estimate` |
| 校验规格是否写全 | `check_spec` |
| 查历史踩坑、可复用经验 | `search_memory` |
| 需求不清楚、要澄清 | `ask_user 或 interview` |
| 工作报告、周报、git 汇总 | `git_work_report` |
| 不确定用哪个 MCP | `workflow` |

---

## 全工具：何时调用

### 编排入口 `start_*`（复杂任务的第一步）

| MCP | 何时调用 |
|-----|----------|
| `start_feature` | 任何**新功能 / 增强**；会先搜记忆，再指引 `add_feature` → `check_spec` → 实现 |
| `start_bugfix` | 任何 **Bug / 报错**；指引 `fix_bug`（真因）→ `gentest` → 测试 |
| `start_ui` | 任何 **UI / 页面 / 组件**；指引设计系统、模板检索、实现约束 |
| `start_onboard` | **新成员 / 新仓库**快速建立心智模型 |
| `start_product` | 从 0 做**产品方案**（PRD、原型思路） |
| `start_ralph` | 需要**多轮自主迭代**、长任务循环时 |

### 路由

| MCP | 何时调用 |
|-----|----------|
| `workflow` | **不确定**该用哪个 MCP；或担心 Agent 跳过 MCP 直接写代码时 |

### 项目与规格

| MCP | 何时调用 |
|-----|----------|
| `init_project_context` | 没有 **AGENTS.md**、`docs/project-context/`、图谱索引；大改前缺上下文 |
| `init_project` | **空目录**需要初始化项目结构 |
| `add_feature` | 需要生成 `docs/specs/<feature>/` 规格（通常由 `start_feature` 触发） |
| `check_spec` | 规格写完后、**写实现代码前**；或 Bug 修完要过规格闸门 |
| `estimate` | 需要**故事点 / 工时 / 风险**评估（通常在 `add_feature` 之后） |

### 代码分析（可直接调，不必等 start_*）

| MCP | 何时调用 |
|-----|----------|
| `code_insight` | 读不懂代码、找入口、看**调用链 / 影响面**；大重构前；`mode=impact` 评估改动范围 |
| `fix_bug` | 需要 **TBP 真因分析**指南（通常由 `start_bugfix` 触发） |
| `gentest` | 需要**补测试 / 回归用例**（Bug 修复后、功能完成后） |
| `code_review` | 用户要**审查**指定文件或 diff（含安全项） |
| `refactor` | 需要**分步重构计划**；范围大时先 `code_insight` |

### Git

| MCP | 何时调用 |
|-----|----------|
| `gencommit` | 变更完成，需要**规范 commit message** |
| `git_work_report` | 需要基于 git 历史的**工作报告 / 周报** |

### UI 子工具（通常由 `start_ui` 串联）

| MCP | 何时调用 |
|-----|----------|
| `ui_design_system` | 需要**设计 token / 组件规范** |
| `ui_search` | 需要搜 **UI/UX 模板、模式** |
| `sync_ui_data` | UI 内嵌数据过期，需要**同步缓存** |

### 记忆（需 MEMORY 已配置）

| MCP | 何时调用 |
|-----|----------|
| `search_memory` | 主动查**历史经验**；`start_*` 未覆盖时补查 |
| `read_memory_asset` | `search_memory` 命中后需要**读全文** |
| `memorize_asset` | Bug **验证通过**后沉淀；有可复用 pattern/component |
| `update_memory_asset` | 修正已有记忆条目 |
| `delete_memory_asset` | 删除错误记忆（需 `confirm: true`） |
| `scan_and_extract_patterns` | 从代码库**批量提取**可复用模式并建议沉淀 |

### 交互

| MCP | 何时调用 |
|-----|----------|
| `ask_user` | 目标模糊、缺关键信息，需要**向用户提问** |
| `interview` | 需要结构化**需求访谈** |

---

## 常见链路（只是调用顺序参考）

**新功能**：`start_feature → add_feature → check_spec（通过）→ 写代码 → gentest → gencommit`

**修 Bug**：`start_bugfix → fix_bug → 改代码 → gentest → 跑测试 → memorize_asset（type=bugfix）`

**不熟代码**：`code_insight → 再 start_feature / start_bugfix`

**大重构**：`code_insight（impact）→ refactor → gentest → code_review`

---

## 不要

- 有对应 MCP 却**直接大段写实现**
- `check_spec` **未通过**就写功能代码
- Bug 修完**不** `memorize_asset`
- `delete_memory_asset` 不带 `confirm: true`

---

*mcp-probe-kit 按版本自动同步（当前 `3.6.11`）。路径：`.agents/skills/mcp-probe-kit/SKILL.md`*

