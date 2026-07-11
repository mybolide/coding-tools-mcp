# 如何编写测试

> 本文档描述 Coding Tools MCP Rust 的测试策略。

## 概述

测试分三层：Rust 单元测试、MCP 合规测试（从旧版移植）、端到端集成测试。

## 测试框架

| 层级 | 框架 | 位置 |
|------|------|------|
| Rust 单元测试 | cargo test | `src-tauri/src/**` 内 `#[cfg(test)]` |
| Rust 集成测试 | cargo test | `src-tauri/tests/` |
| MCP 合规测试 | cargo test | `tests/compliance/` |
| 前端测试 | vitest | `src/**/*.test.ts` |

## MCP 合规测试（核心）

旧版 `old/tests/compliance/` 包含 71 项测试，覆盖：

- MCP 协议契约（initialize, tools/list, tools/call）
- 工具行为 golden test（read_file, apply_patch, exec_command 等）
- 安全边界（路径穿越、敏感环境变量、破坏性命令）
- Schema drift（工具定义与文档一致）
- 端到端场景（bugfix、stdin 交互）

### 移植策略

1. Phase 1：移植 P0 工具的 golden tests
2. Phase 2：移植安全测试和 schema drift
3. Phase 3：移植完整 71 项合规套件

### 合规测试运行

```bash
# 运行全部合规测试
cargo test --test compliance

# 运行单个套件
cargo test --test compliance -- mcp_contract
```

## Rust 单元测试示例

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_profile_endpoint() {
        let profile = WorkspaceProfile::new("/tmp/repo", "test");
        assert_eq!(profile.local_endpoint(), "http://127.0.0.1:28766/mcp");
    }
}
```

## 测试编写原则

1. **行为契约优先** — 以 `old/docs/profile-v0.1.md` 为准，不猜测
2. **先移植再扩展** — 旧版合规测试是回归基线
3. **安全测试不可跳过** — 路径穿越、命令注入等必须覆盖
4. **Windows 兼容** — 进程管理和路径处理需在 Windows 上验证

## 旧版测试参考

```bash
# 旧版 Python 合规测试
cd old && make compliance
```

当前基线：`old/reports/compliance/latest.md` — 71 项全 PASS。

---
*返回索引: [../project-context.md](../project-context.md)*
