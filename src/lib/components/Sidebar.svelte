<script lang="ts">
  import { api } from "../api";
  import { session, syncState, toasts, type Screen } from "../stores/session.svelte";
  import { cart } from "../stores/cart.svelte";
  import { enterShop } from "../shopSwitch";

  let switching = $state(false);
  let menuOpen = $state(false);

  async function switchTo(id: string) {
    if (switching) return;
    switching = true;
    try {
      await enterShop(id);
      menuOpen = false;
    } catch (e) {
      toasts.error(e);
    } finally {
      switching = false;
    }
  }
  const sync = $derived(syncState.status);
  const syncLabel = $derived.by(() => {
    if (!sync || !sync.configured) return null;
    if (sync.syncing) return { dot: "busy", text: "Syncing…" };
    if (sync.pending > 0 && !sync.connected) return { dot: "off", text: `Offline · ${sync.pending} waiting` };
    if (sync.pending > 0) return { dot: "busy", text: `${sync.pending} waiting to sync` };
    if (sync.lastError) return { dot: "off", text: "Sync error" };
    return { dot: "ok", text: "Cloud synced" };
  });

  // `multi`: only on a computer that holds more than one shop.
  const items: { key: Screen; label: string; owner?: boolean; multi?: boolean }[] = [
    { key: "sell", label: "Sell" },
    { key: "products", label: "Products" },
    { key: "stock", label: "Stock" },
    { key: "sales", label: "Sales" },
    { key: "reports", label: "Reports" },
    { key: "allshops", label: "All shops", owner: true, multi: true },
    { key: "audit", label: "Audit log", owner: true },
    { key: "users", label: "Users", owner: true },
    { key: "settings", label: "Settings", owner: true },
  ];

  async function logout() {
    try {
      await api.logout();
      cart.clear();
      session.user = null;
      session.screen = "sell";
      menuOpen = false;
      session.shops = await api.listShops();
    } catch (e) {
      toasts.error(e);
    }
  }
</script>

<nav class="sidebar">
  <div class="brand">
    <span class="mark"></span>
    <span class="name">{session.storeName}</span>
  </div>
  <ul>
    {#each items as item (item.key)}
      {#if (!item.owner || session.isOwner) && (!item.multi || session.multiShop)}
        <li>
          <button class:active={session.screen === item.key} onclick={() => (session.screen = item.key)}>
            {item.label}
          </button>
        </li>
      {/if}
    {/each}
  </ul>
  {#if syncLabel}
    <button class="sync" title={sync?.lastError ?? sync?.lastOk ?? ""} onclick={() => api.syncNow().catch(() => {})}>
      <span class="dot {syncLabel.dot}"></span>{syncLabel.text}
    </button>
  {/if}
  {#if menuOpen && session.shops}
    <div class="shopmenu" role="menu">
      {#each session.shops.shops as s (s.id)}
        <button
          role="menuitem"
          class:on={s.id === session.shops.current}
          disabled={switching || !s.unlocked || s.id === session.shops.current}
          title={s.unlocked ? "" : `Your login does not open ${s.name}. Sign in with its owner PIN.`}
          onclick={() => switchTo(s.id)}
        >
          <span>{s.name}</span>
          <span class="tag">{s.id === session.shops.current ? "open" : s.unlocked ? "" : "locked"}</span>
        </button>
      {/each}
    </div>
  {/if}
  <div class="user">
    <div>
      <div class="who">{session.user?.username}</div>
      <div class="role">{session.user?.role}</div>
    </div>
    <div class="acts">
      {#if session.isOwner && session.multiShop}
        <button class="logout" onclick={() => (menuOpen = !menuOpen)} aria-expanded={menuOpen}>Switch shop</button>
      {/if}
      <button class="logout" onclick={logout}>Log out</button>
    </div>
  </div>
</nav>

<style>
  .sidebar {
    width: 200px; flex-shrink: 0; background: var(--sidebar); color: var(--sidebar-ink);
    display: flex; flex-direction: column; padding: 18px 12px 14px;
  }
  .brand { display: flex; align-items: center; gap: 10px; padding: 0 8px 18px; color: #fff; font-weight: 600; }
  .mark { width: 10px; height: 10px; border-radius: 3px; background: var(--accent); flex-shrink: 0; }
  .name { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 2px; }
  li button {
    width: 100%; text-align: left; border: 0; background: transparent; color: inherit;
    padding: 8px 10px; border-radius: var(--radius); cursor: pointer; font-weight: 500;
  }
  li button:hover { background: var(--sidebar-active); color: #fff; }
  li button.active { background: var(--sidebar-active); color: #fff; box-shadow: inset 3px 0 0 var(--accent); }
  .sync { margin-top: auto; border: 0; background: transparent; color: inherit; cursor: pointer; text-align: left; font-size: 12px; padding: 6px 8px; display: flex; align-items: center; gap: 8px; border-radius: 4px; }
  .sync:hover { background: var(--sidebar-active); }
  .sync + .user, .shopmenu + .user { margin-top: 0; }
  .shopmenu { margin-top: auto; }
  .sync + .shopmenu { margin-top: 8px; }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: #57534e; flex-shrink: 0; }
  .dot.ok { background: #22c55e; }
  .dot.busy { background: #f59e0b; }
  .dot.off { background: #ef4444; }
  .user { margin-top: auto; padding: 12px 8px 0; border-top: 1px solid #292524; display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .who { color: #fff; font-weight: 500; }
  .role { font-size: 12px; text-transform: capitalize; }
  .shopmenu { display: flex; flex-direction: column; gap: 2px; padding: 6px; margin-bottom: 8px; background: var(--sidebar-active); border-radius: var(--radius); }
  .shopmenu button { display: flex; justify-content: space-between; align-items: center; gap: 8px; border: 0; background: transparent; color: #e7e5e4; padding: 7px 8px; border-radius: 4px; cursor: pointer; text-align: left; font-weight: 500; }
  .shopmenu button:hover:not(:disabled) { background: #3f3a36; color: #fff; }
  .shopmenu button:disabled { cursor: default; opacity: 0.65; }
  .shopmenu button.on { opacity: 1; color: #fff; }
  .shopmenu .tag { font-size: 11px; font-weight: 400; color: var(--sidebar-ink); }
  .acts { display: flex; flex-direction: column; align-items: flex-end; gap: 2px; }
  .logout { border: 0; background: transparent; color: inherit; cursor: pointer; font-size: 13px; padding: 4px 6px; border-radius: 4px; }
  .logout:hover { color: #fff; background: var(--sidebar-active); }
</style>
