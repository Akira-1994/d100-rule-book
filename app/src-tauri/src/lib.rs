mod db;
mod editing;
mod rules;
mod sheet;

use db::{
    AffixChapter, BuildInfo, ClassChapter, Db, DistributionGroup, EntryDetail, ErrataEntry,
    FeatChapter, MaterialChapter, ProseChapter, RaceChapter, RefTable, RefTableSummary,
    FeatDifficulty, SearchHit, Toc,
};
use editing::history::ErrataHistory;
use editing::rebuild::RebuildResult;
use editing::{EditRequest, EditableField, EditingStatus};
use rules::CpPlan;
use sheet::store::SheetSummary;
use sheet::{Derived, Sheet};
use tauri::Manager;

/// 角色卡存放的位置。
///
/// 這是本專案第一個寫進使用者資料夾的東西 —— 規則書資料庫是建置產物、
/// 勘誤在 repo 裡，角色卡兩者都不是：它屬於玩家。
pub struct SheetDir(pub std::path::PathBuf);

/// 一次借出連線並執行查詢。連線的生命週期管理在 `Db` 裡。
fn with_conn<T>(
    state: &tauri::State<'_, Db>,
    f: impl FnOnce(&rusqlite::Connection) -> Result<T, String>,
) -> Result<T, String> {
    state.with(f)
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

// 編輯 ------------------------------------------------------------------
//
// 只在開發模式有作用。打包版的 editing_enabled() 為 false，前端不會渲染
// 入口；後端這裡仍然多擋一層，不倚賴前端自律。

#[tauri::command]
fn editing_status() -> EditingStatus {
    editing::status()
}

#[tauri::command]
fn editable_fields(entry_kind: String) -> &'static [EditableField] {
    editing::fields_for(&entry_kind)
}

/// 追加一筆勘誤並重建資料庫。
///
/// 三個步驟任一失敗都要讓狀態回到可用：寫壞的檔案還原、關掉的連線重開。
#[tauri::command]
fn append_errata(
    state: tauri::State<'_, Db>,
    request: EditRequest,
) -> Result<RebuildResult, String> {
    if !editing::editing_enabled() {
        return Err("編輯功能只在開發模式開啟。".into());
    }
    editing::validate(&request)?;

    let before = editing::yaml::snapshot(&request.sheet);
    let path = editing::yaml::append(&request)?;

    // 寫壞 errata 會讓整個建置停擺，所以追加後先確認檔案仍可解析。
    if let Err(e) = editing::rebuild::yaml_parses(&path) {
        editing::yaml::restore(&path, before)?;
        return Err(format!("產生的勘誤無法解析，已還原檔案：\n{e}"));
    }

    // 重建要覆寫 dist/d100.db，Windows 上檔案開著就寫不進去。
    state.close()?;
    let result = editing::rebuild::run();
    // 無論成功失敗都要把連線開回來，否則一次失敗會讓應用再也查不了東西。
    state.reopen()?;

    if !result.ok {
        editing::yaml::restore(&path, before)?;
        // 還原之後要再重建一次，否則資料庫停在半途的狀態。
        state.close()?;
        let _ = editing::rebuild::run();
        state.reopen()?;
    }
    Ok(result)
}

#[tauri::command]
fn errata_history() -> ErrataHistory {
    editing::history::history()
}

// 角色卡 ----------------------------------------------------------------

#[tauri::command]
fn list_sheets(dir: tauri::State<'_, SheetDir>) -> Result<Vec<SheetSummary>, String> {
    sheet::store::list(&dir.0)
}

#[tauri::command]
fn load_sheet(dir: tauri::State<'_, SheetDir>, id: String) -> Result<Sheet, String> {
    sheet::store::load(&dir.0, &id)
}

#[tauri::command]
fn save_sheet(dir: tauri::State<'_, SheetDir>, sheet: Sheet) -> Result<(), String> {
    sheet::store::save(&dir.0, &sheet)
}

#[tauri::command]
fn new_sheet(dir: tauri::State<'_, SheetDir>, name: String) -> Result<Sheet, String> {
    let sheet = Sheet::new(sheet::new_id(), name);
    sheet::store::save(&dir.0, &sheet)?;
    Ok(sheet)
}

/// 刪除是真的刪檔案，沒得復原。呼叫端負責先跟使用者確認。
#[tauri::command]
fn delete_sheet(dir: tauri::State<'_, SheetDir>, id: String) -> Result<(), String> {
    sheet::store::delete(&dir.0, &id)
}

#[tauri::command]
fn import_sheet(
    dir: tauri::State<'_, SheetDir>,
    path: String,
) -> Result<Sheet, String> {
    sheet::store::import(&dir.0, std::path::Path::new(&path), sheet::new_id())
}

#[tauri::command]
fn export_sheet(
    dir: tauri::State<'_, SheetDir>,
    id: String,
    path: String,
) -> Result<(), String> {
    sheet::store::export(&dir.0, &id, std::path::Path::new(&path))
}

/// 算出一張卡的所有衍生值。
///
/// 表單每改一個字就呼叫一次，所以只查資料庫、不碰檔案。
#[tauri::command]
fn derive_sheet(state: tauri::State<'_, Db>, sheet: Sheet) -> Result<Derived, String> {
    with_conn(&state, |conn| {
        let context = sheet::context::load(conn, &sheet)?;
        Ok(sheet::derive(&sheet, &context))
    })
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
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // 資料庫找不到時就讓啟動失敗並印出找過哪些路徑 —— 這比開起一個
            // 每次查詢都報錯的空視窗容易診斷得多。
            let path = db::locate(app.handle())?;
            let conn = db::open(&path)?;
            app.manage(Db::new(path, conn));

            // 角色卡放使用者資料夾。這是第一個會寫進去的東西，資料夾
            // 不存在時由 store 在第一次存檔時建出來。
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("取不到使用者資料夾：{e}"))?;
            app.manage(SheetDir(data_dir));
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
            editing_status,
            editable_fields,
            append_errata,
            errata_history,
            list_sheets,
            load_sheet,
            save_sheet,
            new_sheet,
            delete_sheet,
            import_sheet,
            export_sheet,
            derive_sheet,
            search,
            entry_detail
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
