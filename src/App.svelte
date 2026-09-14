<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "./lib/api";
  import { session, syncState, toasts } from "./lib/stores/session.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import Toasts from "./lib/components/Toasts.svelte";
  import UpdateBanner from "./lib/components/UpdateBanner.svelte";
  import Login from "./lib/screens/Login.svelte";
  import Sell from "./lib/screens/Sell.svelte";
  import Products from "./lib/screens/Products.svelte";
  import Stock from "./lib/screens/Stock.svelte";
  import Sales from "./lib/screens/Sales.svelte";
  import Reports from "./lib/screens/Reports.svelte";
  import Audit from "./lib/screens/Audit.svelte";
  import Users from "./lib/screens/Users.svelte";
  import Settings from "./lib/screens/Settings.svelte";

  let ready = $state(false);

  async function pollSync() {
    try {
      syncState.status = await api.syncStatus();
    } catch {
      /* backend not ready yet */
    }
  }

  onMount(() => {
    (async () => {
      try {
        session.shops = await api.listShops();
        session.settings = await api.getSettings();
        session.user = await api.currentUser();
      } catch (e) {
        toasts.error(e);
      }
      ready = true;
      pollSync();
    })();
    const t = setInterval(pollSync, 10_000);
    return () => clearInterval(t);
  });
</script>

{#if ready}
  {#if !session.user}
    <Login />
  {:else}
    <div class="shell">
      <Sidebar />
      <main class="main">
        <UpdateBanner />
        {#if syncState.stale}
          <div class="stale">
            <strong>Sales are not reaching the cloud.</strong>
            {syncState.status?.pending} changes are waiting. Check that this computer is connected to the internet.
            {#if syncState.status?.lastError}<span class="err">({syncState.status.lastError})</span>{/if}
          </div>
        {/if}
        {#if session.screen === "sell"}
          <Sell />
        {:else if session.screen === "products"}
          <Products />
        {:else if session.screen === "stock"}
          <Stock />
        {:else if session.screen === "sales"}
          <Sales />
        {:else if session.screen === "reports"}
          <Reports />
        {:else if session.screen === "audit"}
          <Audit />
        {:else if session.screen === "users"}
          <Users />
        {:else if session.screen === "settings"}
          <Settings />
        {/if}
      </main>
    </div>
  {/if}
{/if}
<Toasts />

<style>
  .shell { display: flex; height: 100vh; }
  .main { flex: 1; min-width: 0; height: 100vh; overflow: hidden; display: flex; flex-direction: column; }
  .main > :global(.page) { flex: 1; min-height: 0; }
  .stale { padding: 8px 16px; background: var(--danger); color: #fff; font-size: 13px; }
  .stale .err { opacity: 0.85; }
  .main > :global(.sell) { flex: 1; min-height: 0; }
</style>
