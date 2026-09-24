//! 規則書資料庫的讀取層。
//!
//! 資料庫是 `tools/build_db.py` 的建置產物，本身不進 git。這裡只讀不寫 ——
//! Phase 3 的編輯功能是改 `data/errata` 底下的 YAML 再重建資料庫，
//! 而不是直接改這個檔案，否則修改就失去可追溯性了。
//!
//! 依章節拆成子模組：`feat`（專長與其詳情）、`search`（搜尋與分面）。
//! 共用的連線管理、建置資訊與小工具留在這裡。

pub mod class;
pub mod feat;
pub mod item;
pub mod prose;
pub mod race;
pub mod refs;
pub mod search;
pub mod toc;

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;

pub use class::{ClassChapter, class_chapter};
pub use feat::{EntryDetail, FeatChapter, FeatDifficulty, entry_detail, feat_chapter, feat_difficulties};
pub use search::{SearchHit, search};
pub use item::{
    AffixChapter, DistributionGroup, MaterialChapter, affix_chapter, affix_distribution,
    material_chapter, ref_sheet,
};
pub use prose::{ErrataEntry, ProseChapter, errata_list, prose_chapter};
pub use race::{RaceChapter, race_chapter};
pub use refs::{RefTable, RefTableSummary, ref_index};
pub use toc::{Toc, toc};

/// 開啟後常駐的連線。
///
/// SQLite 的 Connection 不是 Sync，所以包一層 Mutex。裡面是 Option 而不是
/// 直接放 Connection，因為重建資料庫時必須先放開檔案 —— Windows 上
/// `build_db.py` 寫不進一個還被開著的檔案。重建期間這裡會是 None，
/// 查詢在那段時間會拿到「正在重建」的錯誤。
pub struct Db {
    conn: Mutex<Option<Connection>>,
    /// 目前這條連線開的是哪個檔案。重建之後要用同一個路徑重開。
    pub(crate) path: PathBuf,
}

impl Db {
    pub fn new(path: PathBuf, conn: Connection) -> Self {
        Self {
            conn: Mutex::new(Some(conn)),
            path,
        }
    }

    /// 借出連線執行查詢。
    ///
    /// Mutex 中毒（別的執行緒在持鎖時 panic）時回報錯誤字串，讓前端顯示
    /// 訊息而不是整個 app 一起 panic。
    pub fn with<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        let guard = self
            .conn
            .lock()
            .map_err(|_| "資料庫連線狀態異常，請重新開啟應用。".to_string())?;
        match guard.as_ref() {
            Some(conn) => f(conn),
            None => Err("資料庫正在重建，請稍候再試。".to_string()),
        }
    }

    /// 放開連線與檔案。重建前必須先做這件事 —— 丟棄 Connection 就會關閉
    /// 檔案控制代碼，Windows 上 `build_db.py` 才寫得進去。
    pub fn close(&self) -> Result<(), String> {
        let mut guard = self
            .conn
            .lock()
            .map_err(|_| "資料庫連線狀態異常，請重新開啟應用。".to_string())?;
        *guard = None;
        Ok(())
    }

    /// 重新開啟。重建無論成功或失敗都要呼叫 —— 一次失敗不該讓應用
    /// 再也查不了東西。
    pub fn reopen(&self) -> Result<(), String> {
        let conn = open(&self.path)?;
        let mut guard = self
            .conn
            .lock()
            .map_err(|_| "資料庫連線狀態異常，請重新開啟應用。".to_string())?;
        *guard = Some(conn);
        Ok(())
    }
}

/// 開發模式的資料庫位置：從 src-tauri 往上兩層就是 repo 根目錄。
pub fn dev_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("dist")
        .join("d100.db")
}

/// 找出資料庫檔案。
///
/// **開發時優先讀 repo 的 `dist/d100.db`，發佈版優先讀打包進去的 resource。**
///
/// 順序很重要，而且曾經是反過來的：`npm run sync-db` 會把 dist 複製一份到
/// `src-tauri/resources/`，那個複本在 `tauri dev` 底下也解析得到。resource
/// 排在前面時，應用讀的是那份複本而不是 `build_db.py` 剛寫好的 dist ——
/// 重跑建置後畫面不會變，而且完全沒有錯誤訊息，因為兩個檔案都是好的。
///
/// 兩種情況都找不到時回報所有找過的位置，比單純說「檔案不存在」好除錯。
pub fn locate(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;

    let resource = app
        .path()
        .resolve("resources/d100.db", tauri::path::BaseDirectory::Resource)
        .ok();

    let tried = candidates(resource, dev_path());
    for path in &tried {
        if path.is_file() {
            return Ok(path.clone());
        }
    }

    Err(format!(
        "找不到規則書資料庫。已尋找：{}
請先執行 python tools/build_db.py",
        tried
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("、")
    ))
}

/// 依序要試的位置。抽成純函式是為了讓順序本身有測試 —— 這裡弄反過的代價
/// 是「改了資料卻看不到變化」，而那不會有任何錯誤訊息。
fn candidates(resource: Option<PathBuf>, dev: PathBuf) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if cfg!(debug_assertions) {
        out.push(dev);
        out.extend(resource);
    } else {
        out.extend(resource);
        out.push(dev);
    }
    out
}

pub fn open(path: &PathBuf) -> Result<Connection, String> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("無法開啟資料庫 {}：{e}", path.display()))
}

// 資料契約 ---------------------------------------------------------------

#[derive(Serialize)]
pub struct BuildInfo {
    pub source_file: String,
    pub source_sha256: String,
    pub schema_version: String,
    pub feats: i64,
    pub races: i64,
    pub affixes: i64,
    pub class_paths: i64,
    pub rule_texts: i64,
}

// 共用查詢工具 -----------------------------------------------------------

/// 取首幾個字做預覽。效果敘述常是整段多行文字，清單上只需要一眼掃過的長度。
pub(crate) fn preview(text: &str, max_chars: usize) -> String {
    let flat = text.replace('\n', " ");
    let mut out: String = flat.chars().take(max_chars).collect();
    if flat.chars().count() > max_chars {
        out.push('…');
    }
    out
}

/// 把「一個 id 對多個值」的查詢結果收成 map，保留查詢本身的排序。
/// 用來一次撈完整章的分類與標記，避免每個條目各查一次。
pub(crate) fn multi_map(
    conn: &Connection,
    sql: &str,
    arg: &str,
) -> Result<std::collections::HashMap<String, Vec<String>>, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([arg], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;
    let mut out: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for row in rows {
        let (id, value) = row.map_err(|e| e.to_string())?;
        out.entry(id).or_default().push(value);
    }
    Ok(out)
}

/// 某條專長住在書裡的哪一章哪一頁。
///
/// 職業專長住在職業章節的該職業頁，其餘住在專長章節的 feat_group 頁。
/// 搜尋結果與前置鏈跳轉都靠這個分流，弄錯的話跳轉會落空，所以只寫一次。
pub(crate) fn feat_address(group: &str, class_name: Option<String>) -> (String, String) {
    match (group, class_name) {
        ("class", Some(class)) => ("classes".to_string(), class),
        _ => ("feats".to_string(), group.to_string()),
    }
}

pub fn build_info(conn: &Connection) -> Result<BuildInfo, String> {
    let get = |key: &str| -> String {
        conn.query_row(
            "SELECT value FROM build_info WHERE key = ?1",
            [key],
            |r| r.get::<_, String>(0),
        )
        .unwrap_or_else(|_| "？".into())
    };
    let count = |table: &str| -> i64 {
        conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap_or(0)
    };

    Ok(BuildInfo {
        source_file: get("source_file"),
        source_sha256: get("source_sha256"),
        schema_version: get("schema_version"),
        feats: count("feat"),
        races: count("race"),
        affixes: count("affix"),
        class_paths: count("class_path"),
        rule_texts: count("rule_text"),
    })
}

/// 測試都跑在真正的規則書資料庫上。造假資料在這裡沒有意義 ——
/// 我們要驗的正是「查詢能不能正確對付這份資料的形狀」。
#[cfg(test)]
pub(crate) fn test_conn() -> Connection {
    let path = dev_path();
    assert!(
        path.is_file(),
        "測試需要 dist/d100.db，請先執行 python tools/build_db.py"
    );
    open(&path).expect("開啟資料庫")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 開發時必須先看 dist —— 那是 build_db.py 寫的檔案。resource 是
    /// sync-db 留下的複本，排在前面會讓重建後的改動看不見。
    #[test]
    fn 開發模式優先讀dist() {
        let resource = PathBuf::from("resources/d100.db");
        let dev = PathBuf::from("dist/d100.db");
        let order = candidates(Some(resource.clone()), dev.clone());

        assert_eq!(order.len(), 2);
        if cfg!(debug_assertions) {
            assert_eq!(order[0], dev, "開發模式要先看 dist");
        } else {
            assert_eq!(order[0], resource, "發佈版要先看打包進去的 resource");
        }
    }

    #[test]
    fn 沒有resource時仍然找得到dist() {
        let dev = PathBuf::from("dist/d100.db");
        assert_eq!(candidates(None, dev.clone()), vec![dev]);
    }

    #[test]
    fn 建置資訊有內容() {
        let c = test_conn();
        let info = build_info(&c).unwrap();
        assert!(info.feats > 500, "專長數 {} 不合理", info.feats);
        assert_eq!(info.races, 13);
        assert!(!info.source_sha256.is_empty());
    }
}
