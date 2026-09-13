# Liquor POS

Offline point-of-sale and inventory for a liquor store. One small Windows installer, no internet needed, barcode scanner works out of the box.

## What it does

- **Sell**: scan or search products, cart, sale-level discount, cash (with change) or M-Pesa (transaction code recorded, duplicates rejected), receipt on screen with print.
- **Inventory**: every sale deducts stock in the same transaction. Receive deliveries, do stock counts, log damage. Low-stock warnings.
- **Reports**: today / this week / last week / any range. Net sales, profit, cash vs M-Pesa split, best sellers, stock value. Export to CSV for Excel.
- **Audit trail**: every sale, void, price change, stock adjustment and login is logged with who and when. Voided sales are kept, never deleted.
- **Users**: owner and cashier roles with PIN login.
- **Backup**: one-click copy of the database to Documents.

First login: username `admin`, PIN `1234`. Change it under Settings on day one.

## Stack

| Layer | Choice | Why |
|---|---|---|
| Shell | Tauri 2 | ~5 MB installer, native window, no bundled browser |
| UI | Svelte 5 + TypeScript, plain CSS | Compiles to tiny vanilla JS, no runtime |
| Data | SQLite via `rusqlite` (bundled) | Single file, transactional, indexed queries |
| Backend | Rust (`src-tauri/src`) | Thin command layer: open DB, run SQL, return rows |

All money is stored as integer cents. All writes that touch more than one table run in a single SQLite transaction.

## Project layout

```
src/                    Svelte frontend
  lib/api.ts            typed wrappers for every backend command
  lib/format.ts         money and date helpers
  lib/stores/           session (user, settings, current screen), cart
  lib/components/       Modal, Toasts, Sidebar, ProductPicker (scanner input), DateRange, Receipt
  lib/screens/          Login, Sell, Products, Stock, Sales, Reports, Audit, Users, Settings
src-tauri/
  migrations/001_init.sql   schema (add 002_*.sql for future changes, register in db.rs)
  src/db.rs             connection, pragmas, migrations
  src/models.rs         structs shared with the frontend (serialised camelCase)
  src/commands/         one file per area: auth, users, products, stock, sales, reports, audit, system
  src/lib.rs            registers every command
.github/workflows/      builds the Windows installer in the cloud
```

## Developing (on this Mac)

```sh
export PATH="$HOME/.cargo/bin:$PATH"   # once per terminal, or add to ~/.zshrc
npm install
npm run tauri dev                      # opens the app with hot reload
npm run check                          # Svelte + TypeScript type check
```

The dev database lives in `~/Library/Application Support/com.liquorpos.app/liquorpos.db`. Delete it to start fresh.

## Building the Windows installer

Windows installers are built by GitHub Actions (free), so no Windows PC is needed:

1. Push this repo to GitHub.
2. Tag a release: `git tag v0.1.0 && git push --tags`.
3. Wait ~10 minutes. Under **Releases** there will be a draft with `Liquor POS_0.1.0_x64-setup.exe`.
4. Send that file to the store. They double-click it, click Next, done.

Or open the **Actions** tab and run "Build Windows installer" by hand.

The installer includes a bootstrapper for Microsoft WebView2, which Windows 10 and 11 normally already have.

## On the store's PC

- Plug in any USB barcode scanner. It acts as a keyboard; no drivers or setup.
- Database: `C:\Users\<name>\AppData\Roaming\com.liquorpos.app\liquorpos.db`
- Backups and CSV exports: `Documents\Liquor POS\`
- Receipt printing uses the Windows default printer via the normal print dialog. Set a thermal printer as default and choose 72/80 mm paper once.

## Keyboard shortcuts on the sell screen

| Key | Action |
|---|---|
| F1 | Focus the scanner box |
| F2 | Pay by cash |
| F3 | Pay by M-Pesa |
| Enter | Confirm payment / start next sale |
| Esc | Close dialog |

## Roadmap

- M-Pesa statement import and reconciliation against recorded codes
- Daraja API (real-time Buy Goods payments) once the store has a registered till
- Thermal printer direct printing (ESC/POS) without the print dialog
- Multiple tills syncing to one owner dashboard
