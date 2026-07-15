# 任务清单：GitHub README 展示

## 交付物清单

- **预计新建文件数**：2 个截图文件、3 个规格文件。
- **预计修改文件数**：2 个 README 文件。
- **预计新增/修改函数数**：0 个。
- **交付物**：`README.md`、`README.en.md`、`docs/images/workspace-overview.png`、`docs/images/workspace-connection.png`。

## 任务列表

## 阶段 1：准备真实素材

- [x] 1.1 审核当前 README 和 v0.1.6 客户端界面，确定不含敏感数据的截图页面
  - **证据块**：当前 README 以能力与开发说明为主，仓库中仅有应用图标，没有产品截图。
  - **涉及文件**：`README.md`、`README.en.md`（只读）；客户端界面（只读）。
  - _需求：FR-1, FR-2_ ｜ _设计：信息架构、决策 2_

- [x] 1.2 捕获并审查两张真实桌面客户端截图，保证文字清晰且无密钥
  - **证据块**：`src/routes/workspace/[id]/+page.svelte` 包含服务、认证、隧道、日志和健康检查区域。
  - **涉及文件**：`docs/images/workspace-overview.png`、`docs/images/workspace-connection.png`。
  - _需求：FR-2_ ｜ _设计：技术方案、决策 2_

## 阶段 2：重写中英文 README

- [x] 2.1 重写中文 README，首屏展示价值、截图、下载和五分钟接入流程
  - **证据块**：现有 `README.md` 首屏直接进入实现背景，缺少 Release 下载按钮、产品截图和分步首次连接。
  - **涉及文件**：`README.md`，预算 220 行以内。
  - _需求：FR-1, FR-2, FR-3, FR-4_ ｜ _设计：信息架构_

- [x] 2.2 同步英文 README 的结构、事实和截图
  - **证据块**：现有 `README.en.md` 与中文结构相近，但同样缺少截图和用户快速开始。
  - **涉及文件**：`README.en.md`，预算 220 行以内。
  - _需求：FR-5_ ｜ _设计：决策 1_

## 阶段 3：验收与发布

- [x] 3.1 核验链接、图片、敏感信息、中英文一致性和 Markdown 空白
  - **证据块**：GitHub README 使用仓库相对路径渲染图片；Release 地址固定为仓库 Releases 页面。
  - **涉及文件**：全部交付物。
  - _需求：FR-1 至 FR-5_ ｜ _设计：测试策略_

## 检查点

- [x] 阶段 1：两张截图均来自真实客户端且无敏感信息。
- [x] 阶段 2：中英文 README 具备同等完整章节。
- [x] 阶段 3：图片与链接可用，`git diff --check` 通过。

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
| --- | --- | --- | --- |
| FR-1 | 信息架构、决策 1 | 1.1, 2.1 | 已完成 |
| FR-2 | 技术方案、决策 2 | 1.2, 2.1 | 已完成 |
| FR-3 | 信息架构 | 2.1, 2.2 | 已完成 |
| FR-4 | 决策 3 | 2.1, 2.2 | 已完成 |
| FR-5 | 决策 1 | 2.2, 3.1 | 已完成 |

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
| --- | --- | --- | --- |
| `README.md` | 修改 | 220 行以内 | 中文 GitHub 首页 |
| `README.en.md` | 修改 | 220 行以内 | 英文 GitHub 首页 |
| `docs/images/workspace-overview.png` | 新建 | 二进制 | 工作区总览截图 |
| `docs/images/workspace-connection.png` | 新建 | 二进制 | 服务连接截图 |
