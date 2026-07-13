# Graph Snapshot

- id: 1783926256771-6fadac
- tool: code_insight
- status: degraded
- createdAt: 2026-07-13T07:04:16.771Z
- summary: GitNexus bridge 已降级：query: Error: Repository "coding-tools-mcp-desktop" not found. Available: opencode-brige-feishu, mcp-probe-kit, feishu-agent-bridge, yumoxingchen-1773394036698-6c5df4, bytezonex, zhixing, hosemi, hognshang-sanrenzu, g...

## Payload
```json
{
  "status": "degraded",
  "provider": "gitnexus",
  "mode": {
    "requested": "auto",
    "resolved": "query"
  },
  "summary": "GitNexus bridge 已降级：query: Error: Repository \"coding-tools-mcp-desktop\" not found. Available: opencode-brige-feishu, mcp-probe-kit, feishu-agent-bridge, yumoxingchen-1773394036698-6c5df4, bytezonex, zhixing, hosemi, hognshang-sanrenzu, g...",
  "warnings": [
    "bridge_call_failed"
  ],
  "executions": [
    {
      "tool": "query",
      "args": {
        "query": "项目整体架构 核心流程 关键模块 依赖关系 入口点",
        "task_context": "用户希望快速了解项目整体结构、技术栈、核心模块、入口和当前改动风险",
        "repo": "coding-tools-mcp-desktop"
      },
      "ok": false,
      "durationMs": 5,
      "text": "Error: Repository \"coding-tools-mcp-desktop\" not found. Available: opencode-brige-feishu, mcp-probe-kit, feishu-agent-bridge, yumoxingchen-1773394036698-6c5df4, bytezonex, zhixing, hosemi, hognshang-sanrenzu, go-word, gongyignshang, alphaloop, coding-tools-mcp (E:\\workspace\\github\\coding-tools-mcp), coding-tools-mcp (E:\\workspace\\github\\coding-tools-mcp-rust)",
      "error": "Error: Repository \"coding-tools-mcp-desktop\" not found. Available: opencode-brige-feishu, mcp-probe-kit, feishu-agent-bridge, yumoxingchen-1773394036698-6c5df4, bytezonex, zhixing, hosemi, hognshang-sanrenzu, go-word, gongyignshang, alphaloop, coding-tools-mcp (E:\\workspace\\github\\coding-tools-mcp), coding-tools-mcp (E:\\workspace\\github\\coding-tools-mcp-rust)"
    }
  ],
  "repo": "coding-tools-mcp-desktop",
  "launcherStrategy": "local",
  "ambiguities": [],
  "workspaceMode": "direct",
  "sourceRoot": "E:\\workspace\\github\\coding-tools-mcp-rust",
  "analysisRoot": "E:\\workspace\\github\\coding-tools-mcp-rust",
  "pathMapped": false,
  "projectDocs": {
    "docsDir": "docs",
    "projectContextFilePath": "E:/workspace/github/coding-tools-mcp-rust/AGENTS.md",
    "latestMarkdownFilePath": "docs/graph-insights/latest.md",
    "latestJsonFilePath": "docs/graph-insights/latest.json",
    "archiveMarkdownFilePath": "E:/workspace/github/coding-tools-mcp-rust/docs/graph-insights/2026-07-13T07-04-16-770Z-auto-gitnexus-bridge-query-error-repository-coding-to.md",
    "archiveJsonFilePath": "E:/workspace/github/coding-tools-mcp-rust/docs/graph-insights/2026-07-13T07-04-16-770Z-auto-gitnexus-bridge-query-error-repository-coding-to.json",
    "navigationSnippet": "### [代码图谱洞察](./graph-insights/latest.md)\n最近一次 code_insight 分析结果，包含调用链、上下文与影响面摘要\n",
    "devGuideSnippet": "- **代码图谱洞察**: [graph-insights/latest.md](./graph-insights/latest.md) - 需要理解模块依赖、调用链和影响面时优先查看\n"
  },
  "plan": {
    "mode": "delegated",
    "kind": "docs",
    "steps": [
      {
        "id": "consume-result",
        "action": "先消费本次分析结果（processes/symbols/impact），确认是否满足当前问题"
      },
      {
        "id": "optional-save",
        "action": "如需保存，再写入 docs/graph-insights/latest.md（文本）和 docs/graph-insights/latest.json（结构化）",
        "outputs": [
          "docs/graph-insights/latest.md",
          "docs/graph-insights/latest.json"
        ],
        "note": "可选同步更新 E:/workspace/github/coding-tools-mcp-rust/AGENTS.md 的图谱入口"
      }
    ]
  },
  "nextAction": "请按 delegated plan 由 Agent 落盘图谱文档，并更新 E:/workspace/github/coding-tools-mcp-rust/AGENTS.md 的索引入口",
  "handles": {
    "graph_resource": "probe://graph/latest"
  },
  "mcp_probe_bootstrap": {
    "projectRoot": "E:\\workspace\\github\\coding-tools-mcp-rust",
    "skill": {
      "skillPath": "E:\\workspace\\github\\coding-tools-mcp-rust\\.agents\\skills\\mcp-probe-kit\\SKILL.md",
      "skillRelPath": ".agents/skills/mcp-probe-kit/SKILL.md",
      "existed": true,
      "created": false,
      "updated": false,
      "version": "3.6.11",
      "previousVersion": "3.6.11"
    },
    "agentsMd": {
      "path": "AGENTS.md",
      "existed": true,
      "created": false,
      "updated": false
    },
    "harness": {
      "detection": {
        "markerHarnesses": [
          "cursor"
        ],
        "detected": [
          "cursor"
        ],
        "skillCanonical": ".agents/skills/mcp-probe-kit/SKILL.md",
        "adaptersToWrite": []
      },
      "adapters": [],
      "layoutHarness": {
        "detected": [
          "cursor"
        ],
        "skillCanonical": ".agents/skills/mcp-probe-kit/SKILL.md",
        "adapters": []
      }
    },
    "workspaceWarning": null
  }
}
```
