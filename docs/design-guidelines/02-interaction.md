# 交互规范

## 交互八态

每个可交互元素必须设计以下状态：

| 状态 | 视觉表现 | 时长 |
|------|----------|------|
| Default | 基础样式 | — |
| Hover | 背景提亮 / 边框加深 | 200ms |
| Focus | 2px accent outline, offset 2px | instant |
| Active | 轻微 scale(0.98) | 100ms |
| Disabled | opacity 0.5, cursor not-allowed | — |
| Loading | spinner + 文案「启动中…」 | — |
| Error | 红色边框 + 错误文案 | — |
| Success | 绿色勾选 + toast「已复制」 | 2s 自动消失 |

## Runtime 状态反馈

### 状态灯（StatusOrb 组件）

```
运行中  → 绿色圆点 + 脉冲动画（2s 周期）
启动中  → 黄色圆点 + 旋转动画
已停止  → 灰色实心圆点
错误    → 红色圆点 + 无动画
```

### 主操作按钮

| Runtime 状态 | 主按钮 | 次按钮 |
|-------------|--------|--------|
| Stopped | 「启动」accent 实心 | — |
| Starting | 「启动中…」disabled + spinner | — |
| Running | 「停止」destructive outline | 「复制地址」secondary |
| Error | 「重试」accent 实心 | 「查看日志」secondary |

## 动效规范

```css
--motion-fast: 150ms;
--motion-normal: 200ms;
--motion-slow: 300ms;
--easing: cubic-bezier(0.16, 1, 0.3, 1);
```

- 页面切换：fade + slide-up 12px，200ms
- 卡片 hover：border-color 变化 + translateY(-1px)，200ms
- 状态灯脉冲：opacity 0.6→1.0，2s ease-in-out infinite
- Toast 进入：slide-in from bottom，200ms

### prefers-reduced-motion

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    transition-duration: 0.01ms !important;
  }
}
```

## 复制反馈

点击「复制」后：
1. 按钮文案短暂变为「已复制」（1.5s）
2. 可选 toast 确认
3. 图标从 copy 变为 check

## 空状态

无 Workspace 时展示引导页：
- 居中插图（简约线条风格，非 emoji）
- 标题：「添加你的第一个工作区」
- 副标题：「选择一个项目目录，一键启动 MCP 服务」
- 主 CTA：「选择目录」

---
*返回: [README.md](./README.md)*
