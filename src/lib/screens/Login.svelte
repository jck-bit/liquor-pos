<script lang="ts">
  import { api } from "../api";
  import { session, toasts } from "../stores/session.svelte";

  let username = $state("");
  let pin = $state("");
  let busy = $state(false);
  let switching = $state(false);
  let userInput = $state<HTMLInputElement | null>(null);

  async function submit(e: Event) {
    e.preventDefault();
    if (busy) return;
    busy = true;
    try {
      session.user = await api.login(username, pin);
      session.screen = "sell";
      session.notice = null;
    } catch (err) {
      toasts.error(err);
      pin = "";
    } finally {
      busy = false;
    }
  }

  async function pick(id: string) {
    if (switching || id === session.shops?.current) return;
    switching = true;
    try {
      session.shops = await api.openShop(id);
      session.settings = await api.getSettings();
      pin = "";
      userInput?.focus();
    } catch (err) {
      toasts.error(err);
    } finally {
      switching = false;
    }
  }
</script>

<div class="login">
  <form class="card" onsubmit={submit}>
    <div class="brand">
      <span class="mark"></span>
      <h1>{session.storeName}</h1>
    </div>
    <p class="muted">Sign in to start selling</p>
    {#if session.multiShop && session.shops}
      <div class="field">
        <span class="label" id="shop-label">Shop</span>
        <div class="shops" role="radiogroup" aria-labelledby="shop-label">
          {#each session.shops.shops as s (s.id)}
            <button
              type="button"
              role="radio"
              aria-checked={s.id === session.shops.current}
              class="shop"
              class:on={s.id === session.shops.current}
              disabled={switching}
              onclick={() => pick(s.id)}>{s.name}</button
            >
          {/each}
        </div>
      </div>
    {/if}
    {#if session.notice}<p class="notice">{session.notice}</p>{/if}
    <div class="field">
      <label for="u">Username</label>
      <!-- svelte-ignore a11y_autofocus -->
      <input id="u" bind:this={userInput} class="input input-lg" bind:value={username} autocomplete="username" autofocus />
    </div>
    <div class="field">
      <label for="p">PIN</label>
      <input id="p" class="input input-lg" type="password" inputmode="numeric" bind:value={pin} autocomplete="current-password" />
    </div>
    <button class="btn btn-primary btn-lg" type="submit" disabled={busy || switching || !username || !pin}>Sign in</button>
  </form>
</div>

<style>
  .login { height: 100vh; display: flex; align-items: center; justify-content: center; }
  .card { width: 360px; padding: 32px; display: flex; flex-direction: column; gap: 16px; }
  .brand { display: flex; align-items: center; gap: 10px; }
  .mark { width: 12px; height: 12px; border-radius: 3px; background: var(--accent); }
  .card p.muted { margin-top: -8px; }
  .label { font-size: 12px; font-weight: 500; color: var(--ink-2); }
  .shops { display: grid; grid-template-columns: repeat(auto-fit, minmax(120px, 1fr)); gap: 6px; }
  .shop {
    height: 40px; padding: 0 12px; border-radius: var(--radius); border: 1px solid var(--border-strong);
    background: var(--surface); cursor: pointer; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .shop:hover { background: var(--surface-2); }
  .shop.on { background: var(--ink); border-color: var(--ink); color: #fff; }
  .notice { padding: 10px 12px; border-radius: var(--radius); background: var(--accent-soft); color: #7c2d12; font-size: 13px; }
</style>
