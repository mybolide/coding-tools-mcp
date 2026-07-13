<script lang="ts">
  import { fly } from "svelte/transition";
  import { AlertTriangle, CheckCircle2, Info, X, XCircle } from "@lucide/svelte";
  import { dismissToast, toasts, type ToastKind } from "$lib/stores/toast";

  const icons: Record<ToastKind, typeof Info> = {
    info: Info,
    success: CheckCircle2,
    warning: AlertTriangle,
    error: XCircle,
  };
</script>

<div class="tx-toast-host" aria-live="polite" aria-atomic="false">
  {#each $toasts as toast (toast.id)}
  {@const Icon = icons[toast.kind]}
    <div
      class="tx-toast tx-toast--{toast.kind}"
      role="status"
      transition:fly={{ x: 24, duration: 220 }}
    >
      <div class="tx-toast__icon" aria-hidden="true">
        <Icon size={18} strokeWidth={2.25} />
      </div>
      <div class="tx-toast__body">
        {#if toast.title}
          <p class="tx-toast__title">{toast.title}</p>
        {/if}
        <p class="tx-toast__message">{toast.message}</p>
      </div>
      <button
        type="button"
        class="tx-toast__close"
        aria-label="关闭"
        onclick={() => dismissToast(toast.id)}
      >
        <X size={14} strokeWidth={2.25} />
      </button>
    </div>
  {/each}
</div>
