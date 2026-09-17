<script lang="ts">
  import { api } from "../../api";
  import type { PaymentMethod, ShopSale, ShopSaleDetail } from "../../types";
  import { toasts } from "../../stores/session.svelte";
  import { fmtDateTime, int, money, receiptNo } from "../../format";
  import Modal from "../../components/Modal.svelte";
  import Receipt from "../../components/Receipt.svelte";

  let { from, to, tick, shopId }: { from: string; to: string; tick: number; shopId: string } = $props();

  let method = $state<"" | PaymentMethod>("");
  let sales = $state<ShopSale[]>([]);
  let picked = $state<ShopSaleDetail | null>(null);
  let open = $state(false);

  $effect(() => {
    void tick;
    api.allShopsSales(from, to, shopId || undefined, method || undefined).then((s) => (sales = s), (e) => toasts.error(e));
  });

  const completed = $derived(sales.filter((s) => s.status === "completed"));
  const total = $derived(completed.reduce((t, s) => t + s.total, 0));

  async function show(s: ShopSale) {
    try {
      picked = await api.shopSaleDetail(s.shopId, s.id);
      open = true;
    } catch (e) {
      toasts.error(e);
    }
  }
</script>

<div class="toolbar">
  <select class="select pick" bind:value={method}>
    <option value="">All payments</option>
    <option value="cash">Cash</option>
    <option value="mpesa">M-Pesa</option>
  </select>
</div>

<div class="card table-wrap">
  <table class="table">
    <thead>
      <tr><th>Shop</th><th>Receipt</th><th>When</th><th>Till</th><th>Cashier</th><th class="num">Items</th><th>Payment</th><th>M-Pesa code</th><th class="num">Total</th><th></th></tr>
    </thead>
    <tbody>
      <!-- Receipt ids repeat from shop to shop, so the key includes the shop. -->
      {#each sales as s (`${s.shopId}:${s.id}`)}
        <tr class="clickable" class:dim={s.status === "voided"} onclick={() => show(s)}>
          <td class="strong">{s.shopName}</td>
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
        <tr><td colspan="10" class="empty">No sales in this period.</td></tr>
      {/each}
    </tbody>
    {#if sales.length}
      <tfoot><tr><td colspan="8" class="right muted">{int(completed.length)} completed sales</td><td class="num strong">{money(total)}</td><td></td></tr></tfoot>
    {/if}
  </table>
</div>

<Modal title={picked ? `${picked.shopName} · receipt ${receiptNo(picked.detail.sale.receiptNo)}` : ""} bind:open width={420}>
  {#if picked}
    <div class="receipt-wrap"><Receipt detail={picked.detail} store={picked.store} /></div>
    <p class="muted hint">To void this sale, open {picked.shopName} and find it under Sales.</p>
  {/if}
  {#snippet footer()}
    <button class="btn" onclick={() => window.print()}>Print</button>
    <button class="btn btn-primary" onclick={() => (open = false)}>Close</button>
  {/snippet}
</Modal>

<style>
  .pick { width: 170px; }
  .strong { font-weight: 500; }
  .receipt-wrap { display: flex; justify-content: center; }
  .hint { margin-top: 12px; text-align: center; font-size: 13px; }
  tfoot td { background: var(--surface-2); font-weight: 500; }
</style>
