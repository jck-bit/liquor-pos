<script lang="ts">
  import { api } from "../../api";
  import type { StockMatrix } from "../../types";
  import { toasts } from "../../stores/session.svelte";
  import { int, money } from "../../format";

  let { tick }: { tick: number } = $props();

  let matrix = $state<StockMatrix | null>(null);
  let query = $state("");
  let lowOnly = $state(false);

  // Reloads whenever the parent finishes a refresh.
  $effect(() => {
    void tick;
    api.allShopsStock().then((m) => (matrix = m), (e) => toasts.error(e));
  });

  const rows = $derived.by(() => {
    if (!matrix) return [];
    const q = query.trim().toLowerCase();
    return matrix.rows.filter(
      (r) =>
        (!q || r.name.toLowerCase().includes(q) || (r.category ?? "").toLowerCase().includes(q)) &&
        (!lowOnly || r.cells.some((c) => c?.low)),
    );
  });
  const totals = $derived(matrix ? matrix.shops.map((_, i) => rows.reduce((t, r) => t + (r.cells[i]?.qty ?? 0), 0)) : []);
  const prices = (r: StockMatrix["rows"][number]) =>
    matrix!.shops.map((s, i) => (r.cells[i] ? `${s.name} ${money(r.cells[i]!.price)}` : null)).filter(Boolean).join(" · ");
</script>

{#if matrix}
  <div class="toolbar">
    <input class="input search" placeholder="Search product or category" bind:value={query} />
    <label class="check"><input type="checkbox" bind:checked={lowOnly} /> Running low in any shop</label>
    <span class="spacer"></span>
    <span class="muted">{int(rows.length)} of {int(matrix.rows.length)} products</span>
  </div>

  <div class="card table-wrap">
    <table class="table">
      <thead>
        <tr>
          <th>Product</th><th>Category</th>
          {#each matrix.shops as s (s.id)}<th class="num">{s.name}</th>{/each}
          <th class="num">All shops</th>
        </tr>
      </thead>
      <tbody>
        {#each rows as r (r.name)}
          <tr>
            <td class="strong">
              {r.name}
              {#if r.priceDiffers}<span class="badge badge-amber" title={prices(r)}>price differs</span>{/if}
            </td>
            <td class="muted">{r.category ?? ""}</td>
            {#each r.cells as c, i (matrix.shops[i].id)}
              <td class="num">
                {#if !c}<span class="muted" title="Not stocked in this shop">—</span>
                {:else if c.qty <= 0}<span class="badge badge-red">{c.qty}</span>
                {:else if c.low}<span class="badge badge-amber">{c.qty}</span>
                {:else}{c.qty}{/if}
              </td>
            {/each}
            <td class="num strong">{int(r.total)}</td>
          </tr>
        {:else}
          <tr><td colspan={matrix.shops.length + 3} class="empty">{query || lowOnly ? "No products match." : "No products yet."}</td></tr>
        {/each}
      </tbody>
      {#if rows.length}
        <tfoot>
          <tr>
            <td colspan="2" class="right muted">Units shown</td>
            {#each totals as t, i (matrix.shops[i].id)}<td class="num strong">{int(t)}</td>{/each}
            <td class="num strong">{int(totals.reduce((a, b) => a + b, 0))}</td>
          </tr>
        </tfoot>
      {/if}
    </table>
  </div>
  {#each matrix.shops.filter((s) => s.error) as s (s.id)}
    <p class="muted"><strong>{s.name}:</strong> {s.error}</p>
  {/each}
{:else}
  <div class="empty">Loading stock…</div>
{/if}

<style>
  .search { width: 300px; }
  .strong { font-weight: 500; }
  tfoot td { background: var(--surface-2); }
</style>
