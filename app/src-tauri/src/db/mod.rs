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
pub mod search;
pub mod toc;

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;

pub use class::{ClassChapter, class_chapter};
pub use feat::{FeatDetail, FeatSummary, feat_detail};
pub use search::{Facets, facets, search_feats};
pub use toc::{Toc, toc};

/// 開啟後常駐的連線。SQLite 的 Connection 不是 Sync，所以包一層 Mutex。
pub struct Db(pub Mutex<Connection>);

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
/// 開發時讀 repo 的 `dist/d100.db`，發佈版讀打包進去的 resource。
/// 兩種情況都找不到時回報所有找過的位置，比單純說「檔案不存在」好除錯。
pub fn locate(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;

    let mut tried: Vec<PathBuf> = Vec::new();

    if let Ok(resource) = app.path().resolve(
        "resources/d100.db",
        tauri::path::BaseDirectory::Resource,
    ) {
        if resource.is_file() {
            return Ok(resource);
        }
        tried.push(resource);
    }

    let dev = dev_path();
    if dev.is_file() {
        return Ok(dev);
    }
    tried.push(dev);

    Err(format!(
        "找不到規則書資料庫。已尋找：{}\n請先執行 python tools/build_db.py",
        tried
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("、")
    ))
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

pub(crate) fn categories_of(
    conn: &Connection,
    feat_id: &str,
) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT c.category FROM feat_category c
         JOIN category k ON k.code = c.category
         WHERE c.feat_id = ?1 ORDER BY k.sort_order",
    )?;
    let rows = stmt.query_map([feat_id], |r| r.get::<_, String>(0))?;
    rows.collect()
}

/// 產生 n 個連續編號的佔位符（"?2,?3,?4"），並把游標往後推。
pub(crate) fn numbered(next: &mut usize, n: usize) -> String {
    let holes: Vec<String> = (0..n)
        .map(|i| format!("?{}", *next + i))
        .collect();
    *next += n;
    holes.join(",")
}

/// 取首幾個字做預覽。效果敘述常是整段多行文字，清單上只需要一眼掃過的長度。
pub(crate) fn preview(text: &str, max_chars: usize) -> String {
    let flat = text.replace('\n', " ");
    let mut out: String = flat.chars().take(max_chars).collect();
    if flat.chars().count() > max_chars {
        out.push('…');
    }
    out
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

    #[test]
    fn 建置資訊有內容() {
        let c = test_conn();
        let info = build_info(&c).unwrap();
        assert!(info.feats > 500, "專長數 {} 不合理", info.feats);
        assert_eq!(info.races, 13);
        assert!(!info.source_sha256.is_empty());
    }
}
