<script lang="ts">
  import { api } from "../api";
  import type { Product } from "../types";
  import { toasts } from "../stores/session.svelte";
  import { money } from "../format";

  let {
    onpick,
    placeholder = "Scan barcode or type a product name",
    large = false,
  }: { onpick: (p: Product) => void; placeholder?: string; large?: boolean } = $props();

  let text = $state("");
  let results = $state<Product[]>([]);
  let hi = $state(0);
  let input = $state<HTMLInputElement | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  // Barcode scanners type digits fast and finish with Enter; skip live search for those.
  const looksLikeBarcode = (s: string) => /^\d{4,}$/.test(s);

  export function focus() {
    input?.focus();
  }

  function oninput() {
    clearTimeout(timer);
    const q = text.trim();
    if (q.length < 2 || looksLikeBarcode(q)) {
      results = [];
      return;
    }
    timer = setTimeout(async () => {
      try {
        results = (await api.listProducts(q)).slice(0, 8);
        hi = 0;
      } catch (e) {
        toasts.error(e);
      }
    }, 120);
  }

  async function onkeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      hi = Math.min(hi + 1, results.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      hi = Math.max(hi - 1, 0);
    } else if (e.key === "Enter") {
      e.preventDefault();
      await submit();
    } else if (e.key === "Escape") {
      text = "";
      results = [];
    }
  }

  async function submit() {
    const q = text.trim();
    if (!q) return;
    if (results.length && !looksLikeBarcode(q)) {
      pick(results[hi]);
      return;
    }
    try {
      const p = await api.findProductByBarcode(q);
      if (p) pick(p);
      else {
        toasts.error(`No product with barcode ${q}`);
        text = "";
      }
    } catch (e) {
      toasts.error(e);
    }
  }

  function pick(p: Product) {
    clearTimeout(timer);
    onpick(p);
    text = "";
    results = [];
    input?.focus();
  }
</script>

<div class="picker">
  <input
    bind:this={input}
    bind:value={text}
    class="input"
    class:input-lg={large}
    {placeholder}
    autocomplete="off"
    spellcheck="false"
    oninput={oninput}
    onkeydown={onkeydown}
    onblur={() => setTimeout(() => (results = []), 150)}
  />
  {#if results.length}
    <ul class="results">
      {#each results as p, i (p.id)}
        <li>
          <button class:hi={i === hi} onmousedown={() => pick(p)} onmouseenter={() => (hi = i)}>
            <span class="pname">{p.name}</span>
            <span class="pmeta muted">{p.category ?? ""}</span>
            <span class="pstock" class:low={p.stockQty <= p.reorderLevel}>{p.stockQty} left</span>
            <span class="pprice num">{money(p.sellPrice)}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .picker { position: relative; }
  .results {
    position: absolute; top: 100%; left: 0; right: 0; margin: 4px 0 0; padding: 4px; list-style: none;
    background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.12); z-index: 20;
  }
  .results button {
    width: 100%; display: grid; grid-template-columns: 1fr auto auto 90px; gap: 12px; align-items: center;
    padding: 8px 10px; border: 0; background: transparent; border-radius: 4px; cursor: pointer; text-align: left;
  }
  .results button.hi { background: var(--accent-soft); }
  .pname { font-weight: 500; }
  .pmeta, .pstock { font-size: 12px; }
  .pstock { color: var(--ink-3); }
  .pstock.low { color: var(--danger); }
  .pprice { text-align: right; font-weight: 500; }
</style>
