<script lang="ts">
  import { toasts } from "../stores/session.svelte";
</script>

<div class="toasts">
  {#each toasts.list as t (t.id)}
    <button class="toast {t.kind}" onclick={() => toasts.dismiss(t.id)}>{t.text}</button>
  {/each}
</div>

<style>
  .toasts { position: fixed; right: 20px; bottom: 20px; display: flex; flex-direction: column; gap: 8px; z-index: 100; }
  .toast {
    text-align: left; border: 0; cursor: pointer; max-width: 380px;
    padding: 10px 14px; border-radius: var(--radius); color: #fff; background: var(--ink);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.2); font-weight: 500;
  }
  .toast.success { background: var(--success); }
  .toast.error { background: var(--danger); }
</style>
