<script lang="ts">
  import { Check, ChevronDown, Copy, History } from "@lucide/svelte";
  import { onDestroy } from "svelte";
  import { showToast } from "$lib/stores/toast";

  const sessionPrompt = `恢复会话。先调用 history_session_bootstrap。
读取 all_history_summary 和 latest_handoff 后继续工作。
本会话每轮最终回复前必须调用 history_session_checkpoint，
只有 checkpoint 返回 ok=true 后才能回复我。`;

  let copying = $state(false);
  let copied = $state(false);
  let expanded = $state(false);
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

<section
  class="rounded-[12px] border border-[var(--color-border)] bg-[var(--card-bg)] px-3 py-2.5 sm:px-4"
  aria-labelledby="chatgpt-session-prompt-title"
>
  <div class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between sm:gap-4">
    <div class="flex min-w-0 items-center gap-3">
      <span
        class="flex size-9 shrink-0 items-center justify-center rounded-[10px] bg-[var(--primary-soft)] text-[var(--primary)]"
        aria-hidden="true"
      >
        <History size={16} />
      </span>
      <div class="min-w-0">
        <h3 id="chatgpt-session-prompt-title" class="text-sm font-semibold text-[var(--color-text)]">
          ChatGPT 会话恢复提示词
        </h3>
        <p class="mt-0.5 text-xs leading-5 text-[var(--color-text-muted)]">
          新会话粘贴一次；之后由 checkpoint 持续保存开发状态。
        </p>
      </div>
    </div>

    <div class="flex shrink-0 flex-wrap items-center gap-2 sm:flex-nowrap">
      <button
        type="button"
        class="tx-btn-primary min-h-11 shrink-0 px-3 py-2 text-xs active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-50"
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

      <button
        type="button"
        class="tx-btn-ghost min-h-11 shrink-0 gap-1.5 px-3 py-2 text-xs active:scale-[0.98]"
        aria-expanded={expanded}
        aria-controls="chatgpt-session-prompt-content"
        onclick={() => (expanded = !expanded)}
      >
        <span>{expanded ? "收起提示词" : "查看完整提示词"}</span>
        <ChevronDown
          size={14}
          class={`transition-transform duration-200 motion-reduce:transition-none ${expanded ? "rotate-180" : ""}`}
          aria-hidden="true"
        />
      </button>
    </div>
  </div>

  {#if expanded}
    <div id="chatgpt-session-prompt-content" class="mt-3 border-t border-[var(--color-border)] pt-3">
      <pre
        class="tx-mono whitespace-pre-wrap break-words rounded-[10px] bg-[var(--surface-hover)] p-3 leading-5 text-[var(--color-text-secondary)]"
      >{sessionPrompt}</pre>
      <p class="mt-2 text-[11px] leading-5 text-[var(--color-text-muted)]">
        复制后粘贴到使用当前工作区 MCP 连接器的 ChatGPT 新会话。
      </p>
    </div>
  {/if}

  {#if errorMessage}
    <p class="mt-2 text-xs text-[var(--danger)]" role="alert">{errorMessage}</p>
  {/if}
  <span class="sr-only" aria-live="polite">{copied ? "提示词已复制" : ""}</span>
</section>
