<script lang="ts">
  import { api } from "../api";
  import { session, toasts } from "../stores/session.svelte";
  import { reloadShopContext } from "../shopSwitch";

  let username = $state("");
  let pin = $state("");
  let busy = $state(false);

  // No shop is chosen here: the username and PIN decide which shop opens.
  async function submit(e: Event) {
    e.preventDefault();
    if (busy) return;
    busy = true;
    try {
      await api.login(username, pin);
      await reloadShopContext();
      session.screen = "sell";
    } catch (err) {
      toasts.error(err);
      pin = "";
    } finally {
      busy = false;
    }
  }
</script>

<div class="login">
  <form class="card" onsubmit={submit}>
    <div class="brand">
      <span class="mark"></span>
      <!-- On a computer with several shops the name stays neutral until someone has logged in. -->
      <h1>{session.shops?.multi ? "Liquor POS" : session.storeName}</h1>
    </div>
    <p class="muted">Sign in to start selling</p>
    <div class="field">
      <label for="u">Username</label>
      <!-- svelte-ignore a11y_autofocus -->
      <input id="u" class="input input-lg" bind:value={username} autocomplete="username" autofocus />
    </div>
    <div class="field">
      <label for="p">PIN</label>
      <input id="p" class="input input-lg" type="password" inputmode="numeric" bind:value={pin} autocomplete="current-password" />
    </div>
    <button class="btn btn-primary btn-lg" type="submit" disabled={busy || !username || !pin}>Sign in</button>
  </form>
</div>

<style>
  .login { height: 100vh; display: flex; align-items: center; justify-content: center; }
  .card { width: 360px; padding: 32px; display: flex; flex-direction: column; gap: 16px; }
  .brand { display: flex; align-items: center; gap: 10px; }
  .mark { width: 12px; height: 12px; border-radius: 3px; background: var(--accent); }
  .card p { margin-top: -8px; }
</style>
