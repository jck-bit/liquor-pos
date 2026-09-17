<script lang="ts">
  import type { ShopOverview } from "../../types";
  import { ago } from "../../format";

  let { shops }: { shops: ShopOverview[] } = $props();

  type State = { dot: "ok" | "busy" | "off" | "idle"; text: string; title?: string };
  const TEN_MINUTES = 10 * 60 * 1000;

  function stateOf(s: ShopOverview): State {
    if (s.locked) return { dot: "idle", text: "Locked", title: s.error ?? undefined };
    if (s.error) return { dot: "off", text: "Cannot be read", title: s.error };
    if (s.syncing) return { dot: "busy", text: "Syncing…" };
    if (!s.connected) return { dot: "idle", text: "Not connected to the cloud" };
    const last = s.figures?.lastSynced;
    if (s.syncError) return { dot: "off", text: last ? `Sync problem · updated ${ago(last)}` : "Sync problem", title: s.syncError };
    if (!last) return { dot: "busy", text: "Waiting for first sync" };
    const fresh = Date.now() - new Date(last.replace(" ", "T")).getTime() < TEN_MINUTES;
    return { dot: fresh ? "ok" : "busy", text: `Updated ${ago(last)}` };
  }
</script>

<div class="fresh">
  {#each shops as s (s.id)}
    {@const st = stateOf(s)}
    <div class="chip" title={st.title}>
      <span class="dot {st.dot}"></span>
      <span class="name">{s.name}</span>
      {#if s.isOpen}<span class="badge badge-gray">open</span>{/if}
      <span class="muted">{st.text}</span>
      {#if s.figures && s.figures.pending > 0}<span class="muted">· {s.figures.pending} waiting to upload</span>{/if}
    </div>
  {/each}
</div>

<style>
  .fresh { display: flex; gap: 8px; flex-wrap: wrap; }
  .chip {
    display: flex; align-items: center; gap: 8px; padding: 6px 12px; font-size: 13px;
    background: var(--surface); border: 1px solid var(--border); border-radius: 999px;
  }
  .name { font-weight: 600; }
  .dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; background: var(--ink-3); }
  .dot.ok { background: #16a34a; }
  .dot.busy { background: #d97706; }
  .dot.off { background: #dc2626; }
</style>
