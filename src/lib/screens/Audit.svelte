<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import type { AuditEntry } from "../types";
  import { toasts } from "../stores/session.svelte";
  import { fmtDateTime, money } from "../format";

  let entries = $state<AuditEntry[]>([]);
  let done = $state(false);
  const PAGE = 200;

  async function load(more = false) {
    try {
      const batch = await api.listAudit(PAGE, more ? entries.length : 0);
      entries = more ? [...entries, ...batch] : batch;
      done = batch.length < PAGE;
    } catch (e) {
      toasts.error(e);
    }
  }
  onMount(() => load());

  const moneyKeys = new Set(["total", "discount", "sellPrice", "costPrice"]);
  function describe(e: AuditEntry): string {
    let d: Record<string, unknown> = {};
    try { d = JSON.parse(e.details); } catch { return e.details; }
    return Object.entries(d)
      .filter(([, v]) => v !== null && v !== undefined && v !== "")
      .map(([k, v]) => {
        if (Array.isArray(v) && v.length === 2) {
          const f = (x: unknown) => (moneyKeys.has(k) && typeof x === "number" ? money(x) : String(x ?? "—"));
          return `${k}: ${f(v[0])} → ${f(v[1])}`;
        }
        if (moneyKeys.has(k) && typeof v === "number") return `${k}: ${money(v)}`;
        return `${k}: ${typeof v === "object" ? JSON.stringify(v) : String(v)}`;
      })
      .join("  ·  ");
  }
  const actionClass: Record<string, string> = { sale: "badge-green", void: "badge-red", deactivate: "badge-red", login: "badge-gray", logout: "badge-gray" };
</script>

<div class="page">
  <div class="page-header">
    <h1>Audit log</h1>
    <span class="muted">Every sale, void, price change, stock adjustment and login. Cannot be edited.</span>
  </div>
  <div class="page-body stack">
    <div class="card table-wrap">
      <table class="table">
        <thead><tr><th>When</th><th>Who</th><th>Action</th><th>On</th><th>Details</th></tr></thead>
        <tbody>
          {#each entries as e (e.id)}
            <tr>
              <td class="muted nowrap">{fmtDateTime(e.createdAt)}</td>
              <td>{e.user ?? "—"}{#if e.till} <span class="muted">on {e.till}</span>{/if}</td>
              <td><span class="badge {actionClass[e.action] ?? 'badge-amber'}">{e.action.replace("_", " ")}</span></td>
              <td class="nowrap">{e.entity}{e.entityId ? ` #${e.entityId}` : ""}</td>
              <td class="details">{describe(e)}</td>
            </tr>
          {:else}
            <tr><td colspan="5" class="empty">Nothing logged yet.</td></tr>
          {/each}
        </tbody>
      </table>
    </div>
    {#if !done}
      <button class="btn" style="align-self: center" onclick={() => load(true)}>Load older entries</button>
    {/if}
  </div>
</div>

<style>
  .nowrap { white-space: nowrap; }
  .details { color: var(--ink-2); font-size: 13px; }
</style>
