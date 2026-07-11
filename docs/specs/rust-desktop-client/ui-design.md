# UI 设计规格：rust-desktop-client

> 配合 `start_ui` 流程生成，基于 2026 开发者工具审美定制

## 设计方向

**Refined Utilitarian** — 参考 Linear / Raycast / Vercel Dashboard 的克制高级感。

完整设计系统见 [`docs/design-system.md`](../../design-system.md)。

## 页面清单

| 页面 | 路由 | 说明 |
|------|------|------|
| 首页 | `/` | Workspace 卡片网格 + 空状态引导 |
| 详情 | `/workspace/[id]` | 控制面板：连接、健康、配置、日志 |
| 添加 | `/workspace/new` | 目录选择 + 名称输入（可用 modal 替代） |

## 组件清单

| 组件 | 文件 | 职责 |
|------|------|------|
| AppShell | `AppShell.svelte` | 侧边栏 + 主画布 + 标题栏 |
| WorkspaceCard | `WorkspaceCard.svelte` | 首页卡片 |
| StatusOrb | `StatusOrb.svelte` | 运行状态灯 |
| Badge | `Badge.svelte` | 隧道类型/状态标签 |
| ConnectionPanel | `ConnectionPanel.svelte` | 本地/公网 endpoint 展示与复制 |
| HealthPanel | `HealthPanel.svelte` | 健康检查项列表 |
| ConfigPanel | `ConfigPanel.svelte` | 折叠式配置（端口/认证/隧道） |
| LogViewer | `LogViewer.svelte` | 日志流 + 来源切换 |
| EmptyState | `EmptyState.svelte` | 无 Workspace 引导 |
| ThemeToggle | `ThemeToggle.svelte` | 深色/浅色切换 |
| CopyButton | `CopyButton.svelte` | 复制 + 反馈动画 |
| PrimaryButton | `PrimaryButton.svelte` | 主操作（启动/停止） |

## 首页线框

```
┌─ Sidebar ─────┬─ Main ─────────────────────────────────────┐
│ ◆ Coding    │                                              │
│   Tools MCP │  工作区                          [+ 添加]    │
│             │  管理你的 MCP 运行时与远程连接                  │
│ ┌─────────┐ │                                              │
│ │● proj-a │ │  ┌─────────────┐  ┌─────────────┐            │
│ └─────────┘ │  │ ● proj-a    │  │ ○ proj-b    │            │
│ ┌─────────┐ │  │ ~/code/a    │  │ ~/code/b    │            │
│ │○ proj-b │ │  │ FRP · 运行中 │  │ 未启动       │            │
│ └─────────┘ │  │ ctp.ex...   │  │             │            │
│             │  └─────────────┘  └─────────────┘            │
│ [+ 添加]    │                                              │
└─────────────┴──────────────────────────────────────────────┘
```

## 详情页线框

```
┌─ Sidebar ─────┬─ Main ─────────────────────────────────────┐
│ (同上)       │  ← 返回  proj-a  ● 运行中   [停止] [复制地址]  │
│             │                                              │
│             │  ┌─ 连接 ─────────────────────────────────┐  │
│             │  │ 本地  http://127.0.0.1:28766/mcp  [复制] │  │
│             │  │ 公网  https://ctp.example.com/mcp [复制] │  │
│             │  └────────────────────────────────────────┘  │
│             │                                              │
│             │  ┌─ 健康检查 ────┐  ┌─ 配置 ──────────────┐  │
│             │  │ ✓ 本地 /mcp   │  │ ▼ 运行时             │  │
│             │  │ ✓ 公网 /mcp   │  │   端口 28766         │  │
│             │  │ ✓ OAuth 元数据 │  │ ▼ 认证               │  │
│             │  └───────────────┘  │   OAuth              │  │
│             │                     │ ▼ 隧道               │  │
│             │                     │   FRP                │  │
│             │                     └─────────────────────┘  │
│             │                                              │
│             │  ┌─ 日志 ─────────────────────────────────┐  │
│             │  │ [MCP] [隧道]              [自动滚动 ✓]  │  │
│             │  │ > Server listening on :28766           │  │
│             │  └────────────────────────────────────────┘  │
└─────────────┴──────────────────────────────────────────────┘
```

## 视觉关键词

- 深色默认，浅色可选
- 1px 细边框，少用阴影
- 大量留白（卡片间距 16px，区块间距 24px）
- 单一靛蓝强调色
- 状态灯脉冲动画
- 等宽字体展示路径和 URL
- Lucide 线性图标

## 与 FR 的映射

| FR | UI 体现 |
|----|---------|
| FR-1 | 首页卡片网格 |
| FR-2 | Sidebar 列表 + 添加流程 |
| FR-3 | 详情页启动/停止按钮 + StatusOrb |
| FR-5 | ConfigPanel 隧道配置 |
| FR-6 | ConfigPanel 认证配置 |
| FR-8 | HealthPanel |
| FR-9 | LogViewer |
| FR-10 | CopyButton + ConnectionPanel |

## 实现优先级

1. AppShell + ThemeToggle + 深色主题 CSS 变量
2. WorkspaceCard + StatusOrb + EmptyState
3. 详情页 ConnectionPanel + PrimaryButton
4. HealthPanel + LogViewer
5. ConfigPanel 折叠面板
6. 浅色主题 + 动效打磨

---
*关联: [design.md](./design.md) | [tasks.md](./tasks.md)*
