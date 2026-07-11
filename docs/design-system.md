# 设计系统：Coding Tools MCP Desktop

> 2026 开发者工具审美 — 克制、精致、有呼吸感

## 设计方向

**风格名称**: Refined Utilitarian（精炼实用主义）

**参考产品**: Linear、Raycast、Vercel Dashboard

**核心感受**: 专业、安静、可信。用户打开应用应感到「这是一个懂开发者的工具」，而不是「又一个后台管理系统」。

## 与旧版 PySide6 的对比

| 维度 | 旧版 | 新版 |
|------|------|------|
| 布局 | 左右分栏 + 表单堆叠 | Workspace 卡片 + 详情画布 |
| 色彩 | 浅灰底 + 黑色按钮 | 深色默认 + 单一靛蓝强调色 |
| 状态 | 文字描述 | 脉冲状态灯 + 颜色编码 |
| 字体 | Segoe UI 系统默认 | Plus Jakarta Sans + JetBrains Mono |
| 信息密度 | 高（四宫格表单） | 低（渐进披露，主操作突出） |

## 色彩策略

- **默认深色主题**，跟随系统可切换浅色
- **单一强调色**（靛蓝 indigo），用于主 CTA 和选中态
- **状态色独立**：运行绿、启动黄、停止灰、错误红
- **禁止**：紫蓝渐变、米色底、多强调色混用

## 字体

| 用途 | 字体 | 说明 |
|------|------|------|
| UI 正文/标题 | Plus Jakarta Sans | 现代几何无衬线，非 Inter |
| 代码/Endpoint | JetBrains Mono | 路径、URL、日志 |

## 核心 Token

详见 [`design-system.json`](./design-system.json)

## 文档索引

| 文档 | 内容 |
|------|------|
| [设计原则](./design-guidelines/01-principles.md) | 价值观与决策指导 |
| [交互规范](./design-guidelines/02-interaction.md) | 八态、动效、反馈 |
| [布局规范](./design-guidelines/03-layout.md) | 页面结构、栅格、组件层级 |
| [技术配置](./design-guidelines/04-config.md) | Tailwind + Svelte 实现 |
| [UI 规格](../specs/rust-desktop-client/ui-design.md) | 页面线框与组件清单 |

## 交付检查清单

- [ ] 无 Inter / Roboto / Open Sans 默认字体
- [ ] 无 AI 紫蓝渐变
- [ ] 卡片圆角 ≤ 16px
- [ ] 交互元素八态齐全
- [ ] 对比度正文 ≥ 4.5:1
- [ ] prefers-reduced-motion 已处理
- [ ] 图标使用 Lucide SVG，不用 emoji
- [ ] 中文界面文案自然，无「赋能」「一站式」

---
*生成时间: 2026-07-10*
*工具: start_ui + ui_design_system（已针对开发者工具定制）*
