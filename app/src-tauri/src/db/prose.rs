//! 散文規則章節與勘誤清單。

use rusqlite::Connection;
use serde::Serialize;

use super::refs::{RefTable, ref_tables_of};

#[derive(Serialize)]
pub struct ProseChapter {
    pub sheet: String,
    pub sections: Vec<ProseSection>,
    /// 同一張工作表上的對照表 —— 「創角色須知」的等級對照表就在那篇內文中間。
    pub tables: Vec<RefTable>,
}

#[derive(Serialize)]
pub struct ProseSection {
    pub title: Option<String>,
    pub blocks: Vec<ProseBlock>,
}

#[derive(Serialize)]
pub struct ProseBlock {
    /// 搜尋跳轉要定位到段落，所以每一段都帶得出 id。
    pub id: i64,
    /// 第二層標題。戰鬥流程就是兩層：大步驟底下再分一般／自由／即時動作。
    pub subsection: Option<String>,
    pub body: String,
}

#[derive(Serialize)]
pub struct ErrataEntry {
    pub sheet: String,
    pub source_row: Option<i64>,
    pub field: Option<String>,
    pub action: String,
    pub raw_value: Option<String>,
    pub fixed_value: Option<String>,
    pub issue: Option<String>,
    pub reason: String,
}

pub fn prose_chapter(conn: &Connection, sheet: &str) -> Result<ProseChapter, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, section, subsection, body FROM rule_text
              WHERE sheet = ?1 ORDER BY sort_order, source_row, source_col",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([sheet], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    // 依 section 收攏，但**只合併相鄰的同名 section** —— 用 map 分組會把
    // 分散在兩處的同名段落黏在一起，原表的閱讀順序就毀了。
    let mut sections: Vec<ProseSection> = Vec::new();
    for row in rows {
        let (id, section, subsection, body) = row.map_err(|e| e.to_string())?;
        let same_as_last = sections
            .last()
            .map(|s: &ProseSection| s.title == section)
            .unwrap_or(false);
        if !same_as_last {
            sections.push(ProseSection {
                title: section,
                blocks: Vec::new(),
            });
        }
        sections
            .last_mut()
            .expect("上面剛推進去")
            .blocks
            .push(ProseBlock { id, subsection, body });
    }

    Ok(ProseChapter {
        sheet: sheet.to_string(),
        sections,
        tables: ref_tables_of(conn, sheet)?,
    })
}

/// 全部勘誤，依工作表與列號排序。
///
/// 這是勘誤唯一該出現的地方 —— 它是給規則書作者對帳用的，不是玩家讀規則時
/// 要看的東西，所以不放在專長卡上。
pub fn errata_list(conn: &Connection) -> Result<Vec<ErrataEntry>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT sheet, source_row, field, action, raw_value, fixed_value, issue, reason
               FROM errata ORDER BY sheet, source_row, id",
        )
        .map_err(|e| e.to_string())?;
    let list = stmt
        .query_map([], |r| {
            Ok(ErrataEntry {
                sheet: r.get(0)?,
                source_row: r.get(1)?,
                field: r.get(2)?,
                action: r.get(3)?,
                raw_value: r.get(4)?,
                fixed_value: r.get(5)?,
                issue: r.get(6)?,
                reason: r.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_conn;

    /// 戰鬥流程是兩層的：大步驟底下再分一般／自由／即時動作。
    /// 層級弄平的話子項會被誤當成大步驟。
    #[test]
    fn 戰鬥流程保留兩層層級() {
        let c = test_conn();
        let ch = prose_chapter(&c, "戰鬥流程").unwrap();

        assert!(!ch.sections.is_empty());
        let blocks: usize = ch.sections.iter().map(|s| s.blocks.len()).sum();
        assert_eq!(blocks, 19, "戰鬥流程共 19 段");
        assert!(
            ch.sections.iter().any(|s| s.title.is_some()),
            "應該有帶標題的段落"
        );
    }

    /// 「創角色須知」的等級對照表就在那篇內文中間，散文與對照表要一起出。
    #[test]
    fn 散文章節一併帶出同表的對照表() {
        let c = test_conn();
        let ch = prose_chapter(&c, "創角色須知").unwrap();
        assert!(!ch.sections.is_empty());
        assert_eq!(ch.tables.len(), 1);
        assert_eq!(ch.tables[0].name, "等級與 CP 總數對照");
    }

    #[test]
    fn 勘誤清單完整且每筆都有理由() {
        let c = test_conn();
        let list = errata_list(&c).unwrap();
        assert_eq!(list.len(), 38);
        assert!(
            list.iter().all(|e| !e.reason.trim().is_empty()),
            "每一筆勘誤都必須寫明理由 —— 這份清單是要拿去跟作者對帳的"
        );
    }
}
