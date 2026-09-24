//! 物品章節：詞綴、素材、價格表、詞綴分布。

use std::collections::HashMap;

use rusqlite::Connection;
use serde::Serialize;

use super::refs::{RefTable, ref_tables_of};

#[derive(Serialize)]
pub struct AffixChapter {
    pub tier: String,
    pub intro: Vec<String>,
    pub affixes: Vec<Affix>,
}

#[derive(Serialize)]
pub struct Affix {
    pub id: String,
    pub name: String,
    pub slots: Vec<String>,
    pub ranks: Vec<AffixRank>,
    pub plus_cost_raw: Option<String>,
    pub effect: String,
    pub source_sheet: String,
    pub source_row: i64,
}

#[derive(Serialize)]
pub struct AffixRank {
    pub rank: i64,
    pub plus_cost: i64,
    /// 非空時代表這是「同一階在不同裝備上的價碼」，不是另一個階級 ——
    /// 〈幽冥〉的「3（鎧甲、盾牌）／2（武器）」兩筆 rank 都是 1。
    pub condition: String,
}

#[derive(Serialize)]
pub struct MaterialChapter {
    pub intro: Vec<String>,
    pub materials: Vec<Material>,
}

#[derive(Serialize)]
pub struct Material {
    pub id: String,
    pub name: String,
    pub material_tier: Option<i64>,
    pub cost_multiplier: Option<f64>,
    pub slots: Vec<String>,
    pub affixes: Vec<MaterialAffix>,
    pub source_sheet: String,
    pub source_row: i64,
}

#[derive(Serialize)]
pub struct MaterialAffix {
    pub id: String,
    pub name: String,
    pub tier_rank: i64,
    pub rarity_multiplier: Option<f64>,
    pub roll_min: i64,
    pub roll_max: i64,
    pub effect: String,
    /// 編輯時要用來定位勘誤。與母素材同一張工作表，但明講比讓前端推斷好。
    pub source_sheet: String,
    pub source_row: i64,
}

/// 混沌石擲詞綴時「某部位在某個加值等級可以出現哪些詞綴」。
#[derive(Serialize)]
pub struct DistributionGroup {
    pub slot: String,
    pub plus_label: String,
    pub affixes: Vec<String>,
}

pub fn affix_chapter(conn: &Connection, tier: &str) -> Result<AffixChapter, String> {
    let slots = group_strings(
        conn,
        "SELECT s.affix_id, s.slot FROM affix_slot s
           JOIN affix a ON a.id = s.affix_id
          WHERE a.affix_tier = ?1
          ORDER BY s.slot",
        tier,
    )?;

    let mut ranks: HashMap<String, Vec<AffixRank>> = HashMap::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT k.affix_id, k.rank, k.plus_cost, k.condition
                   FROM affix_rank k
                   JOIN affix a ON a.id = k.affix_id
                  WHERE a.affix_tier = ?1
                  ORDER BY k.rank, k.condition",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([tier], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    AffixRank {
                        rank: r.get(1)?,
                        plus_cost: r.get(2)?,
                        condition: r.get(3)?,
                    },
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (id, k) = row.map_err(|e| e.to_string())?;
            ranks.entry(id).or_default().push(k);
        }
    }

    let mut stmt = conn
        .prepare(
            "SELECT id, name, plus_cost_raw, effect, source_sheet, source_row
               FROM affix WHERE affix_tier = ?1 ORDER BY source_row",
        )
        .map_err(|e| e.to_string())?;
    let affixes: Vec<Affix> = stmt
        .query_map([tier], |r| {
            let id: String = r.get(0)?;
            Ok(Affix {
                name: r.get(1)?,
                plus_cost_raw: r.get(2)?,
                effect: r.get(3)?,
                source_sheet: r.get(4)?,
                source_row: r.get(5)?,
                slots: Vec::new(),
                ranks: Vec::new(),
                id,
            })
        })
        .map_err(|e| e.to_string())?
        .map(|row| {
            let mut a = row.map_err(|e| e.to_string())?;
            a.slots = slots.get(&a.id).cloned().unwrap_or_default();
            a.ranks = ranks.remove(&a.id).unwrap_or_default();
            Ok(a)
        })
        .collect::<Result<_, String>>()?;

    let intro = intro_of(conn, "SELECT DISTINCT source_sheet FROM affix WHERE affix_tier = ?1", tier)?;

    Ok(AffixChapter {
        tier: tier.to_string(),
        intro,
        affixes,
    })
}

pub fn material_chapter(conn: &Connection) -> Result<MaterialChapter, String> {
    let slots = group_strings(
        conn,
        "SELECT material_id, slot FROM material_slot WHERE ?1 = ?1 ORDER BY slot",
        "",
    )?;

    let mut affixes: HashMap<String, Vec<MaterialAffix>> = HashMap::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT material_id, id, name, tier_rank, rarity_multiplier,
                        roll_min, roll_max, effect, source_sheet, source_row
                   FROM material_affix ORDER BY tier_rank, source_row",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    MaterialAffix {
                        id: r.get(1)?,
                        name: r.get(2)?,
                        tier_rank: r.get(3)?,
                        rarity_multiplier: r.get(4)?,
                        roll_min: r.get(5)?,
                        roll_max: r.get(6)?,
                        effect: r.get(7)?,
                        source_sheet: r.get(8)?,
                        source_row: r.get(9)?,
                    },
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (id, a) = row.map_err(|e| e.to_string())?;
            affixes.entry(id).or_default().push(a);
        }
    }

    let mut stmt = conn
        .prepare(
            "SELECT id, name, material_tier, cost_multiplier, source_sheet, source_row
               FROM material ORDER BY source_row",
        )
        .map_err(|e| e.to_string())?;
    let materials: Vec<Material> = stmt
        .query_map([], |r| {
            let id: String = r.get(0)?;
            Ok(Material {
                name: r.get(1)?,
                material_tier: r.get(2)?,
                cost_multiplier: r.get(3)?,
                source_sheet: r.get(4)?,
                source_row: r.get(5)?,
                slots: Vec::new(),
                affixes: Vec::new(),
                id,
            })
        })
        .map_err(|e| e.to_string())?
        .map(|row| {
            let mut m = row.map_err(|e| e.to_string())?;
            m.slots = slots.get(&m.id).cloned().unwrap_or_default();
            m.affixes = affixes.remove(&m.id).unwrap_or_default();
            Ok(m)
        })
        .collect::<Result<_, String>>()?;

    let intro = intro_of(conn, "SELECT DISTINCT source_sheet FROM material WHERE ?1 = ?1", "")?;

    Ok(MaterialChapter { intro, materials })
}

pub fn affix_distribution(conn: &Connection) -> Result<Vec<DistributionGroup>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT slot, plus_label, affix_name FROM affix_distribution
              ORDER BY source_col, source_row",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    // 保留原表順序：用 Vec 而非 HashMap，遇到新的 (部位, 加值) 才開一組。
    let mut out: Vec<DistributionGroup> = Vec::new();
    for row in rows {
        let (slot, plus_label, affix_name) = row.map_err(|e| e.to_string())?;
        match out
            .iter_mut()
            .find(|g| g.slot == slot && g.plus_label == plus_label)
        {
            Some(g) => g.affixes.push(affix_name),
            None => out.push(DistributionGroup {
                slot,
                plus_label,
                affixes: vec![affix_name],
            }),
        }
    }
    Ok(out)
}

/// 某張工作表上的所有對照表（魔法物品價格表一張就拼了 12 張小表）。
pub fn ref_sheet(conn: &Connection, sheet: &str) -> Result<Vec<RefTable>, String> {
    ref_tables_of(conn, sheet)
}

fn group_strings(
    conn: &Connection,
    sql: &str,
    arg: &str,
) -> Result<HashMap<String, Vec<String>>, String> {
    super::multi_map(conn, sql, arg)
}

/// 該工作表在表頭之前的前言段落。工作表名由資料推出，不寫死。
fn intro_of(conn: &Connection, sheet_sql: &str, arg: &str) -> Result<Vec<String>, String> {
    let sql = format!(
        "SELECT body FROM rule_text WHERE sheet IN ({sheet_sql}) ORDER BY sort_order"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([arg], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_conn;

    #[test]
    fn 三個階級的詞綴數量與資料庫相符() {
        let c = test_conn();
        assert_eq!(affix_chapter(&c, "general").unwrap().affixes.len(), 118);
        assert_eq!(affix_chapter(&c, "advanced").unwrap().affixes.len(), 37);
        assert_eq!(affix_chapter(&c, "eternal").unwrap().affixes.len(), 20);
    }

    /// 〈幽冥〉的兩筆是「同一階在不同裝備上的兩種價碼」而不是兩個階級，
    /// 兩者的 rank 都是 1，差別只在 condition。顯示時不能當成兩階。
    #[test]
    fn 條件式價碼不是兩個階級() {
        let c = test_conn();
        let ch = affix_chapter(&c, "general").unwrap();
        let ghost = ch.affixes.iter().find(|a| a.name == "幽冥").expect("找不到幽冥");

        assert_eq!(ghost.ranks.len(), 2);
        assert!(ghost.ranks.iter().all(|k| k.rank == 1), "兩筆的階級都該是 1");
        assert!(ghost.ranks.iter().all(|k| !k.condition.is_empty()));
    }

    #[test]
    fn 素材帶得出詞綴與機率區間() {
        let c = test_conn();
        let ch = material_chapter(&c).unwrap();
        assert_eq!(ch.materials.len(), 21);

        let phoenix = ch.materials.iter().find(|m| m.name == "鳳羽").expect("找不到鳳羽");
        assert_eq!(phoenix.affixes.len(), 3);
        assert_eq!(phoenix.affixes[0].roll_min, 1);
        assert_eq!(phoenix.affixes[0].roll_max, 70);
        assert_eq!(phoenix.affixes[0].source_sheet, "素材詞綴", "編輯定位要用到");
        assert!(!phoenix.slots.is_empty(), "素材應帶得出可附的部位");
    }

    #[test]
    fn 詞綴分布依部位與加值分組() {
        let c = test_conn();
        let groups = affix_distribution(&c).unwrap();
        assert!(!groups.is_empty());

        let total: usize = groups.iter().map(|g| g.affixes.len()).sum();
        assert_eq!(total, 685, "分組不該漏掉任何一筆");
    }

    #[test]
    fn 價格表一張工作表拼了十二張小表() {
        let c = test_conn();
        let tables = ref_sheet(&c, "魔法物品價格表").unwrap();
        assert_eq!(tables.len(), 12);
        assert!(tables.iter().all(|t| !t.rows.is_empty()));
        assert!(tables.iter().all(|t| !t.columns.is_empty()));
    }
}
