mod db;
mod rules;

use std::sync::Mutex;

use db::{
    AffixChapter, BuildInfo, ClassChapter, Db, DistributionGroup, EntryDetail, ErrataEntry,
    FeatChapter, MaterialChapter, ProseChapter, RaceChapter, RefTable, RefTableSummary,
    FeatDifficulty, SearchHit, Toc,
};
use rules::CpPlan;
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
fn race_chapter(state: tauri::State<'_, Db>) -> Result<RaceChapter, String> {
    with_conn(&state, db::race_chapter)
}

#[tauri::command]
fn affix_chapter(state: tauri::State<'_, Db>, tier: String) -> Result<AffixChapter, String> {
    with_conn(&state, |conn| db::affix_chapter(conn, &tier))
}

#[tauri::command]
fn material_chapter(state: tauri::State<'_, Db>) -> Result<MaterialChapter, String> {
    with_conn(&state, db::material_chapter)
}

#[tauri::command]
fn affix_distribution(state: tauri::State<'_, Db>) -> Result<Vec<DistributionGroup>, String> {
    with_conn(&state, db::affix_distribution)
}

#[tauri::command]
fn ref_sheet(state: tauri::State<'_, Db>, sheet: String) -> Result<Vec<RefTable>, String> {
    with_conn(&state, |conn| db::ref_sheet(conn, &sheet))
}

#[tauri::command]
fn ref_index(state: tauri::State<'_, Db>) -> Result<Vec<RefTableSummary>, String> {
    with_conn(&state, db::ref_index)
}

#[tauri::command]
fn prose_chapter(state: tauri::State<'_, Db>, sheet: String) -> Result<ProseChapter, String> {
    with_conn(&state, |conn| db::prose_chapter(conn, &sheet))
}

#[tauri::command]
fn errata_list(state: tauri::State<'_, Db>) -> Result<Vec<ErrataEntry>, String> {
    with_conn(&state, db::errata_list)
}

#[tauri::command]
fn feat_difficulties(state: tauri::State<'_, Db>) -> Result<Vec<FeatDifficulty>, String> {
    with_conn(&state, db::feat_difficulties)
}

/// CP 成本表。純計算，不碰資料庫。
#[tauri::command]
fn cp_plan(difficulty: f64, scale: String) -> CpPlan {
    rules::cp_plan(difficulty, &scale)
}

#[tauri::command]
fn search(state: tauri::State<'_, Db>, query: String) -> Result<Vec<SearchHit>, String> {
    with_conn(&state, |conn| db::search(conn, &query, 60))
}

#[tauri::command]
fn entry_detail(
    state: tauri::State<'_, Db>,
    kind: String,
    id: String,
) -> Result<EntryDetail, String> {
    with_conn(&state, |conn| db::entry_detail(conn, &kind, &id))
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
            race_chapter,
            affix_chapter,
            material_chapter,
            affix_distribution,
            ref_sheet,
            ref_index,
            prose_chapter,
            errata_list,
            feat_difficulties,
            cp_plan,
            search,
            entry_detail
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
