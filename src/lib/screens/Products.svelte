<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import type { Product } from "../types";
  import { session, toasts } from "../stores/session.svelte";
  import { fromCents, marginPct, money, toCents } from "../format";
  import Modal from "../components/Modal.svelte";
  import { detectColumns, parseCsv, rowsToImport, type Mapping } from "../csv";
  import type { ImportRow } from "../types";

  // CSV import
  let fileInput = $state<HTMLInputElement | null>(null);
  let importOpen = $state(false);
  let importRows = $state<ImportRow[]>([]);
  let importMapping = $state<Mapping | null>(null);
  let importing = $state(false);

  async function onFile(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0];
    if (!file) return;
    try {
      const rows = parseCsv(await file.text());
      if (rows.length < 2) throw new Error("The file has no data rows");
      importMapping = detectColumns(rows[0]);
      if (importMapping.columns.name === undefined) throw new Error("Could not find a product name column. Add a header called Name or Product.");
      importRows = rowsToImport(rows.slice(1), importMapping);
      importOpen = true;
    } catch (err) {
      toasts.error(err);
    } finally {
      if (fileInput) fileInput.value = "";
    }
  }
  async function runImport() {
    importing = true;
    try {
      const r = await api.importProducts(importRows);
      toasts.success(`Imported: ${r.created} new, ${r.updated} updated${r.skipped ? `, ${r.skipped} skipped` : ""}`);
      importOpen = false;
      await load();
    } catch (err) {
      toasts.error(err);
    } finally {
      importing = false;
    }
  }
  const has = (f: keyof ImportRow) => importMapping?.columns[f] !== undefined;

  let products = $state<Product[]>([]);
  let categories = $state<string[]>([]);
  let query = $state("");
  let showInactive = $state(false);
  let loading = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  let formOpen = $state(false);
  let saving = $state(false);
  let editing = $state<Product | null>(null);
  let f = $state({ name: "", barcode: "", category: "", cost: "", price: "", reorder: "5", opening: "" });
  let nameInput = $state<HTMLInputElement | null>(null);

  async function load() {
    loading = true;
    try {
      [products, categories] = await Promise.all([api.listProducts(query, showInactive), api.listCategories()]);
    } catch (e) {
      toasts.error(e);
    } finally {
      loading = false;
    }
  }
  onMount(load);

  function onsearch() {
    clearTimeout(timer);
    timer = setTimeout(load, 150);
  }

  function openNew() {
    editing = null;
    f = { name: "", barcode: "", category: "", cost: "", price: "", reorder: "5", opening: "" };
    formOpen = true;
    setTimeout(() => nameInput?.focus(), 0);
  }
  function openEdit(p: Product) {
    editing = p;
    f = { name: p.name, barcode: p.barcode ?? "", category: p.category ?? "", cost: fromCents(p.costPrice), price: fromCents(p.sellPrice), reorder: String(p.reorderLevel), opening: "" };
    formOpen = true;
    setTimeout(() => nameInput?.focus(), 0);
  }

  async function save(e: Event) {
    e.preventDefault();
    if (saving) return;
    saving = true;
    try {
      const saved = await api.saveProduct({
        id: editing?.id,
        name: f.name,
        barcode: f.barcode || null,
        category: f.category || null,
        costPrice: toCents(f.cost),
        sellPrice: toCents(f.price),
        reorderLevel: parseInt(f.reorder) || 0,
      });
      const opening = parseInt(f.opening) || 0;
      if (!editing && opening > 0) {
        await api.receiveStock([{ productId: saved.id, qty: opening, unitCost: toCents(f.cost) }], "Opening stock");
      }
      toasts.success(editing ? "Product updated" : "Product added");
      formOpen = false;
      await load();
    } catch (err) {
      toasts.error(err);
    } finally {
      saving = false;
    }
  }

  async function toggleActive(p: Product) {
    try {
      await api.setProductActive(p.id, !p.active);
      await load();
    } catch (e) {
      toasts.error(e);
    }
  }
</script>

<div class="page">
  <div class="page-header">
    <h1>Products</h1>
    <div class="toolbar">
      <input class="input search" placeholder="Search name, barcode or category" bind:value={query} oninput={onsearch} />
      <label class="check"><input type="checkbox" bind:checked={showInactive} onchange={load} /> Show inactive</label>
      {#if session.isOwner}
        <input bind:this={fileInput} type="file" accept=".csv,text/csv" hidden onchange={onFile} />
        <button class="btn" onclick={() => fileInput?.click()}>Import CSV</button>
        <button class="btn btn-primary" onclick={openNew}>New product</button>
      {/if}
    </div>
  </div>
  <div class="page-body">
    <div class="card table-wrap">
      <table class="table">
        <thead>
          <tr>
            <th>Name</th><th>Category</th><th>Barcode</th>
            <th class="num">Cost</th><th class="num">Price</th><th class="num">Margin</th><th class="num">Stock</th>
            {#if session.isOwner}<th></th>{/if}
          </tr>
        </thead>
        <tbody>
          {#each products as p (p.id)}
            <tr class:dim={!p.active}>
              <td class="strong">{p.name} {#if !p.active}<span class="badge badge-gray">inactive</span>{/if}</td>
              <td>{p.category ?? ""}</td>
              <td class="mono">{p.barcode ?? ""}</td>
              <td class="num">{money(p.costPrice)}</td>
              <td class="num">{money(p.sellPrice)}</td>
              <td class="num muted">{marginPct(p.costPrice, p.sellPrice)}</td>
              <td class="num">
                {#if p.stockQty <= 0}<span class="badge badge-red">{p.stockQty}</span>
                {:else if p.stockQty <= p.reorderLevel}<span class="badge badge-amber">{p.stockQty}</span>
                {:else}{p.stockQty}{/if}
              </td>
              {#if session.isOwner}
                <td class="actions">
                  <button class="btn btn-sm" onclick={() => openEdit(p)}>Edit</button>
                  <button class="btn btn-ghost btn-sm" onclick={() => toggleActive(p)}>{p.active ? "Deactivate" : "Activate"}</button>
                </td>
              {/if}
            </tr>
          {:else}
            <tr><td colspan="8" class="empty">{loading ? "Loading…" : query ? "No products match" : "No products yet. Add your first one."}</td></tr>
          {/each}
        </tbody>
      </table>
    </div>
  </div>
</div>

<Modal title={editing ? "Edit product" : "New product"} bind:open={formOpen} width={560}>
  <form id="pform" class="stack" onsubmit={save}>
    <div class="field">
      <label for="pn">Name</label>
      <input id="pn" bind:this={nameInput} class="input" bind:value={f.name} required />
    </div>
    <div class="grid-2">
      <div class="field">
        <label for="pb">Barcode</label>
        <input id="pb" class="input mono" bind:value={f.barcode} placeholder="Click here and scan" autocomplete="off" />
      </div>
      <div class="field">
        <label for="pc">Category</label>
        <input id="pc" class="input" bind:value={f.category} list="cats" placeholder="e.g. Whisky" />
        <datalist id="cats">{#each categories as c (c)}<option value={c}></option>{/each}</datalist>
      </div>
    </div>
    <div class="grid-2">
      <div class="field">
        <label for="pcost">Cost price (KES)</label>
        <input id="pcost" class="input num" bind:value={f.cost} inputmode="decimal" placeholder="0.00" />
      </div>
      <div class="field">
        <label for="pprice">Selling price (KES)</label>
        <input id="pprice" class="input num" bind:value={f.price} inputmode="decimal" placeholder="0.00" required />
      </div>
    </div>
    <div class="grid-2">
      <div class="field">
        <label for="pre">Low stock warning at</label>
        <input id="pre" class="input num" type="number" min="0" bind:value={f.reorder} />
      </div>
      {#if !editing}
        <div class="field">
          <label for="pop">Opening stock</label>
          <input id="pop" class="input num" type="number" min="0" bind:value={f.opening} placeholder="0" />
        </div>
      {/if}
    </div>
    {#if f.cost && f.price}
      <p class="muted">Margin: {marginPct(toCents(f.cost), toCents(f.price))} ({money(toCents(f.price) - toCents(f.cost))} per unit)</p>
    {/if}
  </form>
  {#snippet footer()}
    <button class="btn" onclick={() => (formOpen = false)}>Cancel</button>
    <button class="btn btn-primary" type="submit" form="pform" disabled={saving}>{editing ? "Save changes" : "Add product"}</button>
  {/snippet}
</Modal>

<Modal title="Import products" bind:open={importOpen} width={720}>
  <div class="stack">
    <p>
      Found <strong>{importRows.length}</strong> products. Columns detected:
      {#each ["name", "barcode", "category", "sellPrice", "costPrice", "stockQty"] as f (f)}
        <span class="badge {has(f as keyof ImportRow) ? 'badge-green' : 'badge-gray'}">{f === "sellPrice" ? "price" : f === "costPrice" ? "cost" : f === "stockQty" ? "stock" : f}</span>
      {/each}
    </p>
    <p class="muted">Existing products (same barcode or name) are updated and their stock set to the sheet's count. New ones are created with that count as opening stock.</p>
    <div class="table-wrap preview">
      <table class="table">
        <thead><tr><th>Name</th><th>Category</th><th>Barcode</th><th class="num">Price</th><th class="num">Cost</th><th class="num">Stock</th></tr></thead>
        <tbody>
          {#each importRows.slice(0, 8) as r, i (i)}
            <tr>
              <td>{r.name}</td><td>{r.category ?? ""}</td><td class="mono">{r.barcode ?? ""}</td>
              <td class="num">{r.sellPrice == null ? "" : money(r.sellPrice)}</td>
              <td class="num">{r.costPrice == null ? "" : money(r.costPrice)}</td>
              <td class="num">{r.stockQty ?? ""}</td>
            </tr>
          {/each}
          {#if importRows.length > 8}<tr><td colspan="6" class="muted">… and {importRows.length - 8} more</td></tr>{/if}
        </tbody>
      </table>
    </div>
  </div>
  {#snippet footer()}
    <button class="btn" onclick={() => (importOpen = false)}>Cancel</button>
    <button class="btn btn-primary" onclick={runImport} disabled={importing || !importRows.length}>Import {importRows.length} products</button>
  {/snippet}
</Modal>

<style>
  .search { width: 300px; }
  .preview { max-height: 320px; border: 1px solid var(--border); border-radius: var(--radius); }
  .strong { font-weight: 500; }
</style>
