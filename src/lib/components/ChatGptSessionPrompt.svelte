<script lang="ts">
  import { Check, Copy } from "@lucide/svelte";
  import { onDestroy } from "svelte";
  import { showToast } from "$lib/stores/toast";

  const sessionPrompt = `恢复会话。先调用 history_session_bootstrap。
读取 all_history_summary 和 latest_handoff 后继续工作。
本会话每轮最终回复前必须调用 history_session_checkpoint，
只有 checkpoint 返回 ok=true 后才能回复我。`;

  let copying = $state(false);
  let copied = $state(false);
  let errorMessage = $state("");
  let resetTimer: ReturnType<typeof setTimeout> | undefined;

  async function copyPrompt() {
    if (copying) return;
    copying = true;
    copied = false;
    errorMessage = "";
    if (resetTimer) clearTimeout(resetTimer);
    try {
      await navigator.clipboard.writeText(sessionPrompt);
      copied = true;
      showToast("会话恢复提示词已复制，可以直接粘贴到 ChatGPT。", {
        title: "复制成功",
        kind: "success",
        duration: 2500,
      });
      resetTimer = setTimeout(() => {
        copied = false;
      }, 2000);
    } catch (error) {
      errorMessage = "复制失败，请选中提示词后手动复制。";
      showToast(String(error), {
        title: "无法复制提示词",
        kind: "error",
        duration: 6000,
      });
    } finally {
      copying = false;
    }
  }

  onDestroy(() => {
    if (resetTimer) clearTimeout(resetTimer);
  });
</script>

<section aria-labelledby="chatgpt-session-prompt-title">
  <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
    <div class="min-w-0">
      <h3 id="chatgpt-session-prompt-title" class="text-sm font-semibold text-[var(--color-text)]">
        ChatGPT 会话恢复提示词
      </h3>
      <p class="mt-1 text-xs leading-5 text-[var(--color-text-muted)]">
        新版通常只需输入“恢复会话”。首次使用或需要强化持久化规则时，复制完整提示词。
      </p>
    </div>
    <button
      type="button"
      class="tx-btn-ghost min-h-11 shrink-0 px-3 py-2 text-xs disabled:cursor-not-allowed disabled:opacity-50"
      disabled={copying}
      aria-label="复制 ChatGPT 会话恢复提示词"
      onclick={() => void copyPrompt()}
    >
      {#if copied}
        <Check size={14} aria-hidden="true" />
        <span>已复制</span>
      {:else}
        <Copy size={14} aria-hidden="true" />
        <span>{copying ? "复制中…" : "复制完整提示词"}</span>
      {/if}
    </button>
  </div>

  <pre
    class="tx-mono mt-3 whitespace-pre-wrap break-words rounded-[10px] bg-[var(--surface-hover)] p-3 leading-5 text-[var(--color-text-secondary)]"
  >{sessionPrompt}</pre>

  <p class="mt-2 text-[11px] leading-5 text-[var(--color-text-muted)]">
    该入口显示在每个工作区中，复制后粘贴到使用当前工作区 MCP 连接器的 ChatGPT 会话。
  </p>
  {#if errorMessage}
    <p class="mt-2 text-xs text-[var(--danger)]" role="alert">{errorMessage}</p>
  {/if}
  <span class="sr-only" aria-live="polite">{copied ? "提示词已复制" : ""}</span>
</section>
