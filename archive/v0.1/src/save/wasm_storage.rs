//! Browser-storage backend for save/quicksave/launch/blueprint persistence.
//!
//! IndexedDB is async and the existing `SaveGame` API is sync (it has to be —
//! it's called from inside winit's event-loop closure, which can't `.await`).
//! We bridge the two with a hot in-memory cache: at startup,
//! [`init_storage`] reads every record into [`STORE`]. After that, sync reads
//! (`list_saves`, `load_*`) hit the cache directly; writes update the cache
//! synchronously and `spawn_local` an IDB write so the user sees their save
//! immediately while the disk persistence happens in the background.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use idb::{
    Database, DatabaseEvent, Factory, KeyPath, ObjectStoreParams, Query, TransactionMode,
};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;

const DB_NAME: &str = "sunscatter";
const DB_VERSION: u32 = 1;
const STORE_NAME: &str = "kv";

/// Anything we persist is keyed by a string with a typed prefix:
///   "save:<save_id>"
///   "quicksave:<save_id>:<index>"
///   "launch:<save_id>"
///   "blueprint:<name>"
const KEY_SAVE: &str = "save:";
const KEY_QUICKSAVE: &str = "quicksave:";
const KEY_LAUNCH: &str = "launch:";
const KEY_BLUEPRINT: &str = "blueprint:";

#[derive(Clone)]
pub struct SaveRecord {
    pub save_id: String,
    pub name: String,
    pub vessel_count: usize,
    pub simulation_time: f64,
    pub modified_ms: f64,
    pub content: String,
}

#[derive(Clone)]
pub struct QuicksaveRecord {
    pub filename: String,
    pub index: u32,
    pub simulation_time: f64,
    pub modified_ms: f64,
    pub content: String,
}

#[derive(Default)]
struct SaveStore {
    saves: HashMap<String, SaveRecord>,                 // key = save_id
    quicksaves: HashMap<String, Vec<QuicksaveRecord>>,  // key = save_id
    launch: HashMap<String, SaveRecord>,                // key = save_id
    blueprints: HashMap<String, String>,                // key = name, value = RON
    db: Option<Rc<Database>>,
}

thread_local! {
    static STORE: RefCell<SaveStore> = RefCell::new(SaveStore::default());
}

fn now_ms() -> f64 {
    js_sys::Date::now()
}

fn ms_to_system_time(ms: f64) -> SystemTime {
    if ms.is_finite() && ms >= 0.0 {
        UNIX_EPOCH + Duration::from_millis(ms as u64)
    } else {
        UNIX_EPOCH
    }
}

/// Open the database, ensuring the kv object store exists.
async fn open_db() -> Result<Database, String> {
    let factory = Factory::new().map_err(|e| format!("idb factory: {:?}", e))?;
    let mut req = factory
        .open(DB_NAME, Some(DB_VERSION))
        .map_err(|e| format!("idb open: {:?}", e))?;

    req.on_upgrade_needed(|event| {
        let db = event.database().expect("upgrade db");
        if !db
            .store_names()
            .iter()
            .any(|n| n == STORE_NAME)
        {
            let mut params = ObjectStoreParams::new();
            params.key_path(Some(KeyPath::new_single("k")));
            db.create_object_store(STORE_NAME, params)
                .expect("create kv store");
        }
    });

    req.await.map_err(|e| format!("idb open await: {:?}", e))
}

/// Read every record from IDB into the in-memory cache. Call once at startup
/// before `Game::new()`.
pub async fn init_storage() -> Result<(), String> {
    let db = open_db().await?;

    let txn = db
        .transaction(&[STORE_NAME], TransactionMode::ReadOnly)
        .map_err(|e| format!("idb txn: {:?}", e))?;
    let store = txn
        .object_store(STORE_NAME)
        .map_err(|e| format!("idb store: {:?}", e))?;
    let all = store
        .get_all(None, None)
        .map_err(|e| format!("idb get_all: {:?}", e))?
        .await
        .map_err(|e| format!("idb get_all await: {:?}", e))?;

    let mut loaded_saves = HashMap::new();
    let mut loaded_quicks: HashMap<String, Vec<QuicksaveRecord>> = HashMap::new();
    let mut loaded_launch = HashMap::new();
    let mut loaded_bps = HashMap::new();

    for js in all {
        let entry: KvEntry = match serde_wasm_bindgen::from_value(js) {
            Ok(e) => e,
            Err(e) => {
                log::warn!("Skipping unparseable IDB entry: {:?}", e);
                continue;
            }
        };

        if let Some(save_id) = entry.k.strip_prefix(KEY_SAVE) {
            if let Some(rec) = parse_save_record(save_id, &entry) {
                loaded_saves.insert(save_id.to_string(), rec);
            }
        } else if let Some(rest) = entry.k.strip_prefix(KEY_QUICKSAVE) {
            if let Some((save_id, index)) = rest.rsplit_once(':') {
                if let Ok(index) = index.parse::<u32>() {
                    if let Some(rec) = parse_quicksave_record(index, &entry) {
                        loaded_quicks
                            .entry(save_id.to_string())
                            .or_default()
                            .push(rec);
                    }
                }
            }
        } else if let Some(save_id) = entry.k.strip_prefix(KEY_LAUNCH) {
            if let Some(rec) = parse_save_record(save_id, &entry) {
                loaded_launch.insert(save_id.to_string(), rec);
            }
        } else if let Some(name) = entry.k.strip_prefix(KEY_BLUEPRINT) {
            loaded_bps.insert(name.to_string(), entry.content);
        }
    }

    txn.commit()
        .map_err(|e| format!("idb txn commit: {:?}", e))?
        .await
        .map_err(|e| format!("idb txn commit await: {:?}", e))?;

    let counts = (
        loaded_saves.len(),
        loaded_quicks.values().map(|v| v.len()).sum::<usize>(),
        loaded_launch.len(),
        loaded_bps.len(),
    );

    STORE.with(|s| {
        let mut s = s.borrow_mut();
        s.saves = loaded_saves;
        s.quicksaves = loaded_quicks;
        s.launch = loaded_launch;
        s.blueprints = loaded_bps;
        s.db = Some(Rc::new(db));
    });

    log::info!(
        "Loaded from IndexedDB: {} saves, {} quicksaves, {} launch, {} blueprints",
        counts.0, counts.1, counts.2, counts.3,
    );
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct KvEntry {
    k: String,
    name: String,
    #[serde(default)]
    vessel_count: usize,
    #[serde(default)]
    simulation_time: f64,
    modified_ms: f64,
    content: String,
}

fn parse_save_record(save_id: &str, entry: &KvEntry) -> Option<SaveRecord> {
    Some(SaveRecord {
        save_id: save_id.to_string(),
        name: entry.name.clone(),
        vessel_count: entry.vessel_count,
        simulation_time: entry.simulation_time,
        modified_ms: entry.modified_ms,
        content: entry.content.clone(),
    })
}

fn parse_quicksave_record(index: u32, entry: &KvEntry) -> Option<QuicksaveRecord> {
    Some(QuicksaveRecord {
        filename: entry.name.clone(),
        index,
        simulation_time: entry.simulation_time,
        modified_ms: entry.modified_ms,
        content: entry.content.clone(),
    })
}

fn write_kv(entry: KvEntry) {
    let db = STORE.with(|s| s.borrow().db.clone());
    let Some(db) = db else {
        log::warn!("write_kv before init_storage; dropping {}", entry.k);
        return;
    };
    spawn_local(async move {
        if let Err(e) = put_one(&db, &entry).await {
            log::error!("IDB write failed for {}: {}", entry.k, e);
        }
    });
}

async fn put_one(db: &Database, entry: &KvEntry) -> Result<(), String> {
    let txn = db
        .transaction(&[STORE_NAME], TransactionMode::ReadWrite)
        .map_err(|e| format!("idb txn: {:?}", e))?;
    let store = txn
        .object_store(STORE_NAME)
        .map_err(|e| format!("idb store: {:?}", e))?;
    let value: JsValue = serde_wasm_bindgen::to_value(entry)
        .map_err(|e| format!("serialize: {:?}", e))?;
    store
        .put(&value, None)
        .map_err(|e| format!("put: {:?}", e))?
        .await
        .map_err(|e| format!("put await: {:?}", e))?;
    txn.commit()
        .map_err(|e| format!("commit: {:?}", e))?
        .await
        .map_err(|e| format!("commit await: {:?}", e))?;
    Ok(())
}

fn delete_kv(key: String) {
    let db = STORE.with(|s| s.borrow().db.clone());
    let Some(db) = db else { return };
    spawn_local(async move {
        if let Err(e) = delete_one(&db, &key).await {
            log::error!("IDB delete failed for {}: {}", key, e);
        }
    });
}

async fn delete_one(db: &Database, key: &str) -> Result<(), String> {
    let txn = db
        .transaction(&[STORE_NAME], TransactionMode::ReadWrite)
        .map_err(|e| format!("idb txn: {:?}", e))?;
    let store = txn
        .object_store(STORE_NAME)
        .map_err(|e| format!("idb store: {:?}", e))?;
    store
        .delete(Query::Key(JsValue::from_str(key)))
        .map_err(|e| format!("delete: {:?}", e))?
        .await
        .map_err(|e| format!("delete await: {:?}", e))?;
    txn.commit()
        .map_err(|e| format!("commit: {:?}", e))?
        .await
        .map_err(|e| format!("commit await: {:?}", e))?;
    Ok(())
}

// ---- Public synchronous API used by save::SaveGame and parts::BlueprintRegistry ----

pub fn write_save(save_id: &str, name: &str, content: String, simulation_time: f64, vessel_count: usize) {
    let modified_ms = now_ms();
    let record = SaveRecord {
        save_id: save_id.to_string(),
        name: name.to_string(),
        vessel_count,
        simulation_time,
        modified_ms,
        content: content.clone(),
    };
    STORE.with(|s| {
        s.borrow_mut().saves.insert(save_id.to_string(), record);
    });
    write_kv(KvEntry {
        k: format!("{KEY_SAVE}{save_id}"),
        name: name.to_string(),
        vessel_count,
        simulation_time,
        modified_ms,
        content,
    });
}

pub fn write_launch(save_id: &str, name: &str, content: String, simulation_time: f64, vessel_count: usize) {
    let modified_ms = now_ms();
    let record = SaveRecord {
        save_id: save_id.to_string(),
        name: name.to_string(),
        vessel_count,
        simulation_time,
        modified_ms,
        content: content.clone(),
    };
    STORE.with(|s| {
        s.borrow_mut().launch.insert(save_id.to_string(), record);
    });
    write_kv(KvEntry {
        k: format!("{KEY_LAUNCH}{save_id}"),
        name: name.to_string(),
        vessel_count,
        simulation_time,
        modified_ms,
        content,
    });
}

pub fn write_quicksave(
    save_id: &str,
    name: &str,
    content: String,
    simulation_time: f64,
) -> u32 {
    let modified_ms = now_ms();
    let next_index = STORE.with(|s| {
        let s = s.borrow();
        s.quicksaves
            .get(save_id)
            .map(|v| v.iter().map(|q| q.index).max().unwrap_or(0) + 1)
            .unwrap_or(1)
    });
    let filename = format!("quicksave_{}.ron", next_index);
    let record = QuicksaveRecord {
        filename: filename.clone(),
        index: next_index,
        simulation_time,
        modified_ms,
        content: content.clone(),
    };
    STORE.with(|s| {
        s.borrow_mut()
            .quicksaves
            .entry(save_id.to_string())
            .or_default()
            .push(record);
    });
    write_kv(KvEntry {
        k: format!("{KEY_QUICKSAVE}{save_id}:{next_index}"),
        name: name.to_string(),
        vessel_count: 0,
        simulation_time,
        modified_ms,
        content,
    });

    // Enforce the per-save quicksave cap: drop oldest entries (by index)
    // until the count is at most MAX_QUICKSAVES_PER_SAVE. Mirrors the
    // desktop pruning in save::prune_old_quicksaves.
    let to_delete: Vec<String> = STORE.with(|s| {
        let mut s = s.borrow_mut();
        let Some(list) = s.quicksaves.get_mut(save_id) else { return Vec::new() };
        if list.len() <= crate::save::MAX_QUICKSAVES_PER_SAVE { return Vec::new() };
        list.sort_by_key(|q| q.index);
        let drop_n = list.len() - crate::save::MAX_QUICKSAVES_PER_SAVE;
        let dropped: Vec<String> = list
            .drain(..drop_n)
            .map(|q| format!("{KEY_QUICKSAVE}{save_id}:{}", q.index))
            .collect();
        dropped
    });
    for key in to_delete {
        delete_kv(key);
    }

    next_index
}

pub fn list_saves() -> Vec<crate::save::SaveFileInfo> {
    use crate::save::SaveFileInfo;
    STORE.with(|s| {
        let mut out: Vec<SaveFileInfo> = s
            .borrow()
            .saves
            .values()
            .map(|r| SaveFileInfo {
                name: r.name.clone(),
                save_id: r.save_id.clone(),
                vessel_count: r.vessel_count,
                simulation_time: r.simulation_time,
                modified: ms_to_system_time(r.modified_ms),
            })
            .collect();
        out.sort_by(|a, b| b.modified.cmp(&a.modified));
        out
    })
}

pub fn list_quicksaves(save_id: &str) -> Vec<crate::save::QuicksaveInfo> {
    use crate::save::QuicksaveInfo;
    STORE.with(|s| {
        let mut out: Vec<QuicksaveInfo> = s
            .borrow()
            .quicksaves
            .get(save_id)
            .map(|v| v.iter().map(|r| QuicksaveInfo {
                filename: r.filename.clone(),
                index: r.index,
                simulation_time: r.simulation_time,
                modified: ms_to_system_time(r.modified_ms),
            }).collect())
            .unwrap_or_default();
        out.sort_by(|a, b| b.index.cmp(&a.index));
        out
    })
}

pub fn load_save_content(save_id: &str) -> Option<String> {
    STORE.with(|s| s.borrow().saves.get(save_id).map(|r| r.content.clone()))
}

pub fn load_launch_content(save_id: &str) -> Option<String> {
    STORE.with(|s| s.borrow().launch.get(save_id).map(|r| r.content.clone()))
}

pub fn load_quicksave_content(save_id: &str, filename: &str) -> Option<String> {
    STORE.with(|s| {
        s.borrow()
            .quicksaves
            .get(save_id)
            .and_then(|v| v.iter().find(|r| r.filename == filename))
            .map(|r| r.content.clone())
    })
}

pub fn delete_save(save_id: &str) {
    let quicksave_indices: Vec<u32> = STORE.with(|s| {
        let mut s = s.borrow_mut();
        s.saves.remove(save_id);
        s.launch.remove(save_id);
        let drained = s.quicksaves.remove(save_id).unwrap_or_default();
        drained.into_iter().map(|q| q.index).collect()
    });

    delete_kv(format!("{KEY_SAVE}{save_id}"));
    delete_kv(format!("{KEY_LAUNCH}{save_id}"));
    for idx in quicksave_indices {
        delete_kv(format!("{KEY_QUICKSAVE}{save_id}:{idx}"));
    }
}

// ---- Blueprint storage ----

pub fn write_blueprint(name: &str, content: String) {
    let modified_ms = now_ms();
    STORE.with(|s| {
        s.borrow_mut().blueprints.insert(name.to_string(), content.clone());
    });
    write_kv(KvEntry {
        k: format!("{KEY_BLUEPRINT}{name}"),
        name: name.to_string(),
        vessel_count: 0,
        simulation_time: 0.0,
        modified_ms,
        content,
    });
}

pub fn delete_blueprint(name: &str) {
    STORE.with(|s| {
        s.borrow_mut().blueprints.remove(name);
    });
    delete_kv(format!("{KEY_BLUEPRINT}{name}"));
}

pub fn user_blueprints() -> Vec<(String, String)> {
    STORE.with(|s| {
        s.borrow()
            .blueprints
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    })
}

/// Look up a single blueprint's RON content by stored name. Used by the
/// export-blueprint UI.
pub fn load_blueprint_content(name: &str) -> Option<String> {
    STORE.with(|s| s.borrow().blueprints.get(name).cloned())
}
