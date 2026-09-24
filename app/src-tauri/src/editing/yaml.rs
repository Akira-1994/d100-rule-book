//! 把一筆勘誤追加到 `data/errata/<工作表>.yaml`。
//!
//! **從不修改既有條目，只在檔尾追加。** 兩個理由：既有檔案裡有大量手寫
//! 註解與說明（見 `data/errata/高級專長.yaml` 的檔頭），用 YAML 程式庫
//! 重寫整個檔案會把它們全部抹掉；而且追加天然形成歷史，要改回去就再追加
//! 一筆。這在語意上成立，因為 `apply_errata` 是依序套用、後蓋前的。
//!
//! 產生 YAML 不需要 YAML 程式庫：JSON 是 YAML 的子集，值直接用
//! `serde_json` 輸出就是合法的 YAML 純量／流序列。只有 `reason` 為了
//! 可讀性用 `>-` 區塊，與現有檔案的風格一致。

use std::fs;
use std::path::{Path, PathBuf};

use super::{EditRequest, repo_root, target_for};

pub fn errata_dir() -> PathBuf {
    repo_root().join("data").join("errata")
}

/// 追加一筆勘誤，回傳被寫入的檔案路徑。
pub fn append(request: &EditRequest) -> Result<PathBuf, String> {
    let dir = errata_dir();
    if !dir.is_dir() {
        return Err(format!("找不到勘誤資料夾：{}", dir.display()));
    }
    append_to_dir(&dir, request)
}

/// 與 `append` 相同，但可指定資料夾 —— 測試用暫存目錄，絕不碰 repo 裡的
/// 真實資料。
pub fn append_to_dir(dir: &Path, request: &EditRequest) -> Result<PathBuf, String> {
    let path = dir.join(format!("{}.yaml", request.sheet));

    let existing = if path.is_file() {
        fs::read_to_string(&path).map_err(|e| format!("讀不到 {}：{e}", path.display()))?
    } else {
        String::new()
    };

    let mut next = if existing.trim().is_empty() {
        header(&request.sheet)
    } else {
        let mut s = existing.clone();
        if !s.ends_with('\n') {
            s.push('\n');
        }
        s
    };
    next.push_str(&entry_block(request));

    fs::write(&path, &next).map_err(|e| format!("寫不進 {}：{e}", path.display()))?;
    Ok(path)
}

/// 還原檔案。追加後驗證失敗時用 —— 寫壞 errata 會讓整個建置停擺。
pub fn restore(path: &Path, previous: Option<String>) -> Result<(), String> {
    match previous {
        Some(content) => fs::write(path, content)
            .map_err(|e| format!("還原 {} 失敗：{e}", path.display())),
        // 本來就不存在的檔案，還原就是刪掉。
        None => fs::remove_file(path)
            .map_err(|e| format!("移除 {} 失敗：{e}", path.display())),
    }
}

/// 讀出檔案現有內容，供失敗時還原。檔案不存在回 None。
pub fn snapshot(sheet: &str) -> Option<String> {
    fs::read_to_string(errata_dir().join(format!("{sheet}.yaml"))).ok()
}

fn header(sheet: &str) -> String {
    format!(
        "# {sheet} 的勘誤\n\
         #\n\
         # 每一筆都必須寫明 reason。這份檔案是要能直接整理成清單、拿去跟規則書\n\
         # 作者對帳的，所以理由要寫給人看，不是寫給程式看。\n\
         \n\
         sheet: {sheet}\n\
         \n\
         entries:\n"
    )
}

fn entry_block(request: &EditRequest) -> String {
    let mut out = String::from("\n");
    out.push_str(&format!("  - row: {}\n", request.row));
    // 有 source_col 的條目才帶 col。詞綴與素材詞綴在解析結果裡沒有這個
    // 欄位（`apply_errata` 會以預設值 1 比對），寫一個猜出來的值只會讓
    // 勘誤定位失敗。
    if let Some(col) = request.col {
        out.push_str(&format!("    col: {col}\n"));
    }
    if let Some(target) = target_for(&request.entry_kind) {
        out.push_str(&format!("    target: {target}\n"));
    }
    out.push_str(&folded_reason(&request.reason));
    out.push_str("    set:\n");
    for (name, value) in &request.changes {
        out.push_str(&format!("      {name}: {}\n", scalar(value)));
    }
    out
}

/// 理由用 `>-` 折疊區塊，與現有檔案一致。
///
/// 折疊區塊會把換行**併成一個空白**。對英文那是對的，對中文就是在字與字
/// 之間插了一個不該有的空格 —— 實際跑過一次才發現「驗完即還原」被存成
/// 「驗完 即還原」。因此只在本來就有空白的地方折行：那正是折疊插入的
/// 空白無害的位置。中文長句沒有空白可折，就讓它留成一行。
fn folded_reason(reason: &str) -> String {
    let words: Vec<&str> = reason.split_whitespace().collect();
    let mut out = String::from("    reason: >-\n");
    for line in fold_at_spaces(&words, 60) {
        out.push_str(&format!("      {line}\n"));
    }
    out
}

/// 把詞串成行，只在詞與詞之間換行。寬度以顯示寬度計（中文算兩格），
/// 但寬度只是偏好 —— 一個詞再長也不切開。
fn fold_at_spaces(words: &[&str], width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in words {
        let candidate = display_width(&current) + 1 + display_width(word);
        if !current.is_empty() && candidate > width {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn display_width(text: &str) -> usize {
    text.chars()
        .map(|ch| if (ch as u32) > 0x2000 { 2 } else { 1 })
        .sum()
}

/// 值一律以 JSON 輸出。JSON 是 YAML 的子集，字串的 `\n`、引號跳脫、
/// 陣列的 `["操作"]` 都是合法 YAML，不必自己處理跳脫規則。
fn scalar(value: &serde_json::Value) -> String {
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("d100-errata-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn request(sheet: &str, changes: serde_json::Value) -> EditRequest {
        EditRequest {
            entry_kind: "feat".into(),
            sheet: sheet.into(),
            row: 19,
            col: Some(3),
            reason: "本團把難度調低，原值對新手太苛。".into(),
            changes: serde_json::from_value(changes).unwrap(),
        }
    }

    #[test]
    fn 新檔案帶得出檔頭與必要欄位() {
        let dir = temp_dir("new");
        let req = request("高級專長", json!({ "difficulty": 4, "difficulty_raw": "4" }));
        let path = append_to_dir(&dir, &req).unwrap();

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("sheet: 高級專長"));
        assert!(text.contains("entries:"));
        assert!(text.contains("- row: 19"));
        assert!(text.contains("col: 3"), "col 一律要帶");
        assert!(text.contains("reason: >-"));
        assert!(text.contains("difficulty: 4"));
        assert!(text.contains(r#"difficulty_raw: "4""#));
    }

    #[test]
    fn 追加不動既有內容() {
        let dir = temp_dir("append");
        let path = dir.join("高級專長.yaml");
        let original = "# 手寫的註解，不可以被抹掉\nsheet: 高級專長\nentries:\n  - row: 1\n    reason: 舊的\n    set:\n      effect: 舊\n";
        fs::write(&path, original).unwrap();

        append_to_dir(&dir, &request("高級專長", json!({ "difficulty": 4 }))).unwrap();

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with(original), "既有內容必須原封不動留在前面");
        assert!(text.contains("- row: 19"));
    }

    /// 效果敘述常含換行。JSON 字串的 \n 在 YAML 雙引號裡是同一個意思。
    #[test]
    fn 多行字串以跳脫形式寫入() {
        let dir = temp_dir("multiline");
        let req = request("高級專長", json!({ "effect": "第一行\n第二行" }));
        let path = append_to_dir(&dir, &req).unwrap();

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains(r#"effect: "第一行\n第二行""#), "實際內容：{text}");
    }

    #[test]
    fn 清單以流序列寫入() {
        let dir = temp_dir("list");
        let req = request("高級專長", json!({ "categories": ["操作", "知識"] }));
        let path = append_to_dir(&dir, &req).unwrap();

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains(r#"categories: ["操作","知識"]"#), "實際內容：{text}");
    }

    #[test]
    fn 素材詞綴帶得出target() {
        let dir = temp_dir("target");
        let mut req = request("素材詞綴", json!({ "roll_max": 70 }));
        req.entry_kind = "material_affix".into();
        req.col = None;
        let path = append_to_dir(&dir, &req).unwrap();

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("target: material_affix"));
        assert!(!text.contains("col:"), "沒有 source_col 的條目不該寫 col");
    }

    /// YAML 的折疊區塊會把換行併成空白。中文沒有詞間空白，硬折行等於在
    /// 字中間插空格 —— 這是實際跑過一次才發現的。
    #[test]
    fn 中文理由不會被折行插入空格() {
        let dir = temp_dir("fold");
        let long = "這是一段很長的中文理由用來確認折疊區塊不會在字與字之間插入多餘的空白因為那會讓存進資料庫的理由與寫進去的不一樣";
        let mut req = request("高級專長", json!({ "difficulty": 4 }));
        req.reason = long.into();
        let path = append_to_dir(&dir, &req).unwrap();

        let text = fs::read_to_string(&path).unwrap();
        // 中文長句沒有空白可折，整段要留在同一行。
        assert!(text.contains(&format!("      {long}\n")), "實際內容：{text}");
    }

    /// 英文在詞間折行是安全的 —— 折疊插入的空白本來就在那裡。
    #[test]
    fn 英文理由在詞間折行() {
        let dir = temp_dir("fold-en");
        let mut req = request("高級專長", json!({ "difficulty": 4 }));
        req.reason = "the quick brown fox ".repeat(10);
        let path = append_to_dir(&dir, &req).unwrap();

        let text = fs::read_to_string(&path).unwrap();
        let body: Vec<&str> = text
            .lines()
            .skip_while(|l| !l.contains("reason: >-"))
            .skip(1)
            .take_while(|l| l.starts_with("      "))
            .collect();
        assert!(body.len() > 1, "長英文應該折成多行");
        assert!(body.iter().all(|l| !l.trim().is_empty()));
    }

    #[test]
    fn 還原可以救回原本的內容() {
        let dir = temp_dir("restore");
        let path = dir.join("高級專長.yaml");
        let original = "sheet: 高級專長\nentries: []\n";
        fs::write(&path, original).unwrap();

        let before = fs::read_to_string(&path).ok();
        append_to_dir(&dir, &request("高級專長", json!({ "difficulty": 4 }))).unwrap();
        restore(&path, before).unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn 還原本來不存在的檔案等於刪掉它() {
        let dir = temp_dir("restore-new");
        let path = append_to_dir(&dir, &request("新表", json!({ "difficulty": 4 }))).unwrap();
        assert!(path.is_file());

        restore(&path, None).unwrap();
        assert!(!path.exists());
    }
}
