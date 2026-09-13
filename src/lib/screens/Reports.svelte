<script lang="ts">
  import { api } from "../api";
  import type { DailyPoint, SalesSummary, StockValue, TopProduct } from "../types";
  import { session, toasts } from "../stores/session.svelte";
  import { fmtDate, int, isoDate, money, weekStart } from "../format";
  import DateRange from "../components/DateRange.svelte";

  let from = $state(weekStart(isoDate()));
  let to = $state(isoDate());
  let summary = $state<SalesSummary | null>(null);
  let daily = $state<DailyPoint[]>([]);
  let top = $state<TopProduct[]>([]);
  let stock = $state<StockValue | null>(null);
  let exporting = $state(false);

  async function load() {
    try {
      [summary, daily, top, stock] = await Promise.all([
        api.salesSummary(from, to),
        api.dailySales(from, to),
        api.topProducts(from, to, 15),
        api.stockValue(),
      ]);
    } catch (e) {
      toasts.error(e);
    }
  }
  // $effect runs once on mount and again whenever the range changes.
  $effect(() => {
    void from; void to;
    load();
  });

  async function exportCsv() {
    exporting = true;
    try {
      const path = await api.exportSalesCsv(from, to);
      toasts.success(`Saved to ${path}`);
    } catch (e) {
      toasts.error(e);
    } finally {
      exporting = false;
    }
  }

  const pct = (part: number, whole: number) => (whole > 0 ? `${Math.round((part / whole) * 100)}%` : "—");
</script>

<div class="page">
  <div class="page-header">
    <h1>Reports</h1>
    <div class="toolbar">
      <DateRange bind:from bind:to />
      {#if session.isOwner}
        <button class="btn" onclick={exportCsv} disabled={exporting}>Export to Excel (CSV)</button>
      {/if}
    </div>
  </div>
  <div class="page-body stack">
    {#if summary}
      <div class="grid-stats">
        <div class="card stat"><div class="label">Net sales</div><div class="value">{money(summary.net)}</div><div class="sub">{int(summary.salesCount)} sales, {int(summary.itemsSold)} items</div></div>
        <div class="card stat"><div class="label">Profit</div><div class="value">{money(summary.profit)}</div><div class="sub">{pct(summary.profit, summary.net)} margin</div></div>
        <div class="card stat"><div class="label">Cash</div><div class="value">{money(summary.cashTotal)}</div><div class="sub">{pct(summary.cashTotal, summary.net)} of sales</div></div>
        <div class="card stat"><div class="label">M-Pesa</div><div class="value">{money(summary.mpesaTotal)}</div><div class="sub">{pct(summary.mpesaTotal, summary.net)} of sales</div></div>
        <div class="card stat"><div class="label">Discounts</div><div class="value">{money(summary.discounts)}</div><div class="sub">from {money(summary.gross)} gross</div></div>
        <div class="card stat"><div class="label">Voided</div><div class="value">{int(summary.voidedCount)}</div><div class="sub">sales cancelled</div></div>
      </div>
    {/if}

    <div class="grid-2">
      <div class="card">
        <div class="card-header"><h2>By day</h2></div>
        <div class="table-wrap">
          <table class="table">
            <thead><tr><th>Day</th><th class="num">Sales</th><th class="num">Net</th><th class="num">Profit</th></tr></thead>
            <tbody>
              {#each daily as d (d.day)}
                <tr><td>{fmtDate(d.day)}</td><td class="num">{d.salesCount}</td><td class="num">{money(d.net)}</td><td class="num">{money(d.profit)}</td></tr>
              {:else}
                <tr><td colspan="4" class="empty">No sales in this period.</td></tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>
      <div class="card">
        <div class="card-header"><h2>Best sellers</h2></div>
        <div class="table-wrap">
          <table class="table">
            <thead><tr><th>Product</th><th class="num">Qty</th><th class="num">Revenue</th><th class="num">Profit</th></tr></thead>
            <tbody>
              {#each top as t (t.productId)}
                <tr><td>{t.name}</td><td class="num">{t.qty}</td><td class="num">{money(t.revenue)}</td><td class="num">{money(t.profit)}</td></tr>
              {:else}
                <tr><td colspan="4" class="empty">Nothing sold yet.</td></tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>
    </div>

    {#if stock}
      <div>
        <h3 style="margin-bottom: 8px">Stock right now</h3>
        <div class="grid-stats">
          <div class="card stat"><div class="label">Products</div><div class="value">{int(stock.productCount)}</div><div class="sub">{int(stock.units)} units on shelf</div></div>
          <div class="card stat"><div class="label">Stock at cost</div><div class="value">{money(stock.costValue)}</div><div class="sub">money tied up in stock</div></div>
          <div class="card stat"><div class="label">Stock at retail</div><div class="value">{money(stock.retailValue)}</div><div class="sub">if everything sells</div></div>
          <div class="card stat"><div class="label">Low stock</div><div class="value" class:danger={stock.lowStockCount > 0}>{int(stock.lowStockCount)}</div><div class="sub">products to reorder</div></div>
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .danger { color: var(--danger); }
</style>
