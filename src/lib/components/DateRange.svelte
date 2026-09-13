<script lang="ts">
  import { addDays, isoDate, monthStart, weekStart } from "../format";

  let { from = $bindable(isoDate()), to = $bindable(isoDate()) }: { from?: string; to?: string } = $props();

  const today = isoDate();
  const presets: { label: string; range: () => [string, string] }[] = [
    { label: "Today", range: () => [today, today] },
    { label: "Yesterday", range: () => [addDays(today, -1), addDays(today, -1)] },
    { label: "This week", range: () => [weekStart(today), today] },
    { label: "Last week", range: () => [addDays(weekStart(today), -7), addDays(weekStart(today), -1)] },
    { label: "This month", range: () => [monthStart(today), today] },
    { label: "Last 30 days", range: () => [addDays(today, -29), today] },
  ];

  const isActive = (p: (typeof presets)[number]) => {
    const [f, t] = p.range();
    return f === from && t === to;
  };
</script>

<div class="range">
  <div class="presets">
    {#each presets as p (p.label)}
      <button class="btn btn-sm" class:on={isActive(p)} onclick={() => ([from, to] = p.range())}>{p.label}</button>
    {/each}
  </div>
  <input class="input date" type="date" bind:value={from} max={to} />
  <span class="muted">to</span>
  <input class="input date" type="date" bind:value={to} min={from} max={today} />
</div>

<style>
  .range { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .presets { display: flex; gap: 4px; }
  .btn.on { background: var(--ink); color: #fff; border-color: var(--ink); }
  .date { width: 150px; }
</style>
