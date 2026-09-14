# Liquor POS

Offline point-of-sale and inventory for a liquor store. One small Windows installer, no internet needed, barcode scanner works out of the box.

## What it does

- **Sell**: scan or search products, cart, sale-level discount, cash (with change) or M-Pesa (transaction code recorded, duplicates rejected), receipt on screen with print.
- **Inventory**: every sale deducts stock in the same transaction. Receive deliveries, do stock counts, log damage. Low-stock warnings.
- **Reports**: today / this week / last week / any range. Net sales, profit, cash vs M-Pesa split, best sellers, stock value. Export to CSV for Excel.
- **Audit trail**: every sale, void, price change, stock adjustment and login is logged with who and when. Voided sales are kept, never deleted.
- **Users**: owner and cashier roles with PIN login.
- **Backup**: one-click copy of the database to Documents.
- **Cloud sync**: offline-first. SQLite on the till is the source of truth; every change is queued and pushed to Supabase whenever there is internet. Product and price edits made in Supabase flow back down to the till.

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
supabase/schema.sql     cloud tables + RLS, run once in the Supabase SQL editor
src-tauri/
  migrations/001_init.sql   schema (add 004_*.sql for future changes, register in db.rs)
  migrations/002_sync.sql   sync queue table and triggers
  migrations/003_store_sync.sql  store-wide sync: counted levels, origin of each row
  src/shops.rs          shops on this computer: one database file per shop, shops.json
  src/sync/             background cloud sync: push (mod.rs), pull and stock rebuild (pull.rs), Supabase client
  src/db.rs             connection, pragmas, migrations
  src/models.rs         structs shared with the frontend (serialised camelCase)
  src/commands/         one file per area: auth, users, products, stock, sales, reports, audit, system, sync
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

### Every release (from this Mac, no GitHub Actions needed)

```sh
npm run release:windows -- 0.2.0
```

That sets the version in both config files, commits, cross-compiles the Windows installer, signs it, and publishes it with `latest.json` to GitHub Releases. Installed copies show the update bar on next launch. One-time tooling for this: `brew install nsis llvm`, `cargo install cargo-xwin`, `rustup target add x86_64-pc-windows-msvc`.

### Every release (GitHub Actions, once billing is unlocked)

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

## Cloud sync with Supabase

The till never waits on the network. Triggers in SQLite (`src-tauri/migrations/002_sync.sql`) copy every insert and update into a `sync_outbox` queue in the same transaction as the change. A background thread (`src-tauri/src/sync/`) pushes the queue every 30 seconds, or immediately when you click "Sync now", and then pulls everything the store's other computers have uploaded. If the internet is down the queue simply grows and the sidebar shows "Offline · N waiting".

### One-time setup

1. Create a free project at supabase.com.
2. **SQL Editor > New query**, paste the contents of `supabase/schema.sql`, Run. This creates the tables, Row Level Security and a `daily_sales` view.
3. **Authentication > Users > Add user**: create a user for the store, e.g. `till@yourstore.co.ke` with a strong password. Untick "send confirmation email" or confirm it. This account *is* the store: every row it pushes is tagged with its user id and nobody else can read it.
4. **Project Settings > API**: copy the Project URL and the `anon` public key.
5. In the app, log in as owner, **Settings > Cloud sync**, paste the URL, anon key, store email and password, click **Connect**. The app verifies the login before saving.

### Several shops on one computer

Each shop, for example Kimbo and Vintage, is completely separate: its own sales, stock, users, PINs, settings and its own Supabase project. A shop's computer holds only that shop.

An owner's computer can hold several shops. Each shop there has its own database file, listed in `shops.json` in the app data folder.

- **Login screen.** On a computer with more than one shop, a shop picker appears above the username. A computer with one shop, like a till, shows no picker and looks exactly as before.
- **Switching.** Log out, or click Switch shop in the sidebar, then pick the other shop. Switching closes one shop's database and opens the other's, so nothing is ever shown together.
- **Adding a shop.** Settings, Shops, Add shop. It opens with an empty database; log in with `admin` and PIN `1234`, change the PIN, then connect that shop's own Supabase.
- **Protection.** A shop that already holds data from one Supabase store cannot be connected to a different one, and two shops on one computer cannot share a Supabase store. Either mistake would mix two shops, so the app refuses with an explanation.

Never connect an existing shop to another shop's Supabase to "look at" it. Add the other shop instead.

### One store, several computers

Every computer connected with the same store login shares one history. Within a minute, all of them show:

- every sale and void, with its receipt number and the computer it was rung up on (the Till column)
- every stock change: deliveries, counts, damage, sales and voids
- every user account and the full audit log
- product and price edits

**How stock stays the same everywhere.** Stock is never copied between computers as a number. Each computer rebuilds it from the shared list of stock movements: the most recent count sets the level at the moment it was taken, and every sale, delivery, void or damage after that moves it from there. A count done on the office computer at 10:30pm becomes the till's starting stock, and only sales made after 10:30pm reduce it. Keep computer clocks correct; Windows and macOS do this automatically.

**Who can change stock.** Only owners can receive deliveries, count, record damage, edit products or void sales, on any computer. Cashiers sell and can view sales, reports, movements and low stock.

**Users.** Accounts appear on every computer, but a PIN stays on the computer where it was set. To let a cashier log in on another computer, an owner opens Users there and sets their PIN. Accounts with the same username, such as each computer's `admin`, are the same person.

**Current stock in Supabase.** Query the `product_stock` view. `products.stock_qty` only holds the last figure some computer uploaded.

**Upgrading from 0.1.** Run `supabase/schema.sql` again in the SQL Editor before or right after installing 0.2 on the first computer. Until you do, the app shows "Supabase needs the latest setup" and keeps every change queued locally.

### Can a cashier hide sales from the cloud?

Not through the app. Sync is automatic, runs in the background, and pushes within seconds of every sale, void and stock change. Only an owner can turn it off, and doing so is written to the audit log. What a cashier *can* do is unplug the network. Then:

- The till shows a red bar, "Sales are not reaching the cloud", after an hour of unsynced changes.
- The `devices` table in Supabase shows each till's `last_seen`, how many changes are `pending`, the last receipt number and who last logged in. A till that goes quiet during trading hours is visible from anywhere.
- Receipt numbers are sequential, so a gap or a thin day stands out in the `sales` table.
- The moment the network is back, the queue drains. Nothing is lost unless the database file itself is deleted, and even then everything already synced is safe in the cloud.

What no software can catch is a sale that is never rung up. The answer to that is the stock count: Stock > Adjust / count against physical shelves, and the difference is logged.

### Looking at the data

Open **Table Editor** in Supabase, or query the `daily_sales` view. Anything that reads Supabase (a web dashboard, a Google Sheet, a phone app) can be built on top later without touching the till.

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
- Owner web dashboard on top of the Supabase tables
