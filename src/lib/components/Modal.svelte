<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    title,
    open = $bindable(false),
    width = 520,
    onclose,
    children,
    footer,
  }: {
    title: string;
    open?: boolean;
    width?: number;
    onclose?: () => void;
    children: Snippet;
    footer?: Snippet;
  } = $props();

  function close() {
    open = false;
    onclose?.();
  }
  function onkeydown(e: KeyboardEvent) {
    if (open && e.key === "Escape") {
      e.stopPropagation();
      close();
    }
  }
</script>

<svelte:window onkeydown={onkeydown} />

{#if open}
  <div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && close()}>
    <div class="modal" style="width: {width}px" role="dialog" aria-modal="true" aria-label={title}>
      <header>
        <h2>{title}</h2>
        <button class="btn btn-ghost btn-sm" onclick={close} aria-label="Close">✕</button>
      </header>
      <div class="content">{@render children()}</div>
      {#if footer}
        <footer>{@render footer()}</footer>
      {/if}
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed; inset: 0; background: rgba(28, 25, 23, 0.45);
    display: flex; align-items: center; justify-content: center; z-index: 50;
  }
  .modal {
    max-width: calc(100vw - 40px); max-height: calc(100vh - 40px);
    background: var(--surface); border-radius: 8px; box-shadow: 0 20px 50px rgba(0, 0, 0, 0.25);
    display: flex; flex-direction: column;
  }
  header { display: flex; align-items: center; justify-content: space-between; padding: 14px 18px; border-bottom: 1px solid var(--border); }
  .content { padding: 18px; overflow: auto; }
  footer { display: flex; justify-content: flex-end; gap: 8px; padding: 12px 18px; border-top: 1px solid var(--border); background: var(--surface-2); border-radius: 0 0 8px 8px; }
</style>
