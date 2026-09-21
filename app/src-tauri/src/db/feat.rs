//! 專長的資料契約與詳情查詢。

use rusqlite::Connection;
use serde::Serialize;

use super::categories_of;

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
pub(crate) fn cp_table(difficulty: Option<f64>) -> Vec<CpStep> {
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
    use crate::db::search::search_feats;
    use crate::db::test_conn;

    #[test]
    fn 前置鏈雙向都查得到() {
        let c = test_conn();
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
}
