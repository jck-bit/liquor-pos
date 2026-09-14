<script lang="ts">
  import { api } from "../api";
  import type { PaymentMethod, Sale, SaleDetail } from "../types";
  import { session, toasts } from "../stores/session.svelte";
  import { fmtDateTime, isoDate, money, receiptNo } from "../format";
  import DateRange from "../components/DateRange.svelte";
  import Modal from "../components/Modal.svelte";
  import Receipt from "../components/Receipt.svelte";

  let from = $state(isoDate());
  let to = $state(isoDate());
  let method = $state<"" | PaymentMethod>("");
  let sales = $state<Sale[]>([]);
  let detail = $state<SaleDetail | null>(null);
  let open = $state(false);
  let voidReason = $state("");
  let voiding = $state(false);

  const total = $derived(sales.filter((s) => s.status === "completed").reduce((t, s) => t + s.total, 0));

  async function load() {
    try {
      sales = await api.listSales(from, to, method || undefined);
    } catch (e) {
      toasts.error(e);
    }
  }
  // $effect runs once on mount and again whenever the range changes.
  $effect(() => {
    void from; void to; void method;
    load();
  });

  async function show(s: Sale) {
    try {
      detail = await api.getSale(s.id);
      voidReason = "";
      open = true;
    } catch (e) {
      toasts.error(e);
    }
  }

  async function doVoid() {
    if (!detail || voiding) return;
    voiding = true;
    try {
      detail = await api.voidSale(detail.sale.id, voidReason);
      toasts.success(`Sale ${receiptNo(detail.sale.receiptNo)} voided, stock restored`);
      await load();
    } catch (e) {
      toasts.error(e);
    } finally {
      voiding = false;
    }
  }
</script>

<div class="page">
  <div class="page-header">
    <h1>Sales</h1>
    <div class="toolbar">
      <DateRange bind:from bind:to />
      <select class="select method" bind:value={method}>
        <option value="">All payments</option>
        <option value="cash">Cash</option>
        <option value="mpesa">M-Pesa</option>
      </select>
    </div>
  </div>
  <div class="page-body">
    <div class="card table-wrap">
      <table class="table">
        <thead><tr><th>Receipt</th><th>When</th><th>Till</th><th>Cashier</th><th class="num">Items</th><th>Payment</th><th>M-Pesa code</th><th class="num">Total</th><th></th></tr></thead>
        <tbody>
          {#each sales as s (s.id)}
            <tr class="clickable" class:dim={s.status === "voided"} onclick={() => show(s)}>
              <td class="mono">{receiptNo(s.receiptNo)}</td>
              <td>{fmtDateTime(s.createdAt)}</td>
              <td class="muted">{s.till ?? ""}</td>
              <td>{s.cashier}</td>
              <td class="num">{s.itemCount}</td>
              <td><span class="badge {s.paymentMethod === 'mpesa' ? 'badge-green' : 'badge-gray'}">{s.paymentMethod === "mpesa" ? "M-Pesa" : "Cash"}</span></td>
              <td class="mono">{s.mpesaCode ?? ""}</td>
              <td class="num strong">{money(s.total)}</td>
              <td>{#if s.status === "voided"}<span class="badge badge-red">Voided</span>{/if}</td>
            </tr>
          {:else}
            <tr><td colspan="9" class="empty">No sales in this period.</td></tr>
          {/each}
        </tbody>
        {#if sales.length}
          <tfoot><tr><td colspan="7" class="right muted">{sales.filter((s) => s.status === "completed").length} completed sales</td><td class="num strong">{money(total)}</td><td></td></tr></tfoot>
        {/if}
      </table>
    </div>
  </div>
</div>

<Modal title={detail ? `Receipt ${receiptNo(detail.sale.receiptNo)}` : ""} bind:open width={720}>
  {#if detail}
    <div class="detail">
      <Receipt {detail} />
      <div class="side stack">
        <div class="kv"><span class="muted">Status</span><span>{detail.sale.status === "voided" ? "Voided" : "Completed"}</span></div>
        {#if detail.sale.status === "voided"}
          <div class="kv"><span class="muted">Reason</span><span>{detail.sale.voidReason}</span></div>
        {:else if session.isOwner}
          <div class="field">
            <label for="vr">Void this sale</label>
            <input id="vr" class="input" bind:value={voidReason} placeholder="Reason (required)" />
            <span class="muted">Items go back into stock. The sale stays in the records marked as voided.</span>
          </div>
          <button class="btn btn-danger" disabled={!voidReason.trim() || voiding} onclick={doVoid}>Void sale</button>
        {/if}
      </div>
    </div>
  {/if}
  {#snippet footer()}
    <button class="btn" onclick={() => window.print()}>Print</button>
    <button class="btn btn-primary" onclick={() => (open = false)}>Close</button>
  {/snippet}
</Modal>

<style>
  .method { width: 150px; }
  .strong { font-weight: 500; }
  .detail { display: flex; gap: 24px; align-items: flex-start; }
  .side { flex: 1; }
  .kv { display: flex; justify-content: space-between; }
  tfoot td { background: var(--surface-2); font-weight: 500; }
</style>
