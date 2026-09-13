<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "./lib/api";
  import { session, toasts } from "./lib/stores/session.svelte";
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

  onMount(async () => {
    try {
      session.settings = await api.getSettings();
      session.user = await api.currentUser();
    } catch (e) {
      toasts.error(e);
    }
    ready = true;
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
  .main > :global(.sell) { flex: 1; min-height: 0; }
</style>
