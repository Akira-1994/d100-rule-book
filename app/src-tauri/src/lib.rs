mod db;

use std::sync::Mutex;

use db::{BuildInfo, ClassChapter, Db, Facets, FeatChapter, FeatDetail, FeatSummary, Toc};
use tauri::Manager;

/// 一次借出連線並執行查詢。
///
/// Mutex 中毒（別的執行緒在持鎖時 panic）時直接回報錯誤字串，
/// 讓前端顯示「請重開應用」而不是整個 app 一起 panic。
fn with_conn<T>(
    state: &tauri::State<'_, Db>,
    f: impl FnOnce(&rusqlite::Connection) -> Result<T, String>,
) -> Result<T, String> {
    let guard = state
        .0
        .lock()
        .map_err(|_| "資料庫連線狀態異常，請重新開啟應用。".to_string())?;
    f(&guard)
}

#[tauri::command]
fn build_info(state: tauri::State<'_, Db>) -> Result<BuildInfo, String> {
    with_conn(&state, db::build_info)
}

#[tauri::command]
fn toc(state: tauri::State<'_, Db>) -> Result<Toc, String> {
    with_conn(&state, db::toc)
}

#[tauri::command]
fn class_chapter(
    state: tauri::State<'_, Db>,
    class_name: String,
) -> Result<ClassChapter, String> {
    with_conn(&state, |conn| db::class_chapter(conn, &class_name))
}

#[tauri::command]
fn feat_chapter(state: tauri::State<'_, Db>, group: String) -> Result<FeatChapter, String> {
    with_conn(&state, |conn| db::feat_chapter(conn, &group))
}

#[tauri::command]
fn facets(state: tauri::State<'_, Db>) -> Result<Facets, String> {
    with_conn(&state, db::facets)
}

#[tauri::command]
fn search_feats(
    state: tauri::State<'_, Db>,
    query: String,
    groups: Vec<String>,
    categories: Vec<String>,
) -> Result<Vec<FeatSummary>, String> {
    with_conn(&state, |conn| {
        db::search_feats(conn, &query, &groups, &categories, 300)
    })
}

#[tauri::command]
fn feat_detail(state: tauri::State<'_, Db>, id: String) -> Result<FeatDetail, String> {
    with_conn(&state, |conn| db::feat_detail(conn, &id))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 資料庫找不到時就讓啟動失敗並印出找過哪些路徑 —— 這比開起一個
            // 每次查詢都報錯的空視窗容易診斷得多。
            let path = db::locate(app.handle())?;
            let conn = db::open(&path)?;
            app.manage(Db(Mutex::new(conn)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            build_info,
            toc,
            class_chapter,
            feat_chapter,
            facets,
            search_feats,
            feat_detail
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
