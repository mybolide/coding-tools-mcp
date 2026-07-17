import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const workspacePagePath = new URL("../src/routes/workspace/[id]/+page.svelte", import.meta.url);
const quickCopyPath = new URL("../src/lib/components/GptQuickCopy.svelte", import.meta.url);
const sessionPromptPath = new URL(
  "../src/lib/components/ChatGptSessionPrompt.svelte",
  import.meta.url,
);

test("工作区页在服务内容之前展示会话恢复快捷入口", async () => {
  const source = await readFile(workspacePagePath, "utf8");
  const promptIndex = source.indexOf("<ChatGptSessionPrompt />");
  const pageBodyIndex = source.indexOf('<div class="page-body">');

  assert.match(source, /import ChatGptSessionPrompt from/);
  assert.notEqual(promptIndex, -1);
  assert.ok(promptIndex < pageBodyIndex, "会话恢复入口应位于工作区页头，而不是服务内容卡片中");
});

test("GPT 配置卡片不再重复展示会话恢复入口", async () => {
  const source = await readFile(quickCopyPath, "utf8");

  assert.doesNotMatch(source, /ChatGptSessionPrompt/);
});

test("会话恢复快捷入口默认紧凑，并可展开完整提示词", async () => {
  const source = await readFile(sessionPromptPath, "utf8");

  assert.match(source, /let expanded = \$state\(false\)/);
  assert.match(source, /aria-expanded=\{expanded\}/);
  assert.match(source, /查看完整提示词/);
  assert.match(source, /\{#if expanded\}[\s\S]*<pre/);
});

test("复制和展开操作保留可触达尺寸与状态反馈", async () => {
  const source = await readFile(sessionPromptPath, "utf8");

  assert.ok((source.match(/min-h-11/g) ?? []).length >= 2, "两个操作按钮都应至少为 44px 高");
  assert.match(source, /aria-live="polite"/);
  assert.match(source, /复制完整提示词/);
  assert.match(source, /已复制/);
});
