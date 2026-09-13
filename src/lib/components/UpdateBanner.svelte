<script lang="ts">
  import { onMount } from "svelte";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { toasts } from "../stores/session.svelte";

  let update = $state<Update | null>(null);
  let progress = $state<number | null>(null);
  let total = 0;
  let received = 0;

  onMount(async () => {
    try {
      const u = await check();
      if (u) update = u;
    } catch {
      // Offline or no releases yet: silently skip.
    }
  });

  async function install() {
    if (!update) return;
    progress = 0;
    try {
      await update.downloadAndInstall((ev) => {
        if (ev.event === "Started") total = ev.data.contentLength ?? 0;
        else if (ev.event === "Progress") {
          received += ev.data.chunkLength;
          progress = total ? Math.round((received / total) * 100) : null;
        } else if (ev.event === "Finished") progress = 100;
      });
      await relaunch();
    } catch (e) {
      toasts.error(e);
      progress = null;
    }
  }
</script>

{#if update}
  <div class="banner">
    <span><strong>Update {update.version} is ready.</strong> Install takes about a minute and restarts the app.</span>
    {#if progress === null}
      <button class="btn btn-sm" onclick={() => (update = null)}>Later</button>
      <button class="btn btn-sm btn-primary" onclick={install}>Install now</button>
    {:else}
      <span class="num">Downloading… {progress}%</span>
    {/if}
  </div>
{/if}

<style>
  .banner {
    display: flex; align-items: center; gap: 10px; padding: 8px 16px;
    background: var(--accent-soft); border-bottom: 1px solid #f3d9b1; color: #7c2d12;
  }
  .banner span:first-child { flex: 1; }
</style>
