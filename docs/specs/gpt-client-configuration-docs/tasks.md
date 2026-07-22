# 任务清单：gpt-client-configuration-docs

## 概述

补全 README 中从桌面端连接信息到 ChatGPT MCP 插件配置和调用验证的用户路径。

## 交付物清单（Scope-lock）

- **预计新建文件数**：6 个（2 张用户提供的图片、1 张裁剪特写、3 份规格文档）
- **预计修改文件数**：2 个
- **预计新增/修改函数数**：0 个
- **交付物逐项列举**：
  1. `docs/images/gpt-config-1.png`
  2. `docs/images/gpt-config-2.png`
  3. `docs/specs/gpt-client-configuration-docs/requirements.md`
  4. `docs/specs/gpt-client-configuration-docs/design.md`
  5. `docs/specs/gpt-client-configuration-docs/tasks.md`
  6. `README.md`
  7. `README.en.md`
  8. `docs/images/gpt-config-2-detail.png`

## 任务列表

### 阶段 1：准备工作

- [x] 1.1 检查 GPT 配置截图与现有 README 接入章节，确认图片内容和插入位置
  - **证据块**：`README.md` 的“ChatGPT 的两种接入方式”已有 MCP Connector 四步简述；`docs/images/gpt-config-1.png` 展示开发人员模式，`gpt-config-2.png` 展示新建 MCP 插件。
  - **涉及文件**：`README.md`（只读）、两张 PNG（只读）
  - _需求：FR-1、FR-2_ ｜ _设计：信息流、设计决策 1_

### 阶段 2：核心实现

- [x] 2.1 扩写中英文 MCP Connector 教程，补全桌面端准备、GPT 字段映射、授权和验证
  - **证据块**：现有内容只写“创建连接、粘贴地址、完成认证”，没有开发人员模式入口、表单字段和验证方法。
  - **涉及文件**：`README.md`、`README.en.md`，各新增约 50 行，不涉及代码文件行数门禁
  - _需求：FR-1、FR-2_ ｜ _设计：技术方案、信息流_

- [x] 2.2 插入两张真实截图和界面兼容提示，确保图片说明与步骤一一对应
  - **证据块**：图片已经存放于 `docs/images/`，命名与步骤顺序一致。
  - **涉及文件**：`README.md`、`README.en.md`，各包含 2 个相对图片链接
  - _需求：FR-2_ ｜ _设计：设计决策 2_

- [x] 2.3 增加首屏 30 秒连接路径、放大配置截图并补充常见问题表
  - **证据块**：原 README 的完整配置位于中部，第二张原图中的表单较小，首次用户需要额外寻找路径和排障入口。
  - **涉及文件**：`README.md`、`README.en.md`、`docs/images/gpt-config-2-detail.png`
  - _需求：FR-1、FR-2_ ｜ _设计：概述、测试策略_

### 阶段 3：集成验证

- [x] 3.1 对照验收标准检查 README 链路、图片路径、敏感信息和 Markdown diff
  - **证据块**：项目历史 README 模式要求真实截图、相对链接和敏感信息检查。
  - **涉及文件**：`README.md`、`docs/images/gpt-config-1.png`、`docs/images/gpt-config-2.png`
  - _需求：FR-1、FR-2_ ｜ _设计：测试策略_

## 检查点

- [x] 阶段 1 完成后：已确认截图内容和 README 插入位置
- [x] 阶段 2 完成后：教程覆盖准备、配置、认证与验证闭环
- [x] 阶段 3 完成后：路径、敏感信息和 Markdown diff 全部通过检查

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
| --- | --- | --- | --- |
| FR-1 | 信息流、设计决策 1 | 1.1、2.1、2.3、3.1 | 已完成 |
| FR-2 | 技术方案、设计决策 2 | 1.1、2.1、2.2、2.3、3.1 | 已完成 |

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
| --- | --- | --- | --- |
| `README.md` | 修改 | 35 至 50 行 | 增加 ChatGPT MCP 配置闭环教程 |
| `README.en.md` | 修改 | 35 至 50 行 | 增加对应英文教程，保持双语结构一致 |
| `docs/images/gpt-config-1.png` | 新增 | 二进制图片 | 开发人员模式截图 |
| `docs/images/gpt-config-2.png` | 新增 | 二进制图片 | 新建 MCP 插件截图 |
| `docs/images/gpt-config-2-detail.png` | 新增 | 二进制图片 | 从原图确定性裁剪放大的配置表单特写 |
| `docs/specs/gpt-client-configuration-docs/*.md` | 新增 | 约 250 行 | 需求、设计和任务记录 |

## 检查清单

- [x] 交付物清单已填写
- [x] 每条任务标题具体且可验收
- [x] 每条任务含证据块
- [x] 每条任务标注文件与行数预算
- [x] 任务按准备、实现和验证分阶段
- [x] 每条任务回链到需求与设计
- [x] 需求覆盖矩阵无遗漏
- [x] 阶段 3 对照验收标准核验
- [x] 全文无占位内容
