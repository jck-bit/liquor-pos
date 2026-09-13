<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import type { PaymentMethod, SaleDetail } from "../types";
  import { cart } from "../stores/cart.svelte";
  import { session, toasts } from "../stores/session.svelte";
  import { fromCents, money, toCents, receiptNo } from "../format";
  import ProductPicker from "../components/ProductPicker.svelte";
  import Modal from "../components/Modal.svelte";
  import Receipt from "../components/Receipt.svelte";

  let picker = $state<ProductPicker | null>(null);
  let discountText = $state("");
  let clearArmed = $state(false);

  // Payment modal
  let payOpen = $state(false);
  let method = $state<PaymentMethod>("cash");
  let tenderedText = $state("");
  let mpesaCode = $state("");
  let paying = $state(false);
  let payInput = $state<HTMLInputElement | null>(null);

  // Receipt modal
  let receiptOpen = $state(false);
  let lastSale = $state<SaleDetail | null>(null);

  const tendered = $derived(toCents(tenderedText));
  const change = $derived(tendered - cart.total);

  onMount(() => picker?.focus());

  $effect(() => {
    cart.discount = toCents(discountText);
  });

  function openPay(m: PaymentMethod) {
    if (cart.isEmpty) return toasts.info("Scan a product first");
    method = m;
    tenderedText = fromCents(cart.total);
    mpesaCode = "";
    payOpen = true;
    setTimeout(() => {
      payInput?.focus();
      payInput?.select();
    }, 0);
  }

  async function completeSale() {
    if (paying) return;
    paying = true;
    try {
      const detail = await api.createSale({
        items: cart.lines.map((l) => ({ productId: l.product.id, qty: l.qty, unitPrice: l.unitPrice })),
        discount: cart.discount,
        paymentMethod: method,
        mpesaCode: method === "mpesa" ? mpesaCode : null,
        cashTendered: method === "cash" ? tendered : null,
      });
      payOpen = false;
      cart.clear();
      discountText = "";
      lastSale = detail;
      receiptOpen = true;
      toasts.success(`Sale ${receiptNo(detail.sale.id)} completed`);
    } catch (e) {
      toasts.error(e);
    } finally {
      paying = false;
    }
  }

  function newSale() {
    receiptOpen = false;
    lastSale = null;
    picker?.focus();
  }

  function clearCart() {
    if (!clearArmed) {
      clearArmed = true;
      setTimeout(() => (clearArmed = false), 3000);
      return;
    }
    cart.clear();
    discountText = "";
    clearArmed = false;
    picker?.focus();
  }

  function onkeydown(e: KeyboardEvent) {
    if (receiptOpen) {
      if (e.key === "Enter") {
        e.preventDefault();
        newSale();
      }
      return;
    }
    if (payOpen) {
      if (e.key === "Enter") {
        e.preventDefault();
        completeSale();
      }
      return;
    }
    if (e.key === "F2") {
      e.preventDefault();
      openPay("cash");
    } else if (e.key === "F3") {
      e.preventDefault();
      openPay("mpesa");
    } else if (e.key === "F1") {
      e.preventDefault();
      picker?.focus();
    }
  }

  const quickCash = [5000, 10000, 20000, 50000, 100000];
</script>

<svelte:window onkeydown={onkeydown} />

<div class="sell">
  <section class="left">
    <ProductPicker bind:this={picker} onpick={(p) => cart.add(p)} large />
    <div class="card cart">
      {#if cart.isEmpty}
        <div class="empty">
          <div class="big">Scan a product to start</div>
          <div>or type a name to search. F1 focuses the scanner box.</div>
        </div>
      {:else}
        <div class="table-wrap">
          <table class="table">
            <thead>
              <tr><th>Product</th><th class="num">Price</th><th class="qtyh">Qty</th><th class="num">Total</th><th></th></tr>
            </thead>
            <tbody>
              {#each cart.lines as line (line.product.id)}
                <tr>
                  <td>
                    <div class="pname">{line.product.name}</div>
                    {#if line.qty > line.product.stockQty}
                      <div class="warn">Only {line.product.stockQty} in stock</div>
                    {/if}
                  </td>
                  <td class="num">{money(line.unitPrice)}</td>
                  <td>
                    <div class="qty">
                      <button class="btn btn-sm" onclick={() => cart.setQty(line.product.id, line.qty - 1)}>−</button>
                      <input
                        class="input num"
                        type="number"
                        min="1"
                        value={line.qty}
                        onchange={(e) => cart.setQty(line.product.id, parseInt(e.currentTarget.value) || 1)}
                      />
                      <button class="btn btn-sm" onclick={() => cart.setQty(line.product.id, line.qty + 1)}>+</button>
                    </div>
                  </td>
                  <td class="num strong">{money(line.qty * line.unitPrice)}</td>
                  <td class="actions"><button class="btn btn-ghost btn-sm" onclick={() => cart.remove(line.product.id)}>✕</button></td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>
  </section>

  <aside class="card summary">
    <div class="rows">
      <div class="kv"><span class="muted">Items</span><span class="num">{cart.count}</span></div>
      <div class="kv"><span class="muted">Subtotal</span><span class="num">{money(cart.subtotal)}</span></div>
      <div class="kv">
        <label class="muted" for="disc">Discount</label>
        <input id="disc" class="input num disc" placeholder="0.00" bind:value={discountText} inputmode="decimal" />
      </div>
    </div>
    <div class="total">
      <span class="label">Total</span>
      <span class="amount num">{money(cart.total)}</span>
    </div>
    <div class="pay">
      <button class="btn btn-primary btn-lg" disabled={cart.isEmpty} onclick={() => openPay("cash")}>Cash <kbd>F2</kbd></button>
      <button class="btn btn-lg mpesa" disabled={cart.isEmpty} onclick={() => openPay("mpesa")}>M-Pesa <kbd>F3</kbd></button>
      <button class="btn btn-ghost" class:btn-danger={clearArmed} disabled={cart.isEmpty} onclick={clearCart}>
        {clearArmed ? "Click again to clear" : "Clear cart"}
      </button>
    </div>
  </aside>
</div>

<Modal title={method === "cash" ? "Cash payment" : "M-Pesa payment"} bind:open={payOpen} width={420}>
  <div class="stack">
    <div class="paytotal">
      <span class="muted">Amount due</span>
      <span class="num">KES {money(cart.total)}</span>
    </div>
    {#if method === "cash"}
      <div class="field">
        <label for="tend">Cash received</label>
        <input id="tend" bind:this={payInput} class="input input-lg num" bind:value={tenderedText} inputmode="decimal" />
      </div>
      <div class="quick">
        <button class="btn btn-sm" onclick={() => (tenderedText = fromCents(cart.total))}>Exact</button>
        {#each quickCash as q (q)}
          <button class="btn btn-sm" onclick={() => (tenderedText = fromCents(q))}>{money(q).replace(".00", "")}</button>
        {/each}
      </div>
      <div class="change" class:short={change < 0}>
        <span>{change < 0 ? "Short by" : "Change"}</span>
        <span class="num">KES {money(Math.abs(change))}</span>
      </div>
    {:else}
      <div class="field">
        <label for="code">M-Pesa transaction code</label>
        <input id="code" bind:this={payInput} class="input input-lg mono" bind:value={mpesaCode} placeholder="e.g. QGH7XK2M9P" autocomplete="off" spellcheck="false" style="text-transform: uppercase" />
        <span class="muted">Ask the customer for the code from their confirmation message.</span>
      </div>
    {/if}
  </div>
  {#snippet footer()}
    <button class="btn" onclick={() => (payOpen = false)}>Cancel</button>
    <button class="btn btn-primary" disabled={paying || (method === "cash" && change < 0) || (method === "mpesa" && mpesaCode.trim().length < 8)} onclick={completeSale}>
      Complete sale <kbd>Enter</kbd>
    </button>
  {/snippet}
</Modal>

<Modal title="Sale complete" bind:open={receiptOpen} onclose={newSale} width={380}>
  {#if lastSale}
    <div class="receipt-wrap"><Receipt detail={lastSale} /></div>
  {/if}
  {#snippet footer()}
    <button class="btn" onclick={() => window.print()}>Print</button>
    <button class="btn btn-primary" onclick={newSale}>New sale <kbd>Enter</kbd></button>
  {/snippet}
</Modal>

<style>
  .sell { display: flex; gap: 16px; height: 100%; padding: 20px 24px; }
  .left { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 12px; }
  .cart { flex: 1; min-height: 0; overflow: auto; display: flex; flex-direction: column; }
  .empty { margin: auto; }
  .empty .big { font-size: 18px; color: var(--ink-2); font-weight: 500; margin-bottom: 4px; }
  .pname { font-weight: 500; }
  .warn { font-size: 12px; color: var(--danger); }
  .qtyh { width: 140px; text-align: center; }
  .qty { display: flex; gap: 4px; justify-content: center; }
  .qty input { width: 56px; height: 28px; padding: 0 6px; }
  .qty input::-webkit-outer-spin-button, .qty input::-webkit-inner-spin-button { -webkit-appearance: none; margin: 0; }
  .strong { font-weight: 600; }

  .summary { width: 320px; flex-shrink: 0; display: flex; flex-direction: column; padding: 20px; gap: 18px; }
  .rows { display: flex; flex-direction: column; gap: 10px; }
  .kv { display: flex; justify-content: space-between; align-items: center; }
  .disc { width: 130px; }
  .total { display: flex; flex-direction: column; padding: 16px 0; border-top: 1px solid var(--border); border-bottom: 1px solid var(--border); }
  .total .label { font-size: 12px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--ink-3); }
  .total .amount { font-size: 36px; font-weight: 600; line-height: 1.2; }
  .pay { display: flex; flex-direction: column; gap: 8px; margin-top: auto; }
  .mpesa { background: var(--success); color: #fff; border-color: var(--success); }
  .mpesa:hover { background: #166534; }

  .paytotal { display: flex; justify-content: space-between; align-items: baseline; font-size: 20px; font-weight: 600; }
  .quick { display: flex; gap: 6px; flex-wrap: wrap; }
  .change { display: flex; justify-content: space-between; font-size: 18px; font-weight: 600; padding: 10px 12px; background: var(--success-soft); color: var(--success); border-radius: var(--radius); }
  .change.short { background: var(--danger-soft); color: var(--danger); }
  .receipt-wrap { display: flex; justify-content: center; }
</style>
