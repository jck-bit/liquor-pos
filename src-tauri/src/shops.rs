//! Several shops on one computer.
//!
//! Each shop has its own database file, so its sales, stock, users, settings and
//! cloud connection never mix with another shop's. `shops.json` in the app data
//! folder lists the shops and remembers which one was open last. A computer with
//! one shop never shows a shop picker, so shop tills look exactly as before.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rand_core::{OsRng, RngCore};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::{bad, AppResult};
use crate::sync::setting;

const REGISTRY: &str = "shops.json";
const DEFAULT_ID: &str = "main";
const DEFAULT_FILE: &str = "liquorpos.db";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Shop {
    pub id: String,
    pub name: String,
    pub file: String,
    /// "<supabase url>|<store id>" of the cloud store this shop's data belongs to.
    #[serde(default)]
    pub store: Option<String>,
    /// Cloud sync is switched on for this shop.
    #[serde(default)]
    pub connected: bool,
}

impl Shop {
    fn new(id: &str, name: &str, file: &str) -> Self {
        Self { id: id.into(), name: name.into(), file: file.into(), store: None, connected: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Registry {
    shops: Vec<Shop>,
    current: String,
}

impl Registry {
    /// First run, or shops.json missing or unreadable: list the database files that exist.
    fn from_files(dir: &Path) -> Self {
        let mut shops = vec![Shop::new(DEFAULT_ID, "My Liquor Store", DEFAULT_FILE)];
        let mut ids: Vec<String> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter_map(|f| f.strip_prefix("shop-").and_then(|r| r.strip_suffix(".db")).map(str::to_string))
            .collect();
        ids.sort();
        for id in ids {
            let file = format!("shop-{id}.db");
            shops.push(Shop::new(&id, &id, &file));
        }
        Self { shops, current: DEFAULT_ID.into() }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShopInfo {
    pub id: String,
    pub name: String,
    pub connected: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShopList {
    pub shops: Vec<ShopInfo>,
    pub current: String,
}

pub struct Shops {
    dir: PathBuf,
    reg: Mutex<Registry>,
}

impl Shops {
    pub fn load(dir: &Path) -> AppResult<Self> {
        let path = dir.join(REGISTRY);
        let loaded = match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Registry>(&text) {
                Ok(reg) if !reg.shops.is_empty() => Some(reg),
                _ => {
                    // Keep the unreadable file for inspection and rebuild from the database files.
                    let _ = std::fs::rename(&path, dir.join("shops.json.damaged"));
                    None
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        let mut reg = loaded.unwrap_or_else(|| Registry::from_files(dir));
        if !reg.shops.iter().any(|s| s.id == reg.current) {
            reg.current = reg.shops[0].id.clone();
        }
        let shops = Self { dir: dir.to_path_buf(), reg: Mutex::new(reg) };
        shops.save(&shops.lock())?;
        Ok(shops)
    }

    fn lock(&self) -> MutexGuard<'_, Registry> {
        self.reg.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Written to a temporary file first, so a power cut never leaves a half-written list.
    fn save(&self, reg: &Registry) -> AppResult<()> {
        let tmp = self.dir.join("shops.json.tmp");
        let text = serde_json::to_string_pretty(reg).map_err(|e| bad(format!("Could not save shops: {e}")))?;
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, self.dir.join(REGISTRY))?;
        Ok(())
    }

    pub fn current(&self) -> Shop {
        let reg = self.lock();
        reg.shops.iter().find(|s| s.id == reg.current).cloned().unwrap_or_else(|| reg.shops[0].clone())
    }

    pub fn get(&self, id: &str) -> AppResult<Shop> {
        self.lock().shops.iter().find(|s| s.id == id).cloned().ok_or_else(|| bad("That shop is not on this computer"))
    }

    pub fn path_of(&self, shop: &Shop) -> PathBuf {
        self.dir.join(&shop.file)
    }

    pub fn list(&self) -> ShopList {
        let reg = self.lock();
        ShopList {
            shops: reg
                .shops
                .iter()
                .map(|s| ShopInfo { id: s.id.clone(), name: s.name.clone(), connected: s.connected })
                .collect(),
            current: reg.current.clone(),
        }
    }

    pub fn set_current(&self, id: &str) -> AppResult<()> {
        let mut reg = self.lock();
        if !reg.shops.iter().any(|s| s.id == id) {
            return Err(bad("That shop is not on this computer"));
        }
        reg.current = id.to_string();
        self.save(&reg)
    }

    pub fn add(&self, name: &str) -> AppResult<Shop> {
        let name = clean_name(name)?;
        let mut reg = self.lock();
        if let Some(other) = reg.shops.iter().find(|s| s.name.eq_ignore_ascii_case(&name)) {
            return Err(bad(format!("There is already a shop called {} on this computer", other.name)));
        }
        let id = loop {
            let id = format!("{:08x}", OsRng.next_u32());
            if !reg.shops.iter().any(|s| s.id == id) && !self.dir.join(format!("shop-{id}.db")).exists() {
                break id;
            }
        };
        let shop = Shop::new(&id, &name, &format!("shop-{id}.db"));
        reg.shops.push(shop.clone());
        self.save(&reg)?;
        Ok(shop)
    }

    /// The cleaned name, or an error when another shop on this computer already uses it.
    pub fn check_name(&self, id: &str, name: &str) -> AppResult<String> {
        let name = clean_name(name)?;
        if let Some(other) = self.lock().shops.iter().find(|s| s.id != id && s.name.eq_ignore_ascii_case(&name)) {
            return Err(bad(format!("Another shop on this computer is already called {}", other.name)));
        }
        Ok(name)
    }

    /// The other shop on this computer whose data belongs to this cloud store, if any.
    pub fn store_owner(&self, store: &str, except: &str) -> Option<Shop> {
        self.lock().shops.iter().find(|s| s.id != except && s.store.as_deref() == Some(store)).cloned()
    }

    /// Bring the list in line with the shop's own database: name, cloud store, sync on or off.
    pub fn refresh(&self, id: &str, conn: &Connection) -> AppResult<()> {
        let name = setting(conn, "store_name");
        let store = setting(conn, "bound_store");
        let connected = setting(conn, "supabase_url").is_some();
        let mut reg = self.lock();
        let Some(shop) = reg.shops.iter_mut().find(|s| s.id == id) else { return Ok(()) };
        let before = shop.clone();
        if let Some(name) = name {
            shop.name = name;
        }
        shop.store = store;
        shop.connected = connected;
        if *shop != before {
            self.save(&reg)?;
        }
        Ok(())
    }
}

/// File-name friendly version of a shop name, for backups and exports.
pub fn slug(name: &str) -> String {
    let dashed: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let joined = dashed.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if joined.is_empty() { "shop".into() } else { joined }
}

fn clean_name(name: &str) -> AppResult<String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err(bad("Enter a shop name"));
    }
    if name.chars().count() > 40 {
        return Err(bad("Keep the shop name under 40 characters"));
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("liquorpos-shops-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn first_run_keeps_using_the_existing_database() {
        let dir = temp_dir("first");
        let shops = Shops::load(&dir).unwrap();
        assert_eq!(shops.list().shops.len(), 1, "a till must see a single shop and no picker");
        assert_eq!(shops.path_of(&shops.current()), dir.join("liquorpos.db"));
        assert!(dir.join("shops.json").exists());
    }

    #[test]
    fn each_shop_gets_its_own_file_and_a_unique_name() {
        let dir = temp_dir("add");
        let shops = Shops::load(&dir).unwrap();
        let vintage = shops.add("  Vintage ").unwrap();
        assert_eq!(vintage.name, "Vintage");
        assert_ne!(shops.path_of(&vintage), shops.path_of(&shops.current()));
        assert!(shops.add("vintage").is_err());
        shops.set_current(&vintage.id).unwrap();

        let again = Shops::load(&dir).unwrap();
        assert_eq!(again.list().shops.len(), 2);
        assert_eq!(again.current().id, vintage.id, "the last opened shop is remembered");
        assert!(again.check_name(DEFAULT_ID, "VINTAGE").is_err());
        assert!(again.check_name(&vintage.id, "Vintage").is_ok());
    }

    #[test]
    fn a_cloud_store_belongs_to_one_shop() {
        let dir = temp_dir("store");
        let shops = Shops::load(&dir).unwrap();
        let vintage = shops.add("Vintage").unwrap();
        {
            let mut reg = shops.lock();
            reg.shops[0].store = Some("https://a.supabase.co|1".into());
        }
        assert_eq!(shops.store_owner("https://a.supabase.co|1", &vintage.id).map(|s| s.id), Some(DEFAULT_ID.to_string()));
        assert!(shops.store_owner("https://a.supabase.co|1", DEFAULT_ID).is_none());
        assert!(shops.store_owner("https://b.supabase.co|2", &vintage.id).is_none());
    }

    #[test]
    fn a_damaged_list_is_rebuilt_from_the_database_files() {
        let dir = temp_dir("damaged");
        std::fs::write(dir.join("shops.json"), "{ not json").unwrap();
        std::fs::write(dir.join("shop-abc123.db"), "").unwrap();
        let shops = Shops::load(&dir).unwrap();
        let ids: Vec<String> = shops.list().shops.into_iter().map(|s| s.id).collect();
        assert_eq!(ids, vec!["main".to_string(), "abc123".to_string()]);
        assert!(dir.join("shops.json.damaged").exists());
    }

    #[test]
    fn slugs_are_safe_file_names() {
        assert_eq!(slug("Kimbo Shop"), "kimbo-shop");
        assert_eq!(slug("  ***  "), "shop");
    }
}
