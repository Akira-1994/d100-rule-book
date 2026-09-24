//! `derive` 需要的外部資料，從資料庫查出來。
//!
//! 這是計算的**不純的那一半**，刻意與 `derive` 分開：種族的 CP 調整與
//! 專長的難度都在資料庫裡，但那些是輸入而非計算。分開之後 `derive`
//! 才是純函式，測試不必準備一個資料庫。

use rusqlite::Connection;

use super::{FeatCost, Sheet, SheetContext};

pub fn load(conn: &Connection, sheet: &Sheet) -> Result<SheetContext, String> {
    let mut context = SheetContext::default();

    if let Some(race_id) = &sheet.race_id {
        let row = conn.query_row(
            "SELECT cp_cost, cp_raw FROM race WHERE id = ?1",
            [race_id],
            |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, String>(1)?)),
        );
        match row {
            Ok((cp_cost, cp_raw)) => {
                context.race_cp = cp_cost;
                context.race_cp_raw = Some(cp_raw);
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // 種族被刪掉或 id 打錯時不要靜靜地當成沒有種族 —— 那會讓
                // CP 少扣一截而看起來完全正常。
                context.race_cp_raw = Some(format!("找不到種族 {race_id}"));
            }
            Err(e) => return Err(e.to_string()),
        }
    }

    let mut stmt = conn
        .prepare(
            "SELECT name, difficulty, difficulty_raw, difficulty_scale
               FROM feat WHERE id = ?1",
        )
        .map_err(|e| e.to_string())?;

    for entry in &sheet.feats {
        let row = stmt.query_row([&entry.feat_id], |r| {
            Ok(FeatCost {
                name: r.get(0)?,
                difficulty: r.get(1)?,
                difficulty_raw: r.get(2)?,
                scale: r.get(3)?,
            })
        });
        match row {
            Ok(cost) => {
                context.feat_costs.insert(entry.feat_id.clone(), cost);
            }
            // 找不到就不放進 map，`derive` 會為此產生警告。
            Err(rusqlite::Error::QueryReturnedNoRows) => {}
            Err(e) => return Err(e.to_string()),
        }
    }

    Ok(context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_conn;
    use crate::sheet::{SheetFeat, Track};

    #[test]
    fn 查得出種族與專長的計價資料() {
        let conn = test_conn();
        let mut sheet = Sheet::new("t".into(), "測試".into());
        sheet.race_id = Some("race:精靈".into());
        sheet.feats.push(SheetFeat {
            feat_id: "feat:basic:武器使用".into(),
            level: 1,
            track: Track::Melee,
        });

        let ctx = load(&conn, &sheet).unwrap();
        assert_eq!(ctx.race_cp, Some(-30), "精靈的 CP 調整");
    }

    /// 種族 id 打錯時不要靜靜地當成沒有種族 —— 那會讓 CP 少扣一截而
    /// 看起來完全正常。
    #[test]
    fn 找不到的種族會留下說明() {
        let conn = test_conn();
        let mut sheet = Sheet::new("t".into(), "測試".into());
        sheet.race_id = Some("race:不存在".into());

        let ctx = load(&conn, &sheet).unwrap();
        assert_eq!(ctx.race_cp, None);
        assert!(
            ctx.race_cp_raw.unwrap().contains("找不到種族"),
            "應留下說明"
        );
    }

    #[test]
    fn 找不到的專長不進map() {
        let conn = test_conn();
        let mut sheet = Sheet::new("t".into(), "測試".into());
        sheet.feats.push(SheetFeat {
            feat_id: "feat:不存在".into(),
            level: 1,
            track: Track::None,
        });

        let ctx = load(&conn, &sheet).unwrap();
        assert!(ctx.feat_costs.is_empty());
    }
}
