//! 章節目錄：整本書的結構。
//!
//! 章節與頁籤的定義**寫死在這裡，不進資料庫**。哪一章放哪些工作表是我們的
//! 編輯判斷，不是試算表的內容 —— 這條界線跟 `data/raw` 不做語意判斷是同
//! 一個道理。條目數則一律由查詢算出，不寫死，資料變了數字就跟著變。

use rusqlite::Connection;
use serde::Serialize;

#[derive(Serialize)]
pub struct Toc {
    pub chapters: Vec<Chapter>,
}

#[derive(Serialize)]
pub struct Chapter {
    /// 前端路由用的穩定代碼
    pub key: String,
    pub title: String,
    pub tabs: Vec<Tab>,
}

#[derive(Serialize)]
pub struct Tab {
    /// 查詢參數：職業名、feat_group、工作表名⋯依 kind 而定
    pub key: String,
    pub title: String,
    /// 決定前端用哪個章節元件渲染
    pub kind: String,
    pub count: i64,
}

/// 職業頁籤的順序。七個主要職業在前，特殊職業在後 —— 這是編輯決定，
/// 資料庫的 `sort_order` 只在同一張工作表內有意義（多數職業都是 0），
/// 給不出跨職業的順序。清單本身仍從資料庫取，這裡只決定先後；
/// 沒列到的職業會接在最後，不會消失。
const CLASS_ORDER: &[&str] = &[
    "法師",
    "術士",
    "牧師",
    "德魯伊",
    "吟遊詩人",
    "武僧",
    "Warlock",
    "賢者",
    "奈瑟瑞爾奧術師",
    "咒火使者",
    "聖騎士（奉獻之誓）",
    "魔法舞者",
    "魔射手",
    "神掌門",
];

/// 散文頁籤：(工作表名, 顯示標題)。
const RULE_SHEETS: &[(&str, &str)] = &[
    ("創角色須知", "創角流程"),
    ("施法者創角須知", "施法者"),
    ("戰鬥流程", "戰鬥流程"),
    ("FATE特規", "FATE 特規"),
];

const APPENDIX_SHEETS: &[(&str, &str)] = &[
    ("世界觀", "世界觀"),
    ("大陸簡史", "大陸簡史"),
    ("Patch note", "Patch note"),
    ("狩獵任務", "狩獵任務"),
];

/// 專長分組：(feat_group, 顯示標題)。順序照規則書的難度遞進。
const FEAT_GROUPS: &[(&str, &str)] = &[
    ("basic", "基本專長"),
    ("general", "一般專長"),
    ("crafting", "製作專長"),
    ("metamagic", "超魔專長"),
    ("advanced", "高級專長"),
    ("legendary", "傳奇專長"),
];

const AFFIX_TIERS: &[(&str, &str)] = &[
    ("general", "一般詞綴"),
    ("advanced", "高階詞綴"),
    ("eternal", "永恆聖器"),
];

/// 某張散文工作表住在哪一章哪一頁。
///
/// 搜尋要跳到散文段落時需要這個對應，而對應的真相就是上面那兩張常數表 ——
/// 在 search.rs 另寫一份遲早會對不起來。專長表的前言（基本專長、傳奇專長
/// 那些）不是獨立頁籤，它們顯示在該組專長的章節裡，所以回 None。
pub(crate) fn prose_address(sheet: &str) -> Option<(String, String)> {
    if RULE_SHEETS.iter().any(|(s, _)| *s == sheet) {
        return Some(("rules".into(), sheet.into()));
    }
    if APPENDIX_SHEETS.iter().any(|(s, _)| *s == sheet) {
        return Some(("appendix".into(), sheet.into()));
    }
    None
}

pub fn toc(conn: &Connection) -> Result<Toc, String> {
    Ok(Toc {
        chapters: vec![
            Chapter {
                key: "rules".into(),
                title: "創角規則".into(),
                tabs: prose_tabs(conn, RULE_SHEETS)?,
            },
            Chapter {
                key: "feats".into(),
                title: "專長".into(),
                tabs: feat_tabs(conn)?,
            },
            Chapter {
                key: "classes".into(),
                title: "職業".into(),
                tabs: class_tabs(conn)?,
            },
            Chapter {
                key: "items".into(),
                title: "物品".into(),
                tabs: item_tabs(conn)?,
            },
            Chapter {
                key: "races".into(),
                title: "種族".into(),
                tabs: vec![Tab {
                    key: "all".into(),
                    title: "種族與其調整".into(),
                    kind: "race".into(),
                    count: count(conn, "SELECT count(*) FROM race", &[]),
                }],
            },
            Chapter {
                key: "sheets".into(),
                title: "角色卡".into(),
                tabs: vec![Tab {
                    key: "all".into(),
                    title: "我的角色".into(),
                    kind: "sheets".into(),
                    // 角色卡在使用者資料夾裡，不是資料庫的內容，所以這裡
                    // 給不出條目數 —— 數量由前端自己列。
                    count: 0,
                }],
            },
            Chapter {
                key: "appendix".into(),
                title: "附錄".into(),
                tabs: appendix_tabs(conn)?,
            },
        ],
    })
}

/// 散文頁籤的條目數含該工作表的對照表 —— 前端會把兩者渲染在同一頁
/// （「創角色須知」的等級對照表就在該篇內文中間）。
fn prose_tabs(conn: &Connection, sheets: &[(&str, &str)]) -> Result<Vec<Tab>, String> {
    Ok(sheets
        .iter()
        .map(|(sheet, title)| Tab {
            key: (*sheet).into(),
            title: (*title).into(),
            kind: "prose".into(),
            count: count(conn, "SELECT count(*) FROM rule_text WHERE sheet = ?1", &[sheet])
                + count(conn, "SELECT count(*) FROM ref_table WHERE sheet = ?1", &[sheet]),
        })
        .collect())
}

fn feat_tabs(conn: &Connection) -> Result<Vec<Tab>, String> {
    Ok(FEAT_GROUPS
        .iter()
        .map(|(group, title)| Tab {
            key: (*group).into(),
            title: (*title).into(),
            kind: "feats".into(),
            count: count(conn, "SELECT count(*) FROM feat WHERE feat_group = ?1", &[group]),
        })
        .collect())
}

fn class_tabs(conn: &Connection) -> Result<Vec<Tab>, String> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT class_name FROM class_path")
        .map_err(|e| e.to_string())?;
    let mut names: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    // 照 CLASS_ORDER 排；沒列到的接在最後（依名稱），確保不會有職業消失。
    names.sort_by_key(|n| {
        (
            CLASS_ORDER.iter().position(|c| c == n).unwrap_or(usize::MAX),
            n.clone(),
        )
    });

    Ok(names
        .into_iter()
        .map(|name| {
            // 條目數＝專長＋被動特性＋祈喚。祈喚不掛在 class_path 上，
            // 改以工作表歸屬判斷，免得在這裡寫死 Warlock。
            let n = count(
                conn,
                "SELECT (SELECT count(*) FROM feat f
                           JOIN class_path p ON p.id = f.class_path_id
                          WHERE p.class_name = ?1)
                      + (SELECT count(*) FROM class_trait t
                           JOIN class_path p ON p.id = t.class_path_id
                          WHERE p.class_name = ?1)
                      + (SELECT count(*) FROM invocation i
                          WHERE i.source_sheet IN
                                (SELECT source_sheet FROM class_path WHERE class_name = ?1))",
                &[&name],
            );
            Tab {
                key: name.clone(),
                title: name,
                kind: "class".into(),
                count: n,
            }
        })
        .collect())
}

fn item_tabs(conn: &Connection) -> Result<Vec<Tab>, String> {
    let mut tabs: Vec<Tab> = AFFIX_TIERS
        .iter()
        .map(|(tier, title)| Tab {
            key: (*tier).into(),
            title: (*title).into(),
            kind: "affix".into(),
            count: count(conn, "SELECT count(*) FROM affix WHERE affix_tier = ?1", &[tier]),
        })
        .collect();

    tabs.push(Tab {
        key: "material".into(),
        title: "素材詞綴".into(),
        kind: "material".into(),
        count: count(conn, "SELECT count(*) FROM material", &[]),
    });
    tabs.push(Tab {
        key: "魔法物品價格表".into(),
        title: "價格表".into(),
        kind: "ref_sheet".into(),
        count: count(conn, "SELECT count(*) FROM ref_table WHERE sheet = ?1", &[&"魔法物品價格表"]),
    });
    tabs.push(Tab {
        key: "distribution".into(),
        title: "詞綴分布".into(),
        kind: "affix_distribution".into(),
        count: count(conn, "SELECT count(*) FROM affix_distribution", &[]),
    });
    Ok(tabs)
}

fn appendix_tabs(conn: &Connection) -> Result<Vec<Tab>, String> {
    let mut tabs = prose_tabs(conn, APPENDIX_SHEETS)?;
    tabs.push(Tab {
        key: "ref_index".into(),
        title: "對照表總覽".into(),
        kind: "ref_index".into(),
        count: count(conn, "SELECT count(*) FROM ref_table", &[]),
    });
    tabs.push(Tab {
        key: "cp".into(),
        title: "CP 試算".into(),
        kind: "cp_calc".into(),
        count: 0,
    });
    tabs.push(Tab {
        key: "errata".into(),
        title: "勘誤清單".into(),
        kind: "errata".into(),
        count: count(conn, "SELECT count(*) FROM errata", &[]),
    });
    // 修改紀錄只在開發模式出現 —— 它讀的是 git 與 repo 裡的檔案，
    // 打包版兩者都沒有。
    if crate::editing::editing_enabled() {
        tabs.push(Tab {
            key: "history".into(),
            title: "修改紀錄".into(),
            kind: "history".into(),
            count: 0,
        });
    }
    Ok(tabs)
}

/// 數量查詢失敗時回 0 而不是讓整份目錄失敗 —— 目錄少一個數字還能用，
/// 整個應用開不起來就不能用了。
fn count(conn: &Connection, sql: &str, args: &[&dyn rusqlite::ToSql]) -> i64 {
    conn.query_row(sql, args, |r| r.get(0)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_conn;

    #[test]
    fn 目錄涵蓋六個章節且條目數非零() {
        let c = test_conn();
        let toc = toc(&c).unwrap();

        let keys: Vec<&str> = toc.chapters.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(
            keys,
            vec!["rules", "feats", "classes", "items", "races", "sheets", "appendix"]
        );

        for chapter in &toc.chapters {
            assert!(!chapter.tabs.is_empty(), "章節「{}」沒有頁籤", chapter.title);
            for tab in &chapter.tabs {
                // 工具頁（CP 試算、修改紀錄）沒有條目數。
                if matches!(tab.kind.as_str(), "cp_calc" | "history" | "sheets") {
                    continue;
                }
                assert!(
                    tab.count > 0,
                    "「{}／{}」的條目數為 0",
                    chapter.title,
                    tab.title
                );
            }
        }
    }

    #[test]
    fn 職業頁籤涵蓋全部十四個職業且順序固定() {
        let c = test_conn();
        let toc = toc(&c).unwrap();
        let classes = &toc.chapters.iter().find(|c| c.key == "classes").unwrap().tabs;

        assert_eq!(classes.len(), 14, "職業數與資料庫不符");
        assert_eq!(classes[0].title, "法師");
        assert_eq!(classes[6].title, "Warlock");

        // Warlock 的條目數要含 58 條祈喚，不能只算專長與特性。
        let warlock = classes.iter().find(|t| t.key == "Warlock").unwrap();
        assert!(
            warlock.count >= 41 + 58,
            "Warlock 條目數 {} 應含 41 條專長與 58 條祈喚",
            warlock.count
        );
    }
}
