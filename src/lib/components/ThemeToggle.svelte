<script lang="ts">
  import { Moon, Sun } from "@lucide/svelte";
  import { onMount } from "svelte";

  let dark = $state(true);

  onMount(() => {
    const stored = localStorage.getItem("theme");
    if (stored === "light" || stored === "dark") {
      dark = stored === "dark";
    } else {
      dark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    }
    apply();
  });

  function apply() {
    const theme = dark ? "dark" : "light";
    document.documentElement.setAttribute("data-theme", theme);
    document.documentElement.classList.toggle("dark", dark);
    localStorage.setItem("theme", theme);
  }

  function toggle() {
    dark = !dark;
    apply();
  }
</script>

<button
  type="button"
  class="inline-flex h-9 w-9 items-center justify-center rounded-[10px] border border-white/10 bg-white/5 text-[#c5d0ea] transition-colors hover:bg-white/10"
  onclick={toggle}
  aria-label="切换主题"
>
  {#if dark}
    <Sun size={16} />
  {:else}
    <Moon size={16} />
  {/if}
</button>
