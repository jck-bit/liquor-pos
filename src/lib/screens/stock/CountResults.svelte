<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../../api";
  import type { CountSession, VarianceRow } from "../../types";
  import { session, toasts } from "../../stores/session.svelte";
  import { fmtDate, fmtTime, int, money } from "../../format";

  const short = (s: string) => s.slice(8, 10) + "/" + s.slice(5, 7);
  const zero = (n: number) => (n === 0 ? "—" : int(n));

  let sessions = $state<CountSession[]>([]);
  let day = $state("");
  let rows = $state<VarianceRow[]>([]);
  let onlyDifferences = $state(true);

  onMount(async () => {
    try {
      sessions = await api.countSessions();
      if (sessions.length) day = sessions[0].day;
    } catch (e) {
      toasts.error(e);
    }
  });

  $effect(() => {
    if (!day) return;
    api.countVariance(day).then((r) => (rows = r), (e) => toasts.error(e));
  });

  const current = $derived(sessions.find((s) => s.day === day));
  const isBaseline = (r: VarianceRow) => r.previousCount === null;
  const shown = $derived(onlyDifferences ? rows.filter((r) => !isBaseline(r) && r.difference !== 0) : rows);
  const hasCosts = $derived(rows.some((r) => r.costPrice > 0));
  const unchanged = $derived(rows.filter((r) => !isBaseline(r) && r.difference === 0).length);
  const sell = (r: VarianceRow) => r.difference * r.sellPrice;
  const cost = (r: VarianceRow) => r.difference * r.costPrice;
  const signed = (n: number) => (n > 0 ? `+${int(n)}` : int(n));
</script>

{#if !sessions.length}
  <div class="card empty">No stock counts yet. Count under Adjust / count, or fill a count sheet and import it, and the results appear here.</div>
{:else}
  <div class="stack">
    <div class="toolbar no-print">
      <select class="select day" bind:value={day} aria-label="Count day">
        {#each sessions as s (s.day)}
          <option value={s.day}>{fmtDate(s.day)} · {int(s.products)} products</option>
        {/each}
      </select>
      <label class="check"><input type="checkbox" bind:checked={onlyDifferences} /> Only products with a difference</label>
      <span class="spacer"></span>
      <button class="btn btn-primary" onclick={() => window.print()}>Print</button>
    </div>

    {#if current}
      <div class="grid-stats no-print">
        <div class="card stat"><div class="label">Products counted</div><div class="value">{int(current.products)}</div><div class="sub">{current.baselines ? `${int(current.baselines)} first counts set a baseline · ` : ""}{int(unchanged)} matched</div></div>
        <div class="card stat"><div class="label">Missing</div><div class="value short">{int(current.shortUnits)}</div><div class="sub">units short</div></div>
        <div class="card stat"><div class="label">Missing, at selling price</div><div class="value short">{money(current.shortValue)}</div><div class="sub">{hasCosts ? `${money(current.shortCost)} at cost` : "what they would have sold for"}</div></div>
        <div class="card stat"><div class="label">Found extra</div><div class="value">{int(current.overUnits)}</div><div class="sub">units over · {money(current.overValue)}</div></div>
      </div>
    {/if}

    <div class="card sheet print-area">
      <div class="head">
        <div>
          <div class="title">{session.storeName} · Stock count results · {fmtDate(day)}</div>
          <div class="muted">Counted by {current?.users || "—"} · {int(rows.length)} products counted, {int(shown.length)} shown</div>
          <div class="muted">Expected = last count + received − sold ± adjusted. Difference = counted − expected. A product's first count only sets its baseline.</div>
        </div>
        {#if current}
          <div class="totals">
            <div>Missing: <strong>{int(current.shortUnits)} units · {money(current.shortValue)}</strong> at selling price{#if hasCosts}, {money(current.shortCost)} at cost{/if}</div>
            <div>Found extra: <strong>{int(current.overUnits)} units · {money(current.overValue)}</strong></div>
          </div>
        {/if}
      </div>
      <table class="table">
        <thead>
          <tr>
            <th>Product</th>
            <th class="num" title="The count before this one">Last count</th>
            <th class="num">Sold</th><th class="num">Received</th><th class="num" title="Damage and other adjustments">Adjusted</th>
            <th class="num">Expected</th><th class="num">Counted</th><th class="num">Difference</th>
            <th class="num">At selling price</th>{#if hasCosts}<th class="num">At cost</th>{/if}<th>Note</th><th class="no-print">By</th>
          </tr>
        </thead>
        <tbody>
          {#each shown as r (r.productId)}
            <tr>
              <td class="strong">{r.name}{#if r.category}<span class="cat no-print"> · {r.category}</span>{/if}</td>
              <td class="num">
                {#if r.previousCount === null}<span class="muted" title="Never counted before; the working starts from the first record">first</span>
                {:else}{int(r.previousCount)} <span class="muted">{short(r.previousAt ?? "")}</span>{/if}
              </td>
              <td class="num">{zero(r.sold)}</td>
              <td class="num">{zero(r.received)}</td>
              <td class="num">{r.adjusted === 0 ? "—" : signed(r.adjusted)}</td>
              <td class="num">{int(r.expected)}</td>
              <td class="num">{int(r.counted)}</td>
              {#if isBaseline(r)}
                <td class="num baseline" title="First count of this product: the earlier figure was never a real count, so nothing is missing or found">
                  Baseline set{#if r.difference}<span class="muted"> · was {int(r.expected)}</span>{/if}
                </td>
                <td class="num muted">—</td>
                {#if hasCosts}<td class="num muted">—</td>{/if}
              {:else}
                <td class="num" class:short={r.difference < 0} class:over={r.difference > 0}>{signed(r.difference)}</td>
                <td class="num" class:short={r.difference < 0}>{r.difference ? money(sell(r)) : "—"}</td>
                {#if hasCosts}<td class="num">{r.difference && r.costPrice ? money(cost(r)) : "—"}</td>{/if}
              {/if}
              <td class="muted">{r.note ?? ""}</td>
              <td class="muted no-print">{r.user} · {fmtTime(r.at)}</td>
            </tr>
          {:else}
            <tr><td colspan={hasCosts ? 12 : 11} class="empty">{onlyDifferences ? (current?.baselines === current?.products ? "First count for every product: baselines set, nothing to compare yet." : "Every product counted matched the system.") : "Nothing counted that day."}</td></tr>
          {/each}
        </tbody>
      </table>
    </div>
  </div>
{/if}

<style>
  .day { width: 260px; }
  .head { display: flex; justify-content: space-between; align-items: flex-start; gap: 16px; padding: 14px 16px; border-bottom: 1px solid var(--border); }
  .title { font-weight: 600; font-size: 16px; }
  .totals { font-size: 13px; color: var(--ink-2); text-align: right; display: flex; flex-direction: column; gap: 4px; }
  .strong { font-weight: 500; }
  .short { color: var(--danger); }
  .cat { color: var(--ink-3); font-weight: 400; font-size: 12px; }
  .baseline { white-space: nowrap; color: var(--ink-2); }
  .over { color: var(--success); }
  .stat .value { font-variant-numeric: normal; }
  @media print {
    .sheet { border: 0; box-shadow: none; font-size: 11px; }
    .sheet :global(.table th), .sheet :global(.table td) { padding: 5px 6px; border-bottom: 1px solid #999; }
    .sheet :global(.table th) { background: #eee; color: #000; }
  }
</style>
