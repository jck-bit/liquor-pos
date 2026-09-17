<script lang="ts">
  import type { AllShopsOverview, ShopFigures } from "../../types";
  import { ago, fmtDate, fmtDateTime, int, money } from "../../format";

  let { overview }: { overview: AllShopsOverview } = $props();

  const shops = $derived(overview.shops);
  const figures = $derived(shops.flatMap((s) => (s.figures ? [s.figures] : [])));

  const add = (fs: ShopFigures[], pick: (f: ShopFigures) => number) => fs.reduce((t, f) => t + pick(f), 0);
  const pct = (part: number, whole: number) => (whole > 0 ? `${Math.round((part / whole) * 100)}%` : "—");
  const dash = "—";

  const net = $derived(add(figures, (f) => f.summary.net));
  const cost = $derived(add(figures, (f) => f.summary.cost));
  const count = $derived(add(figures, (f) => f.summary.salesCount));
  const items = $derived(add(figures, (f) => f.summary.itemsSold));
  /** No cost prices recorded yet: profit would just repeat sales, so it is not shown. */
  const uncosted = $derived(net > 0 && cost === 0);

  const quiet = "no sales in this period";
  const share = (part: number) => (net > 0 ? `${pct(part, net)} of sales` : quiet);
  const profitOf = (netSales: number, costOfSales: number) => (netSales > 0 && costOfSales === 0 ? dash : money(netSales - costOfSales));
  const average = (netSales: number, sales: number) => (sales > 0 ? money(Math.round(netSales / sales)) : dash);
  const latest = (fs: ShopFigures[]) => fs.map((f) => f.lastSaleAt ?? "").sort().at(-1) || null;

  type Metric = { label: string; cell: (f: ShopFigures) => string; all: (fs: ShopFigures[]) => string; section?: string };
  const metrics: Metric[] = [
    { label: "Net sales", cell: (f) => money(f.summary.net), all: (fs) => money(add(fs, (f) => f.summary.net)) },
    { label: "Share of sales", cell: (f) => pct(f.summary.net, net), all: () => (net > 0 ? "100%" : dash) },
    { label: "Profit", cell: (f) => profitOf(f.summary.net, f.summary.cost), all: (fs) => profitOf(add(fs, (f) => f.summary.net), add(fs, (f) => f.summary.cost)) },
    { label: "Sales", cell: (f) => int(f.summary.salesCount), all: (fs) => int(add(fs, (f) => f.summary.salesCount)) },
    { label: "Items sold", cell: (f) => int(f.summary.itemsSold), all: (fs) => int(add(fs, (f) => f.summary.itemsSold)) },
    { label: "Average sale", cell: (f) => average(f.summary.net, f.summary.salesCount), all: (fs) => average(add(fs, (f) => f.summary.net), add(fs, (f) => f.summary.salesCount)) },
    { label: "Cash", cell: (f) => money(f.summary.cashTotal), all: (fs) => money(add(fs, (f) => f.summary.cashTotal)) },
    { label: "M-Pesa", cell: (f) => money(f.summary.mpesaTotal), all: (fs) => money(add(fs, (f) => f.summary.mpesaTotal)) },
    { label: "Discounts given", cell: (f) => money(f.summary.discounts), all: (fs) => money(add(fs, (f) => f.summary.discounts)) },
    { label: "Voided sales", cell: (f) => int(f.summary.voidedCount), all: (fs) => int(add(fs, (f) => f.summary.voidedCount)) },
    { label: "Last sale", cell: (f) => (f.lastSaleAt ? fmtDateTime(f.lastSaleAt) : dash), all: (fs) => { const l = latest(fs); return l ? fmtDateTime(l) : dash; } },
    { section: "Stock right now", label: "Products", cell: (f) => int(f.stock.productCount), all: (fs) => int(add(fs, (f) => f.stock.productCount)) },
    { label: "Units on the shelf", cell: (f) => int(f.stock.units), all: (fs) => int(add(fs, (f) => f.stock.units)) },
    { label: "Stock at selling price", cell: (f) => money(f.stock.retailValue), all: (fs) => money(add(fs, (f) => f.stock.retailValue)) },
    { label: "Stock at cost", cell: (f) => (f.stock.costValue > 0 ? money(f.stock.costValue) : dash), all: (fs) => { const c = add(fs, (f) => f.stock.costValue); return c > 0 ? money(c) : dash; } },
    { label: "Products running low", cell: (f) => int(f.stock.lowStockCount), all: (fs) => int(add(fs, (f) => f.stock.lowStockCount)) },
  ];

  // One row per day, newest first, with a column per shop.
  const days = $derived.by(() => {
    const byDay = new Map<string, number[]>();
    shops.forEach((s, i) =>
      s.figures?.daily.forEach((d) => {
        const row = byDay.get(d.day) ?? Array(shops.length).fill(0);
        row[i] = d.net;
        byDay.set(d.day, row);
      }),
    );
    return [...byDay.entries()]
      .sort(([a], [b]) => (a < b ? 1 : -1))
      .map(([day, nets]) => ({ day, nets, total: nets.reduce((a, b) => a + b, 0) }));
  });

  const many = $derived(shops.length > 1);
  const tills = $derived(shops.flatMap((s) => (s.figures?.tills ?? []).map((t) => ({ shop: s.name, ...t }))));
</script>

<div class="top">
  <div class="card hero">
    <div class="label">Net sales, {many ? "all shops" : (shops[0]?.name ?? "")}</div>
    <div class="figure">KES {money(net)}</div>
    <div class="sub">{int(count)} sales · {int(items)} items{#if many} · {figures.length} of {shops.length} shops{/if}</div>
  </div>
  <div class="tiles">
    <div class="card stat"><div class="label">Profit</div><div class="value">{profitOf(net, cost)}</div><div class="sub">{net === 0 ? quiet : uncosted ? "add cost prices to see profit" : `${pct(net - cost, net)} margin`}</div></div>
    <div class="card stat"><div class="label">Cash</div><div class="value">{money(add(figures, (f) => f.summary.cashTotal))}</div><div class="sub">{share(add(figures, (f) => f.summary.cashTotal))}</div></div>
    <div class="card stat"><div class="label">M-Pesa</div><div class="value">{money(add(figures, (f) => f.summary.mpesaTotal))}</div><div class="sub">{share(add(figures, (f) => f.summary.mpesaTotal))}</div></div>
    <div class="card stat"><div class="label">Running low</div><div class="value">{int(add(figures, (f) => f.stock.lowStockCount))}</div><div class="sub">{many ? "products across all shops" : "products to reorder"}</div></div>
  </div>
</div>

<div class="card">
  <div class="card-header"><h2>Shop by shop</h2></div>
  <div class="table-wrap">
    <table class="table compare">
      <thead>
        <tr>
          <th></th>
          {#each shops as s (s.id)}<th class="num">{s.name}</th>{/each}
          {#if many}<th class="num all">All shops</th>{/if}
        </tr>
      </thead>
      <tbody>
        {#each metrics as m (m.label)}
          {#if m.section}
            <tr class="section"><td colspan={shops.length + 2}>{m.section}</td></tr>
          {/if}
          <tr>
            <td class="metric">{m.label}</td>
            {#each shops as s (s.id)}
              <td class="num">{s.figures ? m.cell(s.figures) : dash}</td>
            {/each}
            {#if many}<td class="num all">{m.all(figures)}</td>{/if}
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
  {#each shops.filter((s) => s.error) as s (s.id)}
    <p class="note"><strong>{s.name}:</strong> {s.error}</p>
  {/each}
</div>

<div class="grid-2">
  <div class="card">
    <div class="card-header"><h2>Net sales by day</h2></div>
    <div class="table-wrap">
      <table class="table">
        <thead>
          <tr><th>Day</th>{#each shops as s (s.id)}<th class="num">{s.name}</th>{/each}{#if many}<th class="num">Total</th>{/if}</tr>
        </thead>
        <tbody>
          {#each days as d (d.day)}
            <tr>
              <td>{fmtDate(d.day)}</td>
              {#each d.nets as n, i (shops[i].id)}<td class="num">{n ? money(n) : dash}</td>{/each}
              {#if many}<td class="num strong">{money(d.total)}</td>{/if}
            </tr>
          {:else}
            <tr><td colspan={shops.length + 2} class="empty">No sales in this period.</td></tr>
          {/each}
        </tbody>
      </table>
    </div>
  </div>

  <div class="card">
    <div class="card-header"><h2>Best sellers</h2><span class="muted">units sold</span></div>
    <div class="table-wrap">
      <table class="table">
        <thead>
          <tr><th>Product</th>{#each shops as s (s.id)}<th class="num">{s.name}</th>{/each}<th class="num">Revenue</th></tr>
        </thead>
        <tbody>
          {#each overview.bestSellers as b (b.name)}
            <tr>
              <td>{b.name}</td>
              {#each b.perShop as q, i (shops[i].id)}<td class="num">{q || dash}</td>{/each}
              <td class="num strong">{money(b.revenue)}</td>
            </tr>
          {:else}
            <tr><td colspan={shops.length + 2} class="empty">Nothing sold in this period.</td></tr>
          {/each}
        </tbody>
      </table>
    </div>
  </div>
</div>

<div class="card">
  <div class="card-header"><h2>Computers</h2><span class="muted">when each one last reached its shop's cloud</span></div>
  <div class="table-wrap">
    <table class="table">
      <thead><tr><th>Shop</th><th>Computer</th><th>Last seen</th></tr></thead>
      <tbody>
        {#each tills as t (`${t.shop}:${t.name}`)}
          <tr>
            <td>{t.shop}</td>
            <td class="strong">{t.name} {#if t.thisComputer}<span class="badge badge-gray">this computer</span>{/if}</td>
            <td>{t.lastSeen ? `${ago(t.lastSeen)} · ${fmtDateTime(t.lastSeen)}` : dash}</td>
          </tr>
        {:else}
          <tr><td colspan="3" class="empty">No computers have synced yet.</td></tr>
        {/each}
      </tbody>
    </table>
  </div>
</div>

<style>
  .top { display: grid; grid-template-columns: minmax(300px, 1fr) 2fr; gap: 12px; align-items: stretch; }
  .hero { padding: 18px 20px; display: flex; flex-direction: column; justify-content: center; }
  .hero .label, .stat .label { font-size: 12px; color: var(--ink-3); text-transform: uppercase; letter-spacing: 0.04em; }
  /* Headline numbers use the font's normal figures; aligned digits are for table columns only. */
  .figure { font-size: 48px; font-weight: 600; line-height: 1.15; letter-spacing: -0.02em; font-variant-numeric: normal; white-space: nowrap; }
  .hero .sub { color: var(--ink-2); margin-top: 4px; }
  .tiles { display: grid; grid-template-columns: repeat(2, 1fr); gap: 12px; }
  .stat .value { font-variant-numeric: normal; }
  .compare .metric { color: var(--ink-2); white-space: nowrap; }
  .compare .all { font-weight: 600; background: var(--surface-2); }
  .section td { font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.04em; color: var(--ink-3); background: var(--surface-2); }
  .strong { font-weight: 500; }
  .note { padding: 10px 16px; border-top: 1px solid var(--border); color: var(--ink-2); font-size: 13px; }
  @media (max-width: 1150px) { .top { grid-template-columns: 1fr; } .tiles { grid-template-columns: repeat(4, 1fr); } }
</style>
