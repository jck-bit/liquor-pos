<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import type { User } from "../types";
  import { session, toasts } from "../stores/session.svelte";
  import Modal from "../components/Modal.svelte";

  let users = $state<User[]>([]);
  let addOpen = $state(false);
  let pinOpen = $state(false);
  let target = $state<User | null>(null);
  let f = $state({ username: "", pin: "", role: "cashier" });
  let newPin = $state("");
  let busy = $state(false);

  async function load() {
    try {
      users = await api.listUsers();
    } catch (e) {
      toasts.error(e);
    }
  }
  onMount(load);

  async function add(e: Event) {
    e.preventDefault();
    busy = true;
    try {
      await api.createUser(f.username, f.pin, f.role);
      toasts.success(`${f.username} added`);
      addOpen = false;
      f = { username: "", pin: "", role: "cashier" };
      await load();
    } catch (err) {
      toasts.error(err);
    } finally {
      busy = false;
    }
  }
  async function resetPin(e: Event) {
    e.preventDefault();
    if (!target) return;
    busy = true;
    try {
      await api.changePin(target.id, newPin);
      toasts.success(`PIN changed for ${target.username}`);
      pinOpen = false;
      newPin = "";
    } catch (err) {
      toasts.error(err);
    } finally {
      busy = false;
    }
  }
  async function toggle(u: User) {
    try {
      await api.setUserActive(u.id, !u.active);
      await load();
    } catch (e) {
      toasts.error(e);
    }
  }
</script>

<div class="page">
  <div class="page-header">
    <h1>Users</h1>
    <button class="btn btn-primary" onclick={() => (addOpen = true)}>Add user</button>
  </div>
  <div class="page-body">
    <div class="card table-wrap narrow">
      <table class="table">
        <thead><tr><th>Username</th><th>Role</th><th>Status</th><th></th></tr></thead>
        <tbody>
          {#each users as u (u.id)}
            <tr class:dim={!u.active}>
              <td class="strong">{u.username} {#if u.id === session.user?.id}<span class="muted">(you)</span>{/if}</td>
              <td style="text-transform: capitalize">{u.role}</td>
              <td><span class="badge {u.active ? 'badge-green' : 'badge-gray'}">{u.active ? "Active" : "Inactive"}</span></td>
              <td class="actions">
                <button class="btn btn-sm" onclick={() => { target = u; newPin = ""; pinOpen = true; }}>Reset PIN</button>
                {#if u.id !== session.user?.id}
                  <button class="btn btn-ghost btn-sm" onclick={() => toggle(u)}>{u.active ? "Deactivate" : "Activate"}</button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <p class="muted" style="margin-top: 12px">Cashiers can sell and view sales and reports. Owners can also receive deliveries, count and adjust stock, edit products and prices, void sales, manage users and see the audit log. People added on another computer appear here too, but need a PIN set on this computer before they can log in here.</p>
  </div>
</div>

<Modal title="Add user" bind:open={addOpen} width={420}>
  <form id="uform" class="stack" onsubmit={add}>
    <div class="field"><label for="un">Username</label><input id="un" class="input" bind:value={f.username} required autocomplete="off" /></div>
    <div class="field"><label for="up">PIN (4 to 8 digits)</label><input id="up" class="input" type="password" inputmode="numeric" bind:value={f.pin} required /></div>
    <div class="field">
      <label for="ur">Role</label>
      <select id="ur" class="select" bind:value={f.role}><option value="cashier">Cashier</option><option value="owner">Owner</option></select>
    </div>
  </form>
  {#snippet footer()}
    <button class="btn" onclick={() => (addOpen = false)}>Cancel</button>
    <button class="btn btn-primary" type="submit" form="uform" disabled={busy}>Add user</button>
  {/snippet}
</Modal>

<Modal title="Reset PIN for {target?.username ?? ''}" bind:open={pinOpen} width={380}>
  <form id="pinform" class="stack" onsubmit={resetPin}>
    <div class="field"><label for="np">New PIN</label><input id="np" class="input" type="password" inputmode="numeric" bind:value={newPin} required /></div>
  </form>
  {#snippet footer()}
    <button class="btn" onclick={() => (pinOpen = false)}>Cancel</button>
    <button class="btn btn-primary" type="submit" form="pinform" disabled={busy}>Save PIN</button>
  {/snippet}
</Modal>

<style>
  .narrow { max-width: 720px; }
  .strong { font-weight: 500; }
</style>
