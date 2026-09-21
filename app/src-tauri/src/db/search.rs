//! 搜尋與分面統計。
//!
//! 註：`search_feats` 與 `facets` 是為改版前的「清單＋篩選器」介面設計的，
//! 會在階段 1 後段由章節式的 `search` 取代。

use rusqlite::{Connection, Row};
use serde::Serialize;

use super::feat::FeatSummary;
use super::{categories_of, numbered, preview};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_conn;

    #[test]
    fn 關鍵字同時比對名稱與效果() {
        let c = test_conn();

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
        let c = test_conn();

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
    fn 職業專長帶得出流派() {
        let c = test_conn();
        let rows = search_feats(&c, "", &["class".to_string()], &[], 300).unwrap();
        assert!(!rows.is_empty());
        assert!(
            rows.iter().filter(|f| f.class_path.is_some()).count() > rows.len() / 2,
            "多數職業專長應該掛得上流派"
        );
    }
}
