//! 職業章節：某個職業的流派、專長、被動特性與祈喚。
//!
//! 一次查完整章。切到職業頁時才呼叫，之後在章內捲動不再查詢 ——
//! 這是「一篇連貫的章」這個閱讀模型的前提。
//!
//! 四個查詢（流派、專長、特性、祈喚）加兩個附屬查詢（分類、標記），
//! 在 Rust 端組裝成巢狀結構，不做 N+1。

use std::collections::HashMap;

use rusqlite::Connection;
use serde::Serialize;

#[derive(Serialize)]
pub struct ClassChapter {
    pub class_name: String,
    /// 學派／血脈／領域／結社／契約／職業 —— 這個職業怎麼稱呼它的分支
    pub path_kind: String,
    pub paths: Vec<ClassPath>,
    /// 目前只有 Warlock 有。不是可升級的技能，消耗的是祈喚欄位而非 CP。
    pub invocations: Vec<Invocation>,
}

#[derive(Serialize)]
pub struct ClassPath {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub feats: Vec<ChapterFeat>,
    /// 被動特性：沒有難度也不能用 CP 買（武僧的「不殺」、聖騎士的
    /// 「至善之魂」）。與 feats 分開，建卡介面才不會誤以為可以學。
    pub traits: Vec<ClassTrait>,
}

#[derive(Serialize)]
pub struct ChapterFeat {
    pub id: String,
    pub name: String,
    pub difficulty: Option<f64>,
    pub difficulty_raw: Option<String>,
    pub difficulty_scale: String,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    /// 效果全文，不是預覽 —— 卡片預設就顯示完整敘述。
    pub effect: String,
    /// 有沒有前置決定卡片要不要提示「可展開」，不必為此再查一次。
    pub prereq_count: i64,
    pub source_sheet: String,
    pub source_row: i64,
}

#[derive(Serialize)]
pub struct ClassTrait {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source_sheet: String,
    pub source_row: i64,
}

#[derive(Serialize)]
pub struct Invocation {
    pub id: String,
    pub name: String,
    pub cost: Option<i64>,
    pub cost_raw: Option<String>,
    pub prereq_raw: Option<String>,
    pub effect: String,
}

pub fn class_chapter(conn: &Connection, class_name: &str) -> Result<ClassChapter, String> {
    let path_kind: String = conn
        .query_row(
            "SELECT path_kind FROM class_path WHERE class_name = ?1 LIMIT 1",
            [class_name],
            |r| r.get(0),
        )
        .map_err(|e| format!("找不到職業 {class_name}：{e}"))?;

    // 分類與標記各用一個查詢撈完整章，避免每條專長各查一次。
    let categories = multi_map(
        conn,
        "SELECT fc.feat_id, fc.category
           FROM feat_category fc
           JOIN category k ON k.code = fc.category
           JOIN feat f ON f.id = fc.feat_id
           JOIN class_path p ON p.id = f.class_path_id
          WHERE p.class_name = ?1
          ORDER BY k.sort_order",
        class_name,
    )?;
    let tags = multi_map(
        conn,
        "SELECT t.feat_id, t.tag
           FROM feat_tag t
           JOIN feat f ON f.id = t.feat_id
           JOIN class_path p ON p.id = f.class_path_id
          WHERE p.class_name = ?1
          ORDER BY t.tag",
        class_name,
    )?;

    let mut feats_by_path: HashMap<String, Vec<ChapterFeat>> = HashMap::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT f.class_path_id, f.id, f.name, f.difficulty, f.difficulty_raw,
                        f.difficulty_scale, f.effect, f.source_sheet, f.source_row,
                        (SELECT count(*) FROM feat_prereq q WHERE q.feat_id = f.id)
                   FROM feat f
                   JOIN class_path p ON p.id = f.class_path_id
                  WHERE p.class_name = ?1
                  ORDER BY f.source_col, f.source_row",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([class_name], |r| {
                let path_id: String = r.get(0)?;
                let id: String = r.get(1)?;
                Ok((
                    path_id,
                    ChapterFeat {
                        name: r.get(2)?,
                        difficulty: r.get(3)?,
                        difficulty_raw: r.get(4)?,
                        difficulty_scale: r.get(5)?,
                        effect: r.get(6)?,
                        source_sheet: r.get(7)?,
                        source_row: r.get(8)?,
                        prereq_count: r.get(9)?,
                        categories: Vec::new(),
                        tags: Vec::new(),
                        id,
                    },
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (path_id, mut feat) = row.map_err(|e| e.to_string())?;
            feat.categories = categories.get(&feat.id).cloned().unwrap_or_default();
            feat.tags = tags.get(&feat.id).cloned().unwrap_or_default();
            feats_by_path.entry(path_id).or_default().push(feat);
        }
    }

    let mut traits_by_path: HashMap<String, Vec<ClassTrait>> = HashMap::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT t.class_path_id, t.id, t.name, t.description,
                        t.source_sheet, t.source_row
                   FROM class_trait t
                   JOIN class_path p ON p.id = t.class_path_id
                  WHERE p.class_name = ?1
                  ORDER BY t.source_col, t.source_row",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([class_name], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    ClassTrait {
                        id: r.get(1)?,
                        name: r.get(2)?,
                        description: r.get(3)?,
                        source_sheet: r.get(4)?,
                        source_row: r.get(5)?,
                    },
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (path_id, t) = row.map_err(|e| e.to_string())?;
            traits_by_path.entry(path_id).or_default().push(t);
        }
    }

    let mut stmt = conn
        .prepare(
            "SELECT id, name, description FROM class_path
              WHERE class_name = ?1
              ORDER BY sort_order, source_col, source_row",
        )
        .map_err(|e| e.to_string())?;
    let paths: Vec<ClassPath> = stmt
        .query_map([class_name], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .map(|row| {
            let (id, name, description) = row.map_err(|e| e.to_string())?;
            Ok(ClassPath {
                feats: feats_by_path.remove(&id).unwrap_or_default(),
                traits: traits_by_path.remove(&id).unwrap_or_default(),
                id,
                name,
                description,
            })
        })
        .collect::<Result<_, String>>()?;

    // 祈喚不掛在 class_path 上，以工作表歸屬判斷，避免在程式裡寫死 Warlock。
    let mut stmt = conn
        .prepare(
            "SELECT id, name, cost, cost_raw, prereq_raw, effect FROM invocation
              WHERE source_sheet IN
                    (SELECT source_sheet FROM class_path WHERE class_name = ?1)
              ORDER BY source_col, source_row",
        )
        .map_err(|e| e.to_string())?;
    let invocations: Vec<Invocation> = stmt
        .query_map([class_name], |r| {
            Ok(Invocation {
                id: r.get(0)?,
                name: r.get(1)?,
                cost: r.get(2)?,
                cost_raw: r.get(3)?,
                prereq_raw: r.get(4)?,
                effect: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    Ok(ClassChapter {
        class_name: class_name.to_string(),
        path_kind,
        paths,
        invocations,
    })
}

/// 把「一個 id 對多個值」的查詢結果收成 map，保留查詢的排序。
fn multi_map(
    conn: &Connection,
    sql: &str,
    arg: &str,
) -> Result<HashMap<String, Vec<String>>, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([arg], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        let (id, value) = row.map_err(|e| e.to_string())?;
        out.entry(id).or_default().push(value);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_conn;

    #[test]
    fn warlock整章含契約專長與祈喚() {
        let c = test_conn();
        let ch = class_chapter(&c, "Warlock").unwrap();

        assert_eq!(ch.path_kind, "契約");
        assert_eq!(ch.paths.len(), 12, "Warlock 應有 12 個契約");
        assert_eq!(
            ch.paths.iter().map(|p| p.feats.len()).sum::<usize>(),
            41,
            "契約專長合計應為 41 條"
        );
        assert_eq!(ch.invocations.len(), 58, "魔能祈喚應為 58 條（已去重）");
    }

    #[test]
    fn 法師防護學派的專長依原表順序排列() {
        let c = test_conn();
        let ch = class_chapter(&c, "法師").unwrap();

        assert_eq!(ch.paths.len(), 9, "法師應有 9 個學派");
        assert_eq!(ch.paths[0].name, "必備技能", "必備技能應排在最前");

        let ward = ch.paths.iter().find(|p| p.name == "防護").expect("找不到防護學派");
        assert_eq!(ward.feats.len(), 5);
        assert_eq!(ward.feats[0].name, "奧術防禦", "首條應為難度 1 的〈奧術防禦〉");
        assert_eq!(ward.feats[4].name, "法術瞬時反制");

        // 〈學派專精〉的分類是靠勘誤補上的，這裡順帶驗證勘誤有進到章節查詢。
        let core = &ch.paths[0].feats[0];
        assert_eq!(core.categories, vec!["知識".to_string()]);
    }

    #[test]
    fn 被動特性不混進專長() {
        let c = test_conn();
        let ch = class_chapter(&c, "神掌門").unwrap();

        let traits: usize = ch.paths.iter().map(|p| p.traits.len()).sum();
        assert_eq!(traits, 9, "如來神掌的九式應全數落在 traits");
        assert!(
            ch.paths.iter().flat_map(|p| &p.feats).all(|f| f.difficulty.is_some()
                || f.difficulty_raw.is_some()),
            "feats 裡不該出現沒有難度的條目 —— 那些是被動特性"
        );
    }
}
