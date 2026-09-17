import { api } from "./api";
import { cart } from "./stores/cart.svelte";
import { session, syncState } from "./stores/session.svelte";

/** Everything that depends on which shop is open, reloaded after login, a switch, or adding a shop. */
export async function reloadShopContext() {
  const [shops, settings, user] = await Promise.all([api.listShops(), api.getSettings(), api.currentUser()]);
  session.shops = shops;
  session.settings = settings;
  session.user = user;
  syncState.status = await api.syncStatus().catch(() => null);
}

/** Owner only: move to another of this computer's shops without logging out. */
export async function enterShop(shopId: string) {
  await api.enterShop(shopId);
  cart.clear();
  await reloadShopContext();
}
