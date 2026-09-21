//! 跨全書搜尋。
//!
//! 搜尋是導覽工具，不是另一種閱讀模式 —— 每筆結果都帶著「它在哪一章哪一頁」
//! 的位址，前端選中後切章、捲動、高亮，而不是開一個獨立的結果列表。
//!
//! 同時比對名稱與效果內文：規則書的效果敘述往往才是玩家記得的那一句
//! （「每輪回復1d10hp」），只比對名稱等於少掉一半的用處。

use rusqlite::Connection;
use serde::Serialize;

use super::{feat_address, preview};

#[derive(Serialize)]
pub struct SearchHit {
    /// feat / class_trait / invocation
    pub kind: String,
    pub id: String,
    pub name: String,
    /// 章節代碼，對應 toc 的 Chapter.key
    pub chapter: String,
    /// 頁籤代碼，對應 toc 的 Tab.key
    pub tab: String,
    /// 顯示用的所在位置，例如「法師 · 防護」
    pub context: String,
    pub preview: String,
    /// 名稱命中排在效果命中前面：找「健壯」的人多半要的是那個技能本身。
    pub matched_name: bool,
}

pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<SearchHit>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let pattern = format!("%{q}%");

    let mut hits = Vec::new();
    collect_feats(conn, &pattern, q, &mut hits)?;
    collect_traits(conn, &pattern, q, &mut hits)?;
    collect_invocations(conn, &pattern, q, &mut hits)?;

    // 名稱命中優先，其次照章節與名稱，讓同一次搜尋的結果順序穩定。
    hits.sort_by(|a, b| {
        b.matched_name
            .cmp(&a.matched_name)
            .then_with(|| a.chapter.cmp(&b.chapter))
            .then_with(|| a.name.cmp(&b.name))
    });
    hits.truncate(limit);
    Ok(hits)
}

fn collect_feats(
    conn: &Connection,
    pattern: &str,
    needle: &str,
    out: &mut Vec<SearchHit>,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT f.id, f.name, f.feat_group, f.effect, p.class_name, p.name
               FROM feat f
               LEFT JOIN class_path p ON p.id = f.class_path_id
              WHERE f.name LIKE ?1 OR f.effect LIKE ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([pattern], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    for row in rows {
        let (id, name, group, effect, class_name, path_name) = row.map_err(|e| e.to_string())?;
        let context = match (&class_name, &path_name) {
            (Some(c), Some(p)) => format!("{c} · {p}"),
            (Some(c), None) => c.clone(),
            _ => String::new(),
        };
        let (chapter, tab) = feat_address(&group, class_name);
        out.push(SearchHit {
            kind: "feat".into(),
            matched_name: name.contains(needle),
            preview: preview(&effect, 70),
            id,
            name,
            chapter,
            tab,
            context,
        });
    }
    Ok(())
}

fn collect_traits(
    conn: &Connection,
    pattern: &str,
    needle: &str,
    out: &mut Vec<SearchHit>,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.name, t.description, p.class_name, p.name
               FROM class_trait t
               JOIN class_path p ON p.id = t.class_path_id
              WHERE t.name LIKE ?1 OR t.description LIKE ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([pattern], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    for row in rows {
        let (id, name, description, class_name, path_name) = row.map_err(|e| e.to_string())?;
        out.push(SearchHit {
            kind: "class_trait".into(),
            matched_name: name.contains(needle),
            preview: preview(&description, 70),
            context: format!("{class_name} · {path_name}"),
            chapter: "classes".into(),
            tab: class_name,
            id,
            name,
        });
    }
    Ok(())
}

fn collect_invocations(
    conn: &Connection,
    pattern: &str,
    needle: &str,
    out: &mut Vec<SearchHit>,
) -> Result<(), String> {
    // 祈喚不掛在 class_path 上，以工作表歸屬找回它屬於哪個職業，
    // 避免在程式裡寫死 Warlock。
    let mut stmt = conn
        .prepare(
            "SELECT i.id, i.name, i.effect,
                    (SELECT c.class_name FROM class_path c
                      WHERE c.source_sheet = i.source_sheet LIMIT 1)
               FROM invocation i
              WHERE i.name LIKE ?1 OR i.effect LIKE ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([pattern], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    for row in rows {
        let (id, name, effect, class_name) = row.map_err(|e| e.to_string())?;
        let class_name = class_name.unwrap_or_default();
        out.push(SearchHit {
            kind: "invocation".into(),
            matched_name: name.contains(needle),
            preview: preview(&effect, 70),
            context: format!("{class_name} · 魔能祈喚"),
            chapter: "classes".into(),
            tab: class_name,
            id,
            name,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_conn;

    #[test]
    fn 搜尋結果帶得出可跳轉的位址() {
        let c = test_conn();

        // 職業專長要指向職業章節的該職業頁，不是專長章節。
        let hits = search(&c, "奧術防禦", 50).unwrap();
        let hit = hits
            .iter()
            .find(|h| h.name == "奧術防禦")
            .expect("找不到〈奧術防禦〉");
        assert_eq!(hit.chapter, "classes");
        assert_eq!(hit.tab, "法師");
        assert_eq!(hit.context, "法師 · 防護");
        assert!(hit.matched_name);

        // 非職業專長指向專長章節，頁籤是 feat_group。
        let hits = search(&c, "健壯", 50).unwrap();
        let hit = hits.iter().find(|h| h.name == "健壯").expect("找不到〈健壯〉");
        assert_eq!(hit.chapter, "feats");
        assert!(
            hit.tab == "basic" || hit.tab == "general",
            "〈健壯〉的頁籤應該是某個 feat_group，實際 {}",
            hit.tab
        );
    }

    #[test]
    fn 名稱命中排在效果命中前面() {
        let c = test_conn();
        let hits = search(&c, "健壯", 50).unwrap();
        assert_eq!(hits[0].name, "健壯");

        // 這一句只出現在效果敘述裡，名稱完全沒有這幾個字。
        let hits = search(&c, "每輪回復", 50).unwrap();
        assert!(!hits.is_empty(), "效果內文搜尋沒有結果");
        assert!(
            hits.iter().all(|h| !h.matched_name),
            "這個關鍵字不該出現在任何名稱裡"
        );
    }

    #[test]
    fn 搜尋涵蓋非專長型別() {
        let c = test_conn();

        let hits = search(&c, "魔能光束", 50).unwrap();
        let hit = hits
            .iter()
            .find(|h| h.kind == "invocation")
            .expect("找不到祈喚〈魔能光束〉");
        assert_eq!(hit.chapter, "classes");
        assert_eq!(hit.tab, "Warlock");

        let hits = search(&c, "不殺", 50).unwrap();
        let hit = hits
            .iter()
            .find(|h| h.kind == "class_trait")
            .expect("找不到被動特性〈不殺〉");
        assert_eq!(hit.tab, "武僧");
    }

    #[test]
    fn 空字串不回結果() {
        let c = test_conn();
        assert!(search(&c, "   ", 50).unwrap().is_empty());
    }
}
