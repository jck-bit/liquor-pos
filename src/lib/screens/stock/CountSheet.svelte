<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../../api";
  import type { Product } from "../../types";
  import { session, toasts } from "../../stores/session.svelte";
  import { fmtDate, isoDate, int } from "../../format";

  let products = $state<Product[]>([]);
  let blind = $state(false);
  let saving = $state(false);
  const today = isoDate();

  onMount(async () => {
    try {
      products = await api.listProducts("", false);
    } catch (e) {
      toasts.error(e);
    }
  });

  // Grouped by category, in category order, so the sheet follows the shelves.
  const groups = $derived.by(() => {
    const byCat = new Map<string, Product[]>();
    for (const p of products) {
      const key = p.category?.trim() || "Uncategorised";
      byCat.set(key, [...(byCat.get(key) ?? []), p]);
    }
    return [...byCat.entries()]
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([category, items]) => ({ category, items: items.sort((a, b) => a.name.localeCompare(b.name)) }));
  });

  async function saveCsv() {
    saving = true;
    try {
      const path = await api.exportCountSheetCsv();
      toasts.success(`Saved to ${path}. Fill the stock column, then Products > Import CSV.`);
    } catch (e) {
      toasts.error(e);
    } finally {
      saving = false;
    }
  }
</script>

<div class="stack">
  <div class="toolbar no-print">
    <label class="check"><input type="checkbox" bind:checked={blind} /> Blind count (hide the system numbers)</label>
    <span class="spacer"></span>
    <span class="muted">{int(products.length)} products</span>
    <button class="btn" onclick={saveCsv} disabled={saving}>Save as CSV to fill in</button>
    <button class="btn btn-primary" onclick={() => window.print()}>Print</button>
  </div>

  <div class="card sheet print-area">
    <div class="head">
      <div>
        <div class="title">{session.storeName} · Stock count</div>
        <div class="muted">Printed {fmtDate(today)} · {int(products.length)} products</div>
      </div>
      <div class="sign">
        <div>Counted by ________________________</div>
        <div>Checked by ________________________</div>
      </div>
    </div>
    <table class="table">
      <thead>
        <tr><th>Product</th><th>Barcode</th>{#if !blind}<th class="num">In system</th>{/if}<th class="fill">Counted</th><th class="note">Note</th></tr>
      </thead>
      {#each groups as g (g.category)}
        <tbody>
          <tr class="cat"><td colspan={blind ? 4 : 5}>{g.category}</td></tr>
          {#each g.items as p (p.id)}
            <tr>
              <td>{p.name}</td>
              <td class="mono muted">{p.barcode ?? ""}</td>
              {#if !blind}<td class="num">{p.stockQty}</td>{/if}
              <td class="fill"></td>
              <td class="note"></td>
            </tr>
          {/each}
        </tbody>
      {/each}
    </table>
  </div>
</div>

<style>
  .head { display: flex; justify-content: space-between; align-items: flex-start; gap: 16px; padding: 14px 16px; border-bottom: 1px solid var(--border); }
  .title { font-weight: 600; font-size: 16px; }
  .sign { display: flex; flex-direction: column; gap: 6px; font-size: 13px; color: var(--ink-2); white-space: nowrap; }
  .cat td { font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.04em; color: var(--ink-3); background: var(--surface-2); }
  th.fill, td.fill { width: 90px; }
  th.note, td.note { width: 160px; }
  td.fill, td.note { border-bottom: 1px solid var(--border-strong); }
  @media print {
    .sheet { border: 0; box-shadow: none; font-size: 11px; }
    .sheet :global(.table th), .sheet :global(.table td) { padding: 5px 6px; border-bottom: 1px solid #999; }
    .sheet :global(.table th) { background: #eee; color: #000; }
    .cat td { background: #eee; color: #000; }
    td.fill, td.note { border-bottom: 1px solid #000; }
  }
</style>
