//! 專長的資料契約與詳情查詢。

use rusqlite::Connection;
use serde::Serialize;

use super::{feat_address, multi_map};

/// 章節裡的一張專長卡。與 `FeatDetail` 的差別是這裡不查前置與勘誤 ——
/// 那些等使用者點開才查（`entry_detail`），整章一次撈會白做幾百次。
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
pub struct FeatChapter {
    pub group: String,
    /// 該工作表表頭之前的前言段落。傳奇專長那段說明了傳奇技能點怎麼換算，
    /// 不放進來讀者會看不懂難度欄的「傳1」是什麼。
    pub intro: Vec<String>,
    pub sections: Vec<CategorySection>,
    /// 沒有分類的條目（例如〈大師之觸- 金繼〉分類欄整格空白）。
    /// 不塞進任何一節，也不隱藏 —— 資料缺口該看得見。
    pub uncategorized: Vec<ChapterFeat>,
}

#[derive(Serialize)]
pub struct CategorySection {
    pub category: String,
    pub feats: Vec<ChapterFeat>,
}

#[derive(Serialize)]
pub struct Prereq {
    pub kind: String,
    pub ref_feat_id: Option<String>,
    pub ref_feat_name: Option<String>,
    pub ref_code: Option<String>,
    pub min_level: Option<i64>,
    pub raw_text: String,
    /// 被指向的那條專長住在哪一章哪一頁。前置可能跨章（職業專長的前置
    /// 常常是一般專長），光有 id 跳不過去，所以查詢時一併帶上 ——
    /// 這比讓前端為了跳轉再往返一次省事。
    pub ref_chapter: Option<String>,
    pub ref_tab: Option<String>,
}

#[derive(Serialize)]
pub struct Dependent {
    pub id: String,
    pub name: String,
    pub min_level: Option<i64>,
    pub chapter: String,
    pub tab: String,
}

#[derive(Serialize)]
pub struct EntryDetail {
    pub id: String,
    pub kind: String,
    pub prereqs: Vec<Prereq>,
    pub dependents: Vec<Dependent>,
    pub source_sheet: String,
    pub source_row: i64,
    /// 並排區塊的欄號。編輯時要寫進勘誤的 `col:` 才定位得到正確的條目。
    pub source_col: i64,
}

/// 展開一張卡片時才查的內容：前置鏈、被誰當前置、來源列號。
///
/// 卡片已經有名稱、難度、分類與效果全文了（章節查詢就帶了），這裡只補
/// 展開後才看得到的部分，不重複傳一次。
///
/// `kind` 目前只支援 `feat`。`class_trait` 與 `invocation` 沒有前置鏈，
/// 階段 2 接上詞綴時再擴充。
pub fn entry_detail(conn: &Connection, kind: &str, id: &str) -> Result<EntryDetail, String> {
    if kind != "feat" {
        return Err(format!("尚未支援的條目種類：{kind}"));
    }

    let (sheet, row_no, col_no) = conn
        .query_row(
            "SELECT source_sheet, source_row, source_col FROM feat WHERE id = ?1",
            [id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)),
        )
        .map_err(|e| format!("找不到專長 {id}：{e}"))?;

    let mut stmt = conn
        .prepare(
            "SELECT p.kind, p.ref_feat_id, rf.name, p.ref_code, p.min_level, p.raw_text,
                    rf.feat_group, rp.class_name
             FROM feat_prereq p
             LEFT JOIN feat rf ON rf.id = p.ref_feat_id
             LEFT JOIN class_path rp ON rp.id = rf.class_path_id
             WHERE p.feat_id = ?1 ORDER BY p.seq",
        )
        .map_err(|e| e.to_string())?;
    let prereqs: Vec<Prereq> = stmt
        .query_map([id], |r| {
            let group: Option<String> = r.get(6)?;
            let class_name: Option<String> = r.get(7)?;
            let (ref_chapter, ref_tab) = match group {
                Some(g) => {
                    let (c, t) = feat_address(&g, class_name);
                    (Some(c), Some(t))
                }
                // 前置指向的專長還沒建檔（kind=free 或 ref_code 只有名稱）時
                // 沒有位址可跳，留 None 讓前端顯示成純文字。
                None => (None, None),
            };
            Ok(Prereq {
                kind: r.get(0)?,
                ref_feat_id: r.get(1)?,
                ref_feat_name: r.get(2)?,
                ref_code: r.get(3)?,
                min_level: r.get(4)?,
                raw_text: r.get(5)?,
                ref_chapter,
                ref_tab,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    // 反向鏈：誰把這個專長當成前置。查前置鏈時這一半同樣重要 ——
    // 「我點了這個之後能開出什麼」是建卡時最常問的問題。
    let mut stmt = conn
        .prepare(
            "SELECT f.id, f.name, p.min_level, f.feat_group, cp.class_name
             FROM feat_prereq p
             JOIN feat f ON f.id = p.feat_id
             LEFT JOIN class_path cp ON cp.id = f.class_path_id
             WHERE p.ref_feat_id = ?1 ORDER BY f.name",
        )
        .map_err(|e| e.to_string())?;
    let dependents: Vec<Dependent> = stmt
        .query_map([id], |r| {
            let group: String = r.get(3)?;
            let (chapter, tab) = feat_address(&group, r.get(4)?);
            Ok(Dependent {
                id: r.get(0)?,
                name: r.get(1)?,
                min_level: r.get(2)?,
                chapter,
                tab,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    Ok(EntryDetail {
        id: id.to_string(),
        kind: kind.to_string(),
        prereqs,
        dependents,
        source_sheet: sheet,
        source_row: row_no,
        source_col: col_no,
    })
}

/// 全部專長的難度，供 CP 試算工具挑選。
///
/// 刻意不帶效果全文 —— 這是給挑選器用的清單，612 筆各自再塞一段敘述就
/// 變成幾百 KB，而挑選器根本不顯示效果。
#[derive(Serialize)]
pub struct FeatDifficulty {
    pub id: String,
    pub name: String,
    pub feat_group: String,
    pub difficulty: Option<f64>,
    pub difficulty_raw: Option<String>,
    pub difficulty_scale: String,
    pub class_name: Option<String>,
}

pub fn feat_difficulties(conn: &Connection) -> Result<Vec<FeatDifficulty>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT f.id, f.name, f.feat_group, f.difficulty, f.difficulty_raw,
                    f.difficulty_scale, p.class_name
               FROM feat f
               LEFT JOIN class_path p ON p.id = f.class_path_id
              ORDER BY f.name",
        )
        .map_err(|e| e.to_string())?;
    let list = stmt
        .query_map([], |r| {
            Ok(FeatDifficulty {
                id: r.get(0)?,
                name: r.get(1)?,
                feat_group: r.get(2)?,
                difficulty: r.get(3)?,
                difficulty_raw: r.get(4)?,
                difficulty_scale: r.get(5)?,
                class_name: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    Ok(list)
}

/// 非職業的六組專長，依分類分節。
///
/// 一個專長可屬於多個分類（原表的「戰鬥/運動」「知識、感知或交涉」），
/// 因此**同一條會出現在多個分節** —— 這是刻意的，找「戰鬥/運動」的專長時
/// 兩節都該找得到。各節的聯集會大於該組的條目總數。
pub fn feat_chapter(conn: &Connection, group: &str) -> Result<FeatChapter, String> {
    let categories = multi_map(
        conn,
        "SELECT fc.feat_id, fc.category
           FROM feat_category fc
           JOIN feat f ON f.id = fc.feat_id
           JOIN category k ON k.code = fc.category
          WHERE f.feat_group = ?1
          ORDER BY k.sort_order",
        group,
    )?;
    let tags = multi_map(
        conn,
        "SELECT t.feat_id, t.tag
           FROM feat_tag t
           JOIN feat f ON f.id = t.feat_id
          WHERE f.feat_group = ?1
          ORDER BY t.tag",
        group,
    )?;

    let mut stmt = conn
        .prepare(
            "SELECT f.id, f.name, f.difficulty, f.difficulty_raw, f.difficulty_scale,
                    f.effect, f.source_sheet, f.source_row,
                    (SELECT count(*) FROM feat_prereq q WHERE q.feat_id = f.id)
               FROM feat f
              WHERE f.feat_group = ?1
              ORDER BY f.source_row",
        )
        .map_err(|e| e.to_string())?;
    let feats: Vec<ChapterFeat> = stmt
        .query_map([group], |r| {
            let id: String = r.get(0)?;
            Ok(ChapterFeat {
                name: r.get(1)?,
                difficulty: r.get(2)?,
                difficulty_raw: r.get(3)?,
                difficulty_scale: r.get(4)?,
                effect: r.get(5)?,
                source_sheet: r.get(6)?,
                source_row: r.get(7)?,
                prereq_count: r.get(8)?,
                categories: categories.get(&id).cloned().unwrap_or_default(),
                tags: tags.get(&id).cloned().unwrap_or_default(),
                id,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    // 分節順序照 category.sort_order，不是照出現次數 —— 讀者記得的是
    // 規則書那六個分類的固定順序。
    let mut order = conn
        .prepare("SELECT code FROM category ORDER BY sort_order")
        .map_err(|e| e.to_string())?;
    let codes: Vec<String> = order
        .query_map([], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    let mut sections: Vec<CategorySection> = Vec::new();
    for code in codes {
        let in_section: Vec<ChapterFeat> = feats
            .iter()
            .filter(|f| f.categories.contains(&code))
            .map(clone_feat)
            .collect();
        if !in_section.is_empty() {
            sections.push(CategorySection {
                category: code,
                feats: in_section,
            });
        }
    }

    let uncategorized: Vec<ChapterFeat> = feats
        .iter()
        .filter(|f| f.categories.is_empty())
        .map(clone_feat)
        .collect();

    // 前言：該組所屬工作表在表頭之前的段落。工作表名從資料取，不寫死。
    let mut stmt = conn
        .prepare(
            "SELECT body FROM rule_text
              WHERE sheet IN (SELECT DISTINCT source_sheet FROM feat WHERE feat_group = ?1)
              ORDER BY sort_order",
        )
        .map_err(|e| e.to_string())?;
    let intro: Vec<String> = stmt
        .query_map([group], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    Ok(FeatChapter {
        group: group.to_string(),
        intro,
        sections,
        uncategorized,
    })
}

/// 多分類的條目會出現在好幾節，各節持有自己的一份。
fn clone_feat(f: &ChapterFeat) -> ChapterFeat {
    ChapterFeat {
        id: f.id.clone(),
        name: f.name.clone(),
        difficulty: f.difficulty,
        difficulty_raw: f.difficulty_raw.clone(),
        difficulty_scale: f.difficulty_scale.clone(),
        categories: f.categories.clone(),
        tags: f.tags.clone(),
        effect: f.effect.clone(),
        prereq_count: f.prereq_count,
        source_sheet: f.source_sheet.clone(),
        source_row: f.source_row,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::search::search;
    use crate::db::test_conn;

    #[test]
    fn 前置鏈雙向都查得到() {
        let c = test_conn();
        let hit = search(&c, "武器專精", 20).unwrap();
        let id = &hit
            .iter()
            .find(|f| f.name == "武器專精")
            .expect("找不到〈武器專精〉")
            .id;

        let detail = entry_detail(&c, "feat", id).unwrap();
        assert!(
            detail.prereqs.iter().any(|p| p.ref_feat_name.as_deref() == Some("武器使用")),
            "前置應該包含〈武器使用〉"
        );
        assert!(
            detail.dependents.iter().any(|d| d.name == "高等武器專精"),
            "〈高等武器專精〉應該把〈武器專精〉當前置"
        );

        // 光有 id 跳不過去，前置與後續都要帶得出位址。
        let p = detail
            .prereqs
            .iter()
            .find(|p| p.ref_feat_name.as_deref() == Some("武器使用"))
            .unwrap();
        assert!(p.ref_chapter.is_some() && p.ref_tab.is_some(), "前置缺少位址");
        let d = detail
            .dependents
            .iter()
            .find(|d| d.name == "高等武器專精")
            .unwrap();
        assert!(!d.chapter.is_empty() && !d.tab.is_empty(), "後續缺少位址");
    }

    /// 〈知識〉〈語言〉的難度是 `1or2`，依 2026-09-20 的裁示由 DM 個案裁定。
    /// 挑選器必須看得出「這條沒有固定難度」，才能提示使用者自己選。
    #[test]
    fn 難度清單保留非數值的原文() {
        let c = test_conn();
        let list = feat_difficulties(&c).unwrap();
        assert_eq!(list.len(), 612);

        let knowledge = list.iter().find(|f| f.name == "知識").expect("找不到〈知識〉");
        assert!(knowledge.difficulty.is_none());
        assert_eq!(knowledge.difficulty_raw.as_deref(), Some("1or2"));

        assert!(
            list.iter().any(|f| f.difficulty_scale == "legend"),
            "應該帶得出以傳奇技能點計價的條目"
        );
    }

    #[test]
    fn 一般專長依分類分節且不漏條目() {
        let c = test_conn();
        let ch = feat_chapter(&c, "general").unwrap();

        let mut ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for s in &ch.sections {
            for f in &s.feats {
                ids.insert(f.id.as_str());
            }
        }
        for f in &ch.uncategorized {
            ids.insert(f.id.as_str());
        }
        assert_eq!(ids.len(), 103, "一般專長 103 條應全部出現在某一節");
    }

    /// 多分類是刻意的：「戰鬥/運動」的專長在兩節都該找得到，
    /// 所以各節條目數的總和會大於該組的條目總數。
    #[test]
    fn 多分類的專長出現在每一個所屬分節() {
        let c = test_conn();
        let ch = feat_chapter(&c, "general").unwrap();

        let multi = ch
            .sections
            .iter()
            .flat_map(|s| &s.feats)
            .find(|f| f.categories.len() > 1)
            .expect("一般專長裡應該有多分類的條目");

        for cat in &multi.categories {
            let section = ch
                .sections
                .iter()
                .find(|s| &s.category == cat)
                .unwrap_or_else(|| panic!("找不到分節「{cat}」"));
            assert!(
                section.feats.iter().any(|f| f.id == multi.id),
                "〈{}〉屬於「{}」卻沒出現在該節",
                multi.name,
                cat
            );
        }

        let total: usize = ch.sections.iter().map(|s| s.feats.len()).sum();
        assert!(total > 103, "有多分類條目時各節總和應大於 103，實際 {total}");
    }

    /// 傳奇專長的前言說明了傳奇技能點怎麼換算，沒有它讀者看不懂「傳1」。
    #[test]
    fn 傳奇專長帶得出前言與兩套計價() {
        let c = test_conn();
        let ch = feat_chapter(&c, "legendary").unwrap();

        assert!(!ch.intro.is_empty(), "傳奇專長應該有前言");
        let all: Vec<&ChapterFeat> = ch.sections.iter().flat_map(|s| &s.feats).collect();
        assert!(
            all.iter().any(|f| f.difficulty_scale == "legend"),
            "應有以傳奇技能點計價的條目"
        );
        assert!(
            all.iter().any(|f| f.difficulty_scale == "cp"),
            "應有走一般 CP 公式的條目"
        );
    }
}
