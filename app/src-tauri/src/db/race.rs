//! 種族章節。

use std::collections::HashMap;

use rusqlite::Connection;
use serde::Serialize;

#[derive(Serialize)]
pub struct RaceChapter {
    pub races: Vec<Race>,
}

#[derive(Serialize)]
pub struct Race {
    pub id: String,
    pub name: String,
    /// 數值化的 CP 調整。非數值時為 None，原文保留在 `cp_raw`。
    pub cp_cost: Option<i64>,
    pub cp_raw: String,
    /// 解析得出的屬性增減。原文另見 `attr_text`，兩者並存。
    pub modifiers: Vec<AttrModifier>,
    pub attr_text: Option<String>,
    pub racial_feat_text: Option<String>,
    pub skill_mod_text: Option<String>,
    pub special_text: Option<String>,
    pub source_sheet: String,
    pub source_row: i64,
}

#[derive(Serialize)]
pub struct AttrModifier {
    pub attr: String,
    pub delta: i64,
}

pub fn race_chapter(conn: &Connection) -> Result<RaceChapter, String> {
    let mut mods: HashMap<String, Vec<AttrModifier>> = HashMap::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT m.race_id, m.attr, m.delta
                   FROM race_attr_modifier m
                   JOIN attribute a ON a.code = m.attr
                  ORDER BY a.sort_order",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    AttrModifier {
                        attr: r.get(1)?,
                        delta: r.get(2)?,
                    },
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (race_id, m) = row.map_err(|e| e.to_string())?;
            mods.entry(race_id).or_default().push(m);
        }
    }

    let mut stmt = conn
        .prepare(
            "SELECT id, name, cp_cost, cp_raw, attr_text, racial_feat_text,
                    skill_mod_text, special_text, source_sheet, source_row
               FROM race ORDER BY source_row",
        )
        .map_err(|e| e.to_string())?;
    let races: Vec<Race> = stmt
        .query_map([], |r| {
            let id: String = r.get(0)?;
            Ok(Race {
                name: r.get(1)?,
                cp_cost: r.get(2)?,
                cp_raw: r.get(3)?,
                attr_text: r.get(4)?,
                racial_feat_text: r.get(5)?,
                skill_mod_text: r.get(6)?,
                special_text: r.get(7)?,
                source_sheet: r.get(8)?,
                source_row: r.get(9)?,
                modifiers: Vec::new(),
                id,
            })
        })
        .map_err(|e| e.to_string())?
        .map(|row| {
            let mut race = row.map_err(|e| e.to_string())?;
            race.modifiers = mods.remove(&race.id).unwrap_or_default();
            Ok(race)
        })
        .collect::<Result<_, String>>()?;

    Ok(RaceChapter { races })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_conn;

    #[test]
    fn 十三個種族都在且屬性調整解析得出() {
        let c = test_conn();
        let ch = race_chapter(&c).unwrap();
        assert_eq!(ch.races.len(), 13);

        let elf = ch.races.iter().find(|r| r.name == "精靈").expect("找不到精靈");
        assert_eq!(elf.cp_cost, Some(-30));
        assert!(
            elf.modifiers.iter().any(|m| m.attr == "DEX" && m.delta == 2),
            "精靈應有 DEX+2"
        );
    }

    /// 〈原初-星之幼體〉的 CP 是「劇情取得」而非數字，依 2026-09-20 的裁示
    /// 由 DM 建卡時手填。不能顯示成空白或 0，原文必須留著。
    #[test]
    fn 非數值的cp保留原文() {
        let c = test_conn();
        let ch = race_chapter(&c).unwrap();

        let star = ch
            .races
            .iter()
            .find(|r| r.name.contains("星之幼體"))
            .expect("找不到原初-星之幼體");
        assert!(star.cp_cost.is_none(), "非數值的 CP 不該硬轉成數字");
        assert_eq!(star.cp_raw, "劇情取得");
    }
}
