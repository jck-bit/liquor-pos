<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import type { AllShopsOverview } from "../types";
  import { toasts } from "../stores/session.svelte";
  import { isoDate, weekStart } from "../format";
  import DateRange from "../components/DateRange.svelte";
  import Freshness from "./allshops/Freshness.svelte";
  import Overview from "./allshops/Overview.svelte";
  import StockMatrix from "./allshops/StockMatrix.svelte";
  import SalesList from "./allshops/SalesList.svelte";

  type Tab = "overview" | "stock" | "sales";
  let tab = $state<Tab>("overview");
  // Opens on this week, like Reports.
  let from = $state(weekStart(isoDate()));
  let to = $state(isoDate());
  let overview = $state<AllShopsOverview | null>(null);
  /** Bumped after every load so the Stock and Sales tabs reload with it. */
  let tick = $state(0);

  let loading = false;
  let again = false;
  let justAsked = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let alive = true;

  async function load() {
    if (loading) {
      again = true;
      return;
    }
    loading = true;
    try {
      overview = await api.allShopsOverview(from, to);
      tick += 1;
    } catch (e) {
      toasts.error(e);
    } finally {
      loading = false;
      if (again) {
        again = false;
        load();
      } else schedule();
    }
  }

  /** Ask the background sync to fetch every shop now; the figures follow a moment later. */
  async function refresh() {
    justAsked = true;
    await api.refreshAllShops().catch(() => {});
    schedule();
  }

  // Shortly after asking, every 5 s while a shop is syncing, otherwise ask again every 30 s.
  function schedule() {
    clearTimeout(timer);
    if (!alive) return;
    const syncing = overview?.shops.some((s) => s.syncing) ?? false;
    if (justAsked) {
      justAsked = false;
      timer = setTimeout(load, 1500);
    } else if (syncing) timer = setTimeout(load, 5000);
    else timer = setTimeout(refresh, 30000);
  }

  onMount(() => {
    refresh();
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  });

  // Runs on mount and whenever the date range changes.
  $effect(() => {
    void from;
    void to;
    load();
  });
</script>

<div class="page">
  <div class="page-header">
    <div class="row">
      <h1>All shops</h1>
      <div class="tabs">
        <button class:on={tab === "overview"} onclick={() => (tab = "overview")}>Overview</button>
        <button class:on={tab === "stock"} onclick={() => (tab = "stock")}>Stock</button>
        <button class:on={tab === "sales"} onclick={() => (tab = "sales")}>Sales</button>
      </div>
    </div>
    <div class="toolbar">
      {#if tab !== "stock"}<DateRange bind:from bind:to />{/if}
      <button class="btn" onclick={refresh}>Refresh</button>
    </div>
  </div>

  <div class="page-body stack">
    {#if overview}
      <Freshness shops={overview.shops} />
      {#if tab === "overview"}
        <Overview {overview} />
      {:else if tab === "stock"}
        <StockMatrix {tick} />
      {:else}
        <SalesList {from} {to} {tick} shops={overview.shops} />
      {/if}
    {:else}
      <div class="empty">Loading every shop…</div>
    {/if}
  </div>
</div>
