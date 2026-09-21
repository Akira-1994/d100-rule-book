//! 小型對照表的通用渲染資料。
//!
//! 規則書裡散落著 31 張查表，欄位各不相同，硬要各建一張資料表並不划算，
//! 因此資料庫用 `ref_table` / `ref_row` 通用結構收，欄名與儲存格各存成
//! JSON 陣列。這裡負責把那兩個 JSON 字串還原成陣列交給前端照著渲染。

use rusqlite::Connection;
use serde::Serialize;

#[derive(Serialize)]
pub struct RefTable {
    pub id: String,
    pub sheet: String,
    pub name: String,
    pub note: Option<String>,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub source_row: i64,
}

#[derive(Serialize)]
pub struct RefTableSummary {
    pub id: String,
    pub sheet: String,
    pub name: String,
    pub row_count: i64,
}

pub fn ref_tables_of(conn: &Connection, sheet: &str) -> Result<Vec<RefTable>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, sheet, name, note, columns_json, source_row
               FROM ref_table WHERE sheet = ?1 ORDER BY sort_order, source_row, source_col",
        )
        .map_err(|e| e.to_string())?;
    let heads: Vec<(String, String, String, Option<String>, String, i64)> = stmt
        .query_map([sheet], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for (id, sheet, name, note, columns_json, source_row) in heads {
        let mut stmt = conn
            .prepare(
                "SELECT cells_json FROM ref_row WHERE table_id = ?1 ORDER BY row_index",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<Vec<String>> = stmt
            .query_map([&id], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .map(|cells| parse_cells(&cells.map_err(|e| e.to_string())?))
            .collect::<Result<_, String>>()?;

        out.push(RefTable {
            columns: parse_cells(&columns_json)?,
            rows,
            id,
            sheet,
            name,
            note,
            source_row,
        });
    }
    Ok(out)
}

/// 全書 31 張對照表的索引，供附錄章節做總覽。
pub fn ref_index(conn: &Connection) -> Result<Vec<RefTableSummary>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.sheet, t.name,
                    (SELECT count(*) FROM ref_row r WHERE r.table_id = t.id)
               FROM ref_table t ORDER BY t.sheet, t.sort_order, t.source_row",
        )
        .map_err(|e| e.to_string())?;
    let list = stmt
        .query_map([], |r| {
            Ok(RefTableSummary {
                id: r.get(0)?,
                sheet: r.get(1)?,
                name: r.get(2)?,
                row_count: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    Ok(list)
}

/// 儲存格是 JSON 陣列。解析失敗時回報錯誤而不是靜靜地給一張空表 ——
/// 那會讓「資料壞了」看起來像「這張表本來就沒有內容」。
fn parse_cells(json: &str) -> Result<Vec<String>, String> {
    serde_json::from_str::<Vec<Option<String>>>(json)
        .map(|cells| cells.into_iter().map(|c| c.unwrap_or_default()).collect())
        .map_err(|e| format!("對照表的儲存格不是合法的 JSON 陣列：{e}（{json}）"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_conn;

    #[test]
    fn 對照表索引涵蓋三十一張() {
        let c = test_conn();
        let index = ref_index(&c).unwrap();
        assert_eq!(index.len(), 31);
        assert!(index.iter().all(|t| t.row_count > 0), "不該有空表");
    }

    #[test]
    fn 欄名與儲存格都還原成陣列() {
        let c = test_conn();
        let tables = ref_tables_of(&c, "創角色須知").unwrap();
        let table = &tables[0];

        assert_eq!(table.columns, vec!["等級", "CP總數(施法或者近戰)"]);
        assert_eq!(table.rows[0], vec!["1", "2"]);
        // 每一列的欄數應與欄名一致，否則前端渲染會錯位。
        assert!(
            table.rows.iter().all(|r| r.len() == table.columns.len()),
            "有列的欄數與表頭不符"
        );
    }
}
