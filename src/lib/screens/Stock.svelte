<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import type { Product, StockMovement } from "../types";
  import { session, toasts } from "../stores/session.svelte";
  import { fmtDateTime, fromCents, money, receiptNo, toCents } from "../format";
  import ProductPicker from "../components/ProductPicker.svelte";

  type Tab = "receive" | "adjust" | "movements" | "low";
  let tab = $state<Tab>("receive");

  // Receive
  type Line = { product: Product; qty: number; cost: string };
  let lines = $state<Line[]>([]);
  let note = $state("");
  let savingReceive = $state(false);
  const receiveTotal = $derived(lines.reduce((s, l) => s + l.qty * toCents(l.cost), 0));

  function addLine(p: Product) {
    const existing = lines.find((l) => l.product.id === p.id);
    if (existing) existing.qty += 1;
    else lines.push({ product: p, qty: 1, cost: fromCents(p.costPrice) });
  }
  async function saveReceive() {
    if (savingReceive || !lines.length) return;
    savingReceive = true;
    try {
      await api.receiveStock(
        lines.map((l) => ({ productId: l.product.id, qty: l.qty, unitCost: l.cost ? toCents(l.cost) : null })),
        note,
      );
      toasts.success(`Received ${lines.reduce((s, l) => s + l.qty, 0)} units`);
      lines = [];
      note = "";
    } catch (e) {
      toasts.error(e);
    } finally {
      savingReceive = false;
    }
  }

  // Adjust (owner)
  let adjProduct = $state<Product | null>(null);
  let adjMode = $state<"count" | "damage" | "adjustment">("count");
  let adjValue = $state("");
  let adjNote = $state("");
  let savingAdj = $state(false);
  const adjDelta = $derived.by(() => {
    const n = parseInt(adjValue);
    if (!adjProduct || !Number.isFinite(n)) return 0;
    if (adjMode === "count") return n - adjProduct.stockQty;
    if (adjMode === "damage") return -Math.abs(n);
    return n;
  });
  async function saveAdjust() {
    if (!adjProduct || savingAdj) return;
    savingAdj = true;
    try {
      const counted = adjMode === "count" ? parseInt(adjValue) : undefined;
      adjProduct = await api.adjustStock(adjProduct.id, adjDelta, adjMode, adjNote, counted);
      toasts.success(`${adjProduct.name} now at ${adjProduct.stockQty}`);
      adjValue = "";
      adjNote = "";
    } catch (e) {
      toasts.error(e);
    } finally {
      savingAdj = false;
    }
  }

  // Movements + low stock
  let movements = $state<StockMovement[]>([]);
  let low = $state<Product[]>([]);
  async function loadTab(t: Tab) {
    tab = t;
    try {
      if (t === "movements") movements = await api.listStockMovements(undefined, 300);
      if (t === "low") low = await api.lowStockProducts();
    } catch (e) {
      toasts.error(e);
    }
  }
  onMount(() => loadTab(tab));

  const reasonLabel: Record<string, string> = { sale: "Sale", void: "Void", purchase: "Delivery", adjustment: "Adjustment", damage: "Damage", count: "Stock count" };
</script>

<div class="page">
  <div class="page-header">
    <h1>Stock</h1>
    <div class="tabs">
      <button class:on={tab === "receive"} onclick={() => loadTab("receive")}>Receive delivery</button>
      <button class:on={tab === "adjust"} onclick={() => loadTab("adjust")}>Adjust / count</button>
      <button class:on={tab === "movements"} onclick={() => loadTab("movements")}>Movements</button>
      <button class:on={tab === "low"} onclick={() => loadTab("low")}>Low stock</button>
    </div>
  </div>

  <div class="page-body">
    {#if tab === "receive"}
      <div class="stack narrow">
        <ProductPicker onpick={addLine} placeholder="Scan or search a product to add it to this delivery" large />
        <div class="card">
          <div class="table-wrap">
            <table class="table">
              <thead><tr><th>Product</th><th class="num">In stock</th><th class="num">Qty received</th><th class="num">Unit cost</th><th class="num">Line cost</th><th></th></tr></thead>
              <tbody>
                {#each lines as l, i (l.product.id)}
                  <tr>
                    <td class="strong">{l.product.name}</td>
                    <td class="num muted">{l.product.stockQty}</td>
                    <td class="num"><input class="input num w80" type="number" min="1" bind:value={l.qty} /></td>
                    <td class="num"><input class="input num w110" bind:value={l.cost} inputmode="decimal" /></td>
                    <td class="num">{money(l.qty * toCents(l.cost))}</td>
                    <td class="actions"><button class="btn btn-ghost btn-sm" onclick={() => lines.splice(i, 1)}>✕</button></td>
                  </tr>
                {:else}
                  <tr><td colspan="6" class="empty">Scan each product from the delivery, then set the quantities.</td></tr>
                {/each}
              </tbody>
            </table>
          </div>
          <div class="card-footer">
            <input class="input" placeholder="Note, e.g. supplier or invoice number" bind:value={note} />
            <span class="num total">Total {money(receiveTotal)}</span>
            <button class="btn btn-primary" disabled={!lines.length || savingReceive} onclick={saveReceive}>Save delivery</button>
          </div>
        </div>
      </div>

    {:else if tab === "adjust"}
      <div class="stack narrow">
        <ProductPicker onpick={(p) => (adjProduct = p)} placeholder="Scan or search the product to adjust" large />
        {#if adjProduct}
          <div class="card card-body stack">
            <div class="row between">
              <div>
                <div class="strong big">{adjProduct.name}</div>
                <div class="muted">Currently {adjProduct.stockQty} in stock</div>
              </div>
            </div>
            <div class="seg">
              <button class:on={adjMode === "count"} onclick={() => (adjMode = "count")}>Set counted quantity</button>
              <button class:on={adjMode === "damage"} onclick={() => (adjMode = "damage")}>Remove damaged / expired</button>
              <button class:on={adjMode === "adjustment"} onclick={() => (adjMode = "adjustment")}>Other (+/−)</button>
            </div>
            <div class="grid-2">
              <div class="field">
                <label for="av">{adjMode === "count" ? "Counted quantity" : adjMode === "damage" ? "Units to remove" : "Change (negative to remove)"}</label>
                <input id="av" class="input num" type="number" bind:value={adjValue} />
              </div>
              <div class="field">
                <label for="an">Reason / note</label>
                <input id="an" class="input" bind:value={adjNote} placeholder="Required for the audit trail" />
              </div>
            </div>
            <div class="row between">
              <span class="muted">Stock will go from {adjProduct.stockQty} to <strong>{adjProduct.stockQty + adjDelta}</strong> ({adjDelta >= 0 ? "+" : ""}{adjDelta})</span>
              <button class="btn btn-primary" disabled={(adjMode === "count" ? !Number.isFinite(parseInt(adjValue)) : adjDelta === 0) || !adjNote.trim() || savingAdj} onclick={saveAdjust}>Apply</button>
            </div>
          </div>
        {/if}
      </div>

    {:else if tab === "movements"}
      <div class="card table-wrap">
        <table class="table">
          <thead><tr><th>When</th><th>Product</th><th>Type</th><th class="num">Change</th><th>Note</th><th>By</th></tr></thead>
          <tbody>
            {#each movements as m (m.id)}
              <tr>
                <td class="muted">{fmtDateTime(m.createdAt)}</td>
                <td class="strong">{m.productName}</td>
                <td><span class="badge {m.qtyDelta < 0 ? 'badge-gray' : 'badge-green'}">{reasonLabel[m.reason] ?? m.reason}</span></td>
                <td class="num" class:neg={m.qtyDelta < 0}>{m.qtyDelta > 0 ? "+" : ""}{m.qtyDelta}</td>
                <td class="muted">{m.refReceiptNo ? `Receipt ${receiptNo(m.refReceiptNo)}` : m.note ?? ""}</td>
                <td>{m.user}{#if m.till} <span class="muted">on {m.till}</span>{/if}</td>
              </tr>
            {:else}
              <tr><td colspan="6" class="empty">No stock movements yet.</td></tr>
            {/each}
          </tbody>
        </table>
      </div>

    {:else}
      <div class="card table-wrap">
        <table class="table">
          <thead><tr><th>Product</th><th>Category</th><th class="num">In stock</th><th class="num">Warn at</th></tr></thead>
          <tbody>
            {#each low as p (p.id)}
              <tr>
                <td class="strong">{p.name}</td>
                <td>{p.category ?? ""}</td>
                <td class="num"><span class="badge {p.stockQty <= 0 ? 'badge-red' : 'badge-amber'}">{p.stockQty}</span></td>
                <td class="num muted">{p.reorderLevel}</td>
              </tr>
            {:else}
              <tr><td colspan="4" class="empty">Nothing is running low.</td></tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
</div>

<style>
  .seg button { border: 0; background: transparent; padding: 6px 12px; border-radius: 4px; cursor: pointer; font-weight: 500; color: var(--ink-2); }
  .seg button.on { background: var(--surface); color: var(--ink); box-shadow: var(--shadow); }
  .seg { display: inline-flex; gap: 2px; background: var(--surface-2); border: 1px solid var(--border); border-radius: var(--radius); padding: 2px; align-self: flex-start; }
  .narrow { max-width: 960px; }
  .strong { font-weight: 500; }
  .big { font-size: 16px; }
  .w80 { width: 80px; height: 30px; }
  .w110 { width: 110px; height: 30px; }
  .card-footer { display: flex; gap: 12px; align-items: center; padding: 12px 16px; border-top: 1px solid var(--border); }
  .card-footer .input { max-width: 380px; }
  .total { margin-left: auto; font-weight: 600; }
  .between { justify-content: space-between; }
  .neg { color: var(--danger); }
</style>
