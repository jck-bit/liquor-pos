<script lang="ts">
  import type { SaleDetail } from "../types";
  import { session } from "../stores/session.svelte";
  import { fmtDateTime, money, receiptNo } from "../format";

  let { detail }: { detail: SaleDetail } = $props();
  const s = $derived(detail.sale);
  const st = $derived(session.settings);
</script>

<div class="receipt">
  <div class="center">
    <div class="store">{st.store_name || "Liquor POS"}</div>
    {#if st.store_address}<div>{st.store_address}</div>{/if}
    {#if st.store_phone}<div>Tel: {st.store_phone}</div>{/if}
  </div>
  <div class="rule"></div>
  <div class="kv"><span>Receipt</span><span>{receiptNo(s.receiptNo)}</span></div>
  {#if s.till}<div class="kv"><span>Till</span><span>{s.till}</span></div>{/if}
  <div class="kv"><span>Date</span><span>{fmtDateTime(s.createdAt)}</span></div>
  <div class="kv"><span>Served by</span><span>{s.cashier}</span></div>
  <div class="rule"></div>
  <table>
    <tbody>
      {#each detail.items as it (it.id)}
        <tr>
          <td class="name" colspan="3">{it.productName}</td>
        </tr>
        <tr>
          <td class="qty">{it.qty} × {money(it.unitPrice)}</td>
          <td></td>
          <td class="amt">{money(it.lineTotal)}</td>
        </tr>
      {/each}
    </tbody>
  </table>
  <div class="rule"></div>
  {#if s.discount > 0}
    <div class="kv"><span>Subtotal</span><span>{money(s.subtotal)}</span></div>
    <div class="kv"><span>Discount</span><span>-{money(s.discount)}</span></div>
  {/if}
  <div class="kv total"><span>TOTAL</span><span>KES {money(s.total)}</span></div>
  {#if s.paymentMethod === "cash"}
    <div class="kv"><span>Cash</span><span>{money(s.cashTendered ?? s.total)}</span></div>
    <div class="kv"><span>Change</span><span>{money(s.changeGiven ?? 0)}</span></div>
  {:else}
    <div class="kv"><span>M-Pesa</span><span>{s.mpesaCode}</span></div>
  {/if}
  {#if s.status === "voided"}
    <div class="void">*** VOIDED ***</div>
  {/if}
  <div class="rule"></div>
  <div class="center footer">{st.receipt_footer || ""}</div>
</div>

<style>
  .receipt { font-family: var(--mono); font-size: 12px; line-height: 1.4; color: #000; background: #fff; padding: 12px; width: 300px; user-select: text; }
  .center { text-align: center; }
  .store { font-weight: 700; font-size: 14px; }
  .rule { border-top: 1px dashed #000; margin: 6px 0; }
  .kv { display: flex; justify-content: space-between; gap: 8px; }
  .kv.total { font-weight: 700; font-size: 14px; margin: 4px 0; }
  table { width: 100%; border-collapse: collapse; }
  td { padding: 0; vertical-align: top; }
  td.name { padding-top: 3px; }
  td.qty { padding-left: 8px; }
  td.amt { text-align: right; white-space: nowrap; }
  .void { text-align: center; font-weight: 700; margin: 6px 0; }
  .footer { margin-top: 4px; }
</style>
