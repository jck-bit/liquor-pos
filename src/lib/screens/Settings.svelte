<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import { session, toasts } from "../stores/session.svelte";

  let f = $state({ store_name: "", store_phone: "", store_address: "", receipt_footer: "", allow_negative_stock: false });
  let dbPath = $state("");
  let lastBackup = $state("");
  let busy = $state(false);
  let myPin = $state("");

  onMount(async () => {
    const s = session.settings;
    f = {
      store_name: s.store_name ?? "",
      store_phone: s.store_phone ?? "",
      store_address: s.store_address ?? "",
      receipt_footer: s.receipt_footer ?? "",
      allow_negative_stock: s.allow_negative_stock === "1",
    };
    try {
      dbPath = await api.databasePath();
    } catch (e) {
      toasts.error(e);
    }
  });

  async function save(e: Event) {
    e.preventDefault();
    busy = true;
    try {
      await api.updateSettings({ ...f, allow_negative_stock: f.allow_negative_stock ? "1" : "0" });
      session.settings = await api.getSettings();
      toasts.success("Settings saved");
    } catch (err) {
      toasts.error(err);
    } finally {
      busy = false;
    }
  }
  async function backup() {
    busy = true;
    try {
      lastBackup = await api.backupDatabase();
      toasts.success("Backup saved");
    } catch (e) {
      toasts.error(e);
    } finally {
      busy = false;
    }
  }
  async function changeMyPin(e: Event) {
    e.preventDefault();
    if (!session.user) return;
    try {
      await api.changePin(session.user.id, myPin);
      myPin = "";
      toasts.success("Your PIN was changed");
    } catch (err) {
      toasts.error(err);
    }
  }
</script>

<div class="page">
  <div class="page-header"><h1>Settings</h1></div>
  <div class="page-body">
    <div class="cols">
      <form class="card card-body stack" onsubmit={save}>
        <h2>Store</h2>
        <div class="field"><label for="sn">Store name</label><input id="sn" class="input" bind:value={f.store_name} required /></div>
        <div class="grid-2">
          <div class="field"><label for="sp">Phone</label><input id="sp" class="input" bind:value={f.store_phone} /></div>
          <div class="field"><label for="sa">Address</label><input id="sa" class="input" bind:value={f.store_address} /></div>
        </div>
        <div class="field"><label for="sf">Receipt footer</label><input id="sf" class="input" bind:value={f.receipt_footer} /></div>
        <label class="check">
          <input type="checkbox" bind:checked={f.allow_negative_stock} />
          Allow selling when stock shows zero (stock can go negative)
        </label>
        <div><button class="btn btn-primary" type="submit" disabled={busy}>Save settings</button></div>
      </form>

      <div class="stack">
        <div class="card card-body stack">
          <h2>Backup</h2>
          <p class="muted">Makes a copy of the whole database in your Documents folder under "Liquor POS / backups". Copy that folder to a USB stick or Google Drive regularly.</p>
          <div><button class="btn" onclick={backup} disabled={busy}>Back up now</button></div>
          {#if lastBackup}<p class="mono small">{lastBackup}</p>{/if}
          <p class="muted small">Live database: <span class="mono">{dbPath}</span></p>
        </div>
        <form class="card card-body stack" onsubmit={changeMyPin}>
          <h2>Your PIN</h2>
          <div class="field"><label for="mp">New PIN for {session.user?.username}</label><input id="mp" class="input" type="password" inputmode="numeric" bind:value={myPin} required /></div>
          <div><button class="btn" type="submit">Change my PIN</button></div>
        </form>
      </div>
    </div>
  </div>
</div>

<style>
  .cols { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; max-width: 1100px; }
  .small { font-size: 12px; word-break: break-all; }
</style>
