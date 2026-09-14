<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import { session, toasts } from "../stores/session.svelte";
  import type { SyncStatus } from "../types";
  import { fmtDateTime } from "../format";

  let sync = $state<SyncStatus | null>(null);
  let sf = $state({ url: "", anonKey: "", email: "", password: "" });
  let syncBusy = $state(false);

  async function loadSync() {
    try {
      sync = await api.syncStatus();
      const s = session.settings;
      sf.url = s.supabase_url ?? "";
      sf.anonKey = s.supabase_anon_key ?? "";
      sf.email = s.sync_email ?? "";
    } catch (e) {
      toasts.error(e);
    }
  }
  async function connectSync(e: Event) {
    e.preventDefault();
    syncBusy = true;
    try {
      sync = await api.configureSync(sf.url, sf.anonKey, sf.email, sf.password);
      session.settings = await api.getSettings();
      sf.password = "";
      toasts.success("Connected. First sync is running in the background.");
    } catch (err) {
      toasts.error(err);
    } finally {
      syncBusy = false;
    }
  }
  async function disconnectSync() {
    syncBusy = true;
    try {
      await api.disableSync();
      session.settings = await api.getSettings();
      await loadSync();
      toasts.success("Cloud sync turned off");
    } catch (err) {
      toasts.error(err);
    } finally {
      syncBusy = false;
    }
  }
  async function syncNow() {
    try {
      await api.syncNow();
      setTimeout(loadSync, 2500);
    } catch (err) {
      toasts.error(err);
    }
  }

  let f = $state({ store_name: "", device_name: "", store_phone: "", store_address: "", receipt_footer: "", allow_negative_stock: false });
  let dbPath = $state("");
  let lastBackup = $state("");
  let busy = $state(false);
  let myPin = $state("");

  onMount(async () => {
    const s = session.settings;
    f = {
      store_name: s.store_name ?? "",
      device_name: s.device_name ?? "",
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
    await loadSync();
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
        <div class="field">
          <label for="dn">This computer's name</label>
          <input id="dn" class="input" bind:value={f.device_name} placeholder="e.g. Counter till" />
          <span class="muted">Shown in the Till column on the store's other computers.</span>
        </div>
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
        <form class="card card-body stack" onsubmit={connectSync}>
          <div class="row between">
            <h2>Cloud sync (Supabase)</h2>
            {#if sync?.configured}
              <span class="badge {sync.connected ? 'badge-green' : 'badge-red'}">{sync.connected ? "Connected" : "Not reachable"}</span>
            {/if}
          </div>
          <p class="muted">Sales, stock and products are saved here first and copied to the cloud whenever there is internet. The till keeps working offline.</p>
          {#if sync?.configured}
            <div class="kv"><span class="muted">Account</span><span>{sync.email}</span></div>
            <div class="kv"><span class="muted">Waiting to upload</span><span class="num">{sync.pending}</span></div>
            <div class="kv"><span class="muted">Last successful sync</span><span>{sync.lastOk ? fmtDateTime(sync.lastOk) : "never"}</span></div>
            {#if sync.lastError}<p class="err">{sync.lastError}</p>{/if}
            <div class="row">
              <button class="btn" type="button" onclick={syncNow}>Sync now</button>
              <button class="btn btn-danger" type="button" onclick={disconnectSync} disabled={syncBusy}>Turn off</button>
            </div>
          {:else}
            <div class="field"><label for="su">Project URL</label><input id="su" class="input mono" bind:value={sf.url} placeholder="https://xxxx.supabase.co" /></div>
            <div class="field"><label for="sk">Anon (public) key</label><input id="sk" class="input mono" bind:value={sf.anonKey} /></div>
            <div class="grid-2">
              <div class="field"><label for="se">Store login email</label><input id="se" class="input" bind:value={sf.email} autocomplete="off" /></div>
              <div class="field"><label for="spw">Password</label><input id="spw" class="input" type="password" bind:value={sf.password} autocomplete="new-password" /></div>
            </div>
            <div><button class="btn btn-primary" type="submit" disabled={syncBusy || !sf.url || !sf.anonKey || !sf.email || !sf.password}>Connect</button></div>
          {/if}
        </form>
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
  .between { justify-content: space-between; }
  .kv { display: flex; justify-content: space-between; }
  .err { color: var(--danger); font-size: 13px; word-break: break-word; }
</style>
