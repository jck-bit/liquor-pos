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

## Releasing: from your Mac to the store's PC

Everything is built in the cloud by GitHub Actions. You never need a Windows PC.

### One-time setup

1. Create a **public** GitHub repository (public so the store's app can download updates without a login) and push:
   ```sh
   git remote add origin https://github.com/<you>/liquor-pos.git
   git push -u origin main
   ```
2. In `src-tauri/tauri.conf.json`, replace `GITHUB_USER` in the updater endpoint with your GitHub username. Commit.
3. Add two repository secrets under **Settings > Secrets and variables > Actions**:
   - `TAURI_SIGNING_PRIVATE_KEY`: the contents of `~/.tauri/liquorpos.key` (`cat ~/.tauri/liquorpos.key | pbcopy`)
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: leave the value empty (the key has no password)

   Back up `~/.tauri/liquorpos.key` somewhere safe. Updates are signed with it; if it is lost, installed copies can never update again and would need a fresh install.

### Every release

1. Bump the version in **both** `package.json` and `src-tauri/tauri.conf.json` (they must match the tag).
2. Commit, tag and push:
   ```sh
   git commit -am "Release 0.2.0"
   git tag v0.2.0
   git push && git push --tags
   ```
3. About 10 minutes later the **Releases** page has `Liquor POS_0.2.0_x64-setup.exe` plus `latest.json`.

### First install at the store

Send the `.exe` from the release page. They double-click it, click Next, and the app is in the Start menu. That is the only time anyone touches an installer.

### Updates at the store

On every launch the app checks the release page. If a newer version exists, an orange bar appears at the top: **"Update 0.2.0 is ready. Install now."** One click downloads it, installs, and restarts the app. No internet at that moment just means the bar does not appear; it will next time.

The Windows installer includes a bootstrapper for Microsoft WebView2, which Windows 10 and 11 normally already have.

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
- Supabase sync: local SQLite stays the source of truth, sales and stock are mirrored to Postgres so the owner can see them from anywhere
