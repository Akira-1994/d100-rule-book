//! 規則書資料庫的讀取層。
//!
//! 資料庫是 `tools/build_db.py` 的建置產物，本身不進 git。這裡只讀不寫 ——
//! Phase 3 的編輯功能是改 `data/errata` 底下的 YAML 再重建資料庫，
//! 而不是直接改這個檔案，否則修改就失去可追溯性了。

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::{Connection, OpenFlags, Row};
use serde::Serialize;

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

#[derive(Serialize)]
pub struct FeatSummary {
    pub id: String,
    pub name: String,
    pub feat_group: String,
    pub difficulty: Option<f64>,
    pub difficulty_raw: Option<String>,
    pub difficulty_scale: String,
    pub categories: Vec<String>,
    pub class_path: Option<String>,
    pub effect_preview: String,
}

#[derive(Serialize)]
pub struct Prereq {
    pub kind: String,
    pub ref_feat_id: Option<String>,
    pub ref_feat_name: Option<String>,
    pub ref_code: Option<String>,
    pub min_level: Option<i64>,
    pub raw_text: String,
}

#[derive(Serialize)]
pub struct Dependent {
    pub id: String,
    pub name: String,
    pub min_level: Option<i64>,
}

#[derive(Serialize)]
pub struct CpStep {
    pub level: i64,
    pub step_cost: f64,
    pub cumulative: f64,
}

#[derive(Serialize)]
pub struct FeatDetail {
    pub id: String,
    pub name: String,
    pub feat_group: String,
    pub difficulty: Option<f64>,
    pub difficulty_raw: Option<String>,
    pub difficulty_scale: String,
    pub effect: String,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub class_name: Option<String>,
    pub class_path: Option<String>,
    pub parent_name: Option<String>,
    pub prereqs: Vec<Prereq>,
    pub dependents: Vec<Dependent>,
    pub cp_table: Vec<CpStep>,
    pub source_sheet: String,
    pub source_row: i64,
    pub errata: Vec<ErrataNote>,
}

#[derive(Serialize)]
pub struct ErrataNote {
    pub action: String,
    pub field: Option<String>,
    pub issue: Option<String>,
    pub reason: String,
}

#[derive(Serialize)]
pub struct Facet {
    pub value: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct Facets {
    pub groups: Vec<Facet>,
    pub categories: Vec<Facet>,
    pub classes: Vec<Facet>,
}

// 查詢 -------------------------------------------------------------------

fn categories_of(conn: &Connection, feat_id: &str) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT c.category FROM feat_category c
         JOIN category k ON k.code = c.category
         WHERE c.feat_id = ?1 ORDER BY k.sort_order",
    )?;
    let rows = stmt.query_map([feat_id], |r| r.get::<_, String>(0))?;
    rows.collect()
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

/// 搜尋專長。
///
/// `query` 同時比對名稱與效果內文 —— 規則書的效果敘述往往才是玩家記得的
/// 那一句（「每輪回復1d10hp」），只比對名稱等於少掉一半的用處。
pub fn search_feats(
    conn: &Connection,
    query: &str,
    groups: &[String],
    categories: &[String],
    limit: i64,
) -> Result<Vec<FeatSummary>, String> {
    let mut sql = String::from(
        "SELECT f.id, f.name, f.feat_group, f.difficulty, f.difficulty_raw,
                f.difficulty_scale, p.name AS path_name, f.effect
         FROM feat f
         LEFT JOIN class_path p ON p.id = f.class_path_id
         WHERE 1 = 1",
    );

    // 全部用明確編號的佔位符（?1、?2…）。混用 `?` 與 `?1` 在 SQLite 是合法的，
    // 但自動編號取的是「目前最大編號 +1」，所以只要 ?1 出現在某個 `?` 後面，
    // 編號就會撞在一起 —— 沒有關鍵字卻有篩選條件時剛好會踩到。
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    // ?1 固定是搜尋樣式，ORDER BY 要重複用到它。沒有關鍵字時綁一個恆不命中
    // 的字串，讓排序條件失效但語法仍然成立。
    let has_query = !query.trim().is_empty();
    args.push(Box::new(if has_query {
        format!("%{}%", query.trim())
    } else {
        String::from("\u{0}")
    }));
    let mut next = 2;

    if has_query {
        sql.push_str(" AND (f.name LIKE ?1 OR f.effect LIKE ?1)");
    }
    if !groups.is_empty() {
        let holes = numbered(&mut next, groups.len());
        sql.push_str(&format!(" AND f.feat_group IN ({holes})"));
        for g in groups {
            args.push(Box::new(g.clone()));
        }
    }
    if !categories.is_empty() {
        let holes = numbered(&mut next, categories.len());
        sql.push_str(&format!(
            " AND EXISTS (SELECT 1 FROM feat_category c
                          WHERE c.feat_id = f.id AND c.category IN ({holes}))"
        ));
        for c in categories {
            args.push(Box::new(c.clone()));
        }
    }
    // 名稱命中排在效果命中前面：找「健壯」的人多半要的是那個技能本身。
    sql.push_str(&format!(
        " ORDER BY CASE WHEN f.name LIKE ?1 THEN 0 ELSE 1 END,
                   f.feat_group, f.name LIMIT ?{next}"
    ));
    args.push(Box::new(limit));

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref()).collect();

    let mapper = |row: &Row| -> Result<(String, String, String, Option<f64>, Option<String>, String, Option<String>, String), rusqlite::Error> {
        Ok((
            row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?,
            row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?,
        ))
    };

    let rows = stmt
        .query_map(refs.as_slice(), mapper)
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for row in rows {
        let (id, name, group, difficulty, difficulty_raw, scale, path, effect) =
            row.map_err(|e| e.to_string())?;
        let categories = categories_of(conn, &id).map_err(|e| e.to_string())?;
        out.push(FeatSummary {
            effect_preview: preview(&effect, 90),
            id,
            name,
            feat_group: group,
            difficulty,
            difficulty_raw,
            difficulty_scale: scale,
            categories,
            class_path: path,
        });
    }
    Ok(out)
}

/// 產生 n 個連續編號的佔位符（"?2,?3,?4"），並把游標往後推。
fn numbered(next: &mut usize, n: usize) -> String {
    let holes: Vec<String> = (0..n)
        .map(|i| format!("?{}", *next + i))
        .collect();
    *next += n;
    holes.join(",")
}

/// 取首幾個字做預覽。效果敘述常是整段多行文字，清單上只需要一眼掃過的長度。
fn preview(text: &str, max_chars: usize) -> String {
    let flat = text.replace('\n', " ");
    let mut out: String = flat.chars().take(max_chars).collect();
    if flat.chars().count() > max_chars {
        out.push('…');
    }
    out
}

pub fn facets(conn: &Connection) -> Result<Facets, String> {
    let collect = |sql: &str| -> Result<Vec<Facet>, String> {
        let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Facet {
                    value: r.get(0)?,
                    count: r.get(1)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    };

    Ok(Facets {
        groups: collect(
            "SELECT feat_group, count(*) FROM feat GROUP BY 1 ORDER BY 2 DESC",
        )?,
        categories: collect(
            "SELECT c.code, count(fc.feat_id) FROM category c
             LEFT JOIN feat_category fc ON fc.category = c.code
             GROUP BY c.code ORDER BY c.sort_order",
        )?,
        classes: collect(
            "SELECT class_name, count(DISTINCT id) FROM class_path
             GROUP BY 1 ORDER BY 2 DESC",
        )?,
    })
}

pub fn feat_detail(conn: &Connection, id: &str) -> Result<FeatDetail, String> {
    let (
        name,
        group,
        difficulty,
        difficulty_raw,
        scale,
        effect,
        class_name,
        path_name,
        parent_name,
        sheet,
        row_no,
    ) = conn
        .query_row(
            "SELECT f.name, f.feat_group, f.difficulty, f.difficulty_raw,
                    f.difficulty_scale, f.effect, p.class_name, p.name,
                    parent.name, f.source_sheet, f.source_row
             FROM feat f
             LEFT JOIN class_path p ON p.id = f.class_path_id
             LEFT JOIN feat parent ON parent.id = f.parent_id
             WHERE f.id = ?1",
            [id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<f64>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                    r.get::<_, Option<String>>(8)?,
                    r.get::<_, String>(9)?,
                    r.get::<_, i64>(10)?,
                ))
            },
        )
        .map_err(|e| format!("找不到專長 {id}：{e}"))?;

    let categories = categories_of(conn, id).map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare("SELECT tag FROM feat_tag WHERE feat_id = ?1 ORDER BY tag")
        .map_err(|e| e.to_string())?;
    let tags: Vec<String> = stmt
        .query_map([id], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT p.kind, p.ref_feat_id, rf.name, p.ref_code, p.min_level, p.raw_text
             FROM feat_prereq p
             LEFT JOIN feat rf ON rf.id = p.ref_feat_id
             WHERE p.feat_id = ?1 ORDER BY p.seq",
        )
        .map_err(|e| e.to_string())?;
    let prereqs: Vec<Prereq> = stmt
        .query_map([id], |r| {
            Ok(Prereq {
                kind: r.get(0)?,
                ref_feat_id: r.get(1)?,
                ref_feat_name: r.get(2)?,
                ref_code: r.get(3)?,
                min_level: r.get(4)?,
                raw_text: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    // 反向鏈：誰把這個專長當成前置。查前置鏈時這一半同樣重要 ——
    // 「我點了這個之後能開出什麼」是建卡時最常問的問題。
    let mut stmt = conn
        .prepare(
            "SELECT f.id, f.name, p.min_level FROM feat_prereq p
             JOIN feat f ON f.id = p.feat_id
             WHERE p.ref_feat_id = ?1 ORDER BY f.name",
        )
        .map_err(|e| e.to_string())?;
    let dependents: Vec<Dependent> = stmt
        .query_map([id], |r| {
            Ok(Dependent {
                id: r.get(0)?,
                name: r.get(1)?,
                min_level: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT action, field, issue, reason FROM errata
             WHERE sheet = ?1 AND (source_row = ?2 OR source_row IS NULL)
             ORDER BY action",
        )
        .map_err(|e| e.to_string())?;
    let errata: Vec<ErrataNote> = stmt
        .query_map(rusqlite::params![sheet, row_no], |r| {
            Ok(ErrataNote {
                action: r.get(0)?,
                field: r.get(1)?,
                issue: r.get(2)?,
                reason: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    Ok(FeatDetail {
        cp_table: cp_table(difficulty),
        id: id.to_string(),
        name,
        feat_group: group,
        difficulty,
        difficulty_raw,
        difficulty_scale: scale,
        effect,
        categories,
        tags,
        class_name,
        class_path: path_name,
        parent_name,
        prereqs,
        dependents,
        source_sheet: sheet,
        source_row: row_no,
        errata,
    })
}

/// 等級 0～5 的 CP 成本表。
///
/// 創角色須知：CP 消耗公式為 `2^等級 × 技能難度`。等級 0 是獨立選項
/// （只為免除該技能判定的 20% 減值），不計入升級的累積成本，因此
/// 累計欄從等級 1 開始算。
fn cp_table(difficulty: Option<f64>) -> Vec<CpStep> {
    let Some(d) = difficulty else {
        return Vec::new();
    };
    let mut cumulative = 0.0;
    (0..=5)
        .map(|level| {
            let step = (2f64).powi(level as i32) * d;
            if level >= 1 {
                cumulative += step;
            }
            CpStep {
                level,
                step_cost: step,
                cumulative: if level == 0 { 0.0 } else { cumulative },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 測試都跑在真正的規則書資料庫上。造假資料在這裡沒有意義 ——
    /// 我們要驗的正是「查詢能不能正確對付這份資料的形狀」。
    fn conn() -> Connection {
        let path = dev_path();
        assert!(
            path.is_file(),
            "測試需要 dist/d100.db，請先執行 python tools/build_db.py"
        );
        open(&path).expect("開啟資料庫")
    }

    #[test]
    fn 建置資訊有內容() {
        let c = conn();
        let info = build_info(&c).unwrap();
        assert!(info.feats > 500, "專長數 {} 不合理", info.feats);
        assert_eq!(info.races, 13);
        assert!(!info.source_sha256.is_empty());
    }

    #[test]
    fn 關鍵字同時比對名稱與效果() {
        let c = conn();

        let by_name = search_feats(&c, "健壯", &[], &[], 50).unwrap();
        assert!(
            by_name.iter().any(|f| f.name == "健壯"),
            "名稱搜尋找不到〈健壯〉"
        );
        // 名稱命中要排在效果命中前面。
        assert_eq!(by_name[0].name, "健壯");

        // 這一句只出現在效果敘述裡，名稱完全沒有這幾個字。
        let by_effect = search_feats(&c, "每輪回復", &[], &[], 50).unwrap();
        assert!(!by_effect.is_empty(), "效果內文搜尋沒有結果");
        assert!(
            by_effect.iter().all(|f| !f.name.contains("每輪回復")),
            "這個關鍵字不該出現在任何名稱裡"
        );
    }

    /// 這是實際踩過的 bug：SQLite 混用 `?` 與 `?1` 時，自動編號取的是
    /// 「目前最大編號 +1」，所以沒有關鍵字（?1 排在篩選條件之後）時
    /// 參數會整個錯位，篩選變成拿搜尋樣式去比對分組。
    #[test]
    fn 無關鍵字時篩選仍然正確() {
        let c = conn();

        let groups = vec!["general".to_string(), "advanced".to_string()];
        let rows = search_feats(&c, "", &groups, &[], 300).unwrap();
        assert!(!rows.is_empty(), "只用分組篩選卻沒有結果");
        assert!(
            rows.iter().all(|f| groups.contains(&f.feat_group)),
            "結果混進了沒有被選取的分組"
        );

        let cats = vec!["戰鬥".to_string()];
        let rows = search_feats(&c, "", &groups, &cats, 300).unwrap();
        assert!(!rows.is_empty(), "分組加分類篩選沒有結果");
        assert!(
            rows.iter()
                .all(|f| groups.contains(&f.feat_group) && f.categories.contains(&"戰鬥".into())),
            "分組與分類的交集篩選不正確"
        );
    }

    #[test]
    fn 前置鏈雙向都查得到() {
        let c = conn();
        let hit = search_feats(&c, "武器專精", &[], &[], 10).unwrap();
        let id = &hit
            .iter()
            .find(|f| f.name == "武器專精")
            .expect("找不到〈武器專精〉")
            .id;

        let detail = feat_detail(&c, id).unwrap();
        assert!(
            detail.prereqs.iter().any(|p| p.ref_feat_name.as_deref() == Some("武器使用")),
            "前置應該包含〈武器使用〉"
        );
        assert!(
            detail.dependents.iter().any(|d| d.name == "高等武器專精"),
            "〈高等武器專精〉應該把〈武器專精〉當前置"
        );
    }

    /// 創角色須知的原文範例：武器使用（難度1）學到等級 3 為 (2＋4＋8) = 14 點。
    #[test]
    fn cp表符合規則書範例() {
        let table = cp_table(Some(1.0));
        assert_eq!(table.len(), 6, "應涵蓋等級 0 到 5");

        assert_eq!(table[0].step_cost, 1.0, "等級 0 的花費是 2^0 × 難度");
        assert_eq!(table[0].cumulative, 0.0, "等級 0 不計入升級累計");

        assert_eq!(table[3].level, 3);
        assert_eq!(table[3].cumulative, 14.0, "難度1學到等級3應為 14 點");
        assert_eq!(table[5].step_cost, 32.0, "難度1的等級5單級為 32 點");

        assert!(cp_table(None).is_empty(), "非數值難度不該產生成本表");
    }

    #[test]
    fn 職業專長帶得出流派() {
        let c = conn();
        let rows = search_feats(&c, "", &["class".to_string()], &[], 300).unwrap();
        assert!(!rows.is_empty());
        assert!(
            rows.iter().filter(|f| f.class_path.is_some()).count() > rows.len() / 2,
            "多數職業專長應該掛得上流派"
        );
    }
}
