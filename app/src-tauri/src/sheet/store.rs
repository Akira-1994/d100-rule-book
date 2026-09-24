//! 角色卡的讀寫。
//!
//! 一張卡一個 JSON，放使用者資料夾的 `characters/`。**JSON 就是模型本身**，
//! 所以匯出匯入幾乎是附贈的：匯出是複製檔案，匯入是讀進來、換一個新 id、
//! 存起來。

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::{SCHEMA_VERSION, Sheet};

#[derive(Serialize)]
pub struct SheetSummary {
    pub id: String,
    pub name: String,
    /// 讀不起來的卡也要列出來並說明原因，不要靜靜地消失 —— 那是使用者
    /// 自己的資料，消失比報錯嚇人得多。
    pub error: Option<String>,
}

/// 角色卡資料夾。呼叫端從 Tauri 拿到使用者資料夾後傳進來，
/// 這個模組本身不依賴 Tauri，測試才跑得動。
pub fn characters_dir(base: &Path) -> PathBuf {
    base.join("characters")
}

pub fn list(base: &Path) -> Result<Vec<SheetSummary>, String> {
    let dir = characters_dir(base);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| format!("讀不到 {}：{e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        match read_file(&path) {
            Ok(sheet) => out.push(SheetSummary {
                id: sheet.id,
                name: sheet.name,
                error: None,
            }),
            Err(e) => out.push(SheetSummary {
                id,
                name: "（讀取失敗）".into(),
                error: Some(e),
            }),
        }
    }
    // 依名稱排序。注意中文是**依 Unicode 碼位**比較，不是筆劃或注音 ——
    // 「乙」(U+4E59) 會排在「甲」(U+7532) 前面。對玩家而言這個順序看起來
    // 是任意的，但它穩定且可預期；要做到字典序得引進整套定序表，
    // 對一份十來張卡的清單不划算。
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn load(base: &Path, id: &str) -> Result<Sheet, String> {
    read_file(&path_for(base, id))
}

pub fn save(base: &Path, sheet: &Sheet) -> Result<(), String> {
    let dir = characters_dir(base);
    fs::create_dir_all(&dir).map_err(|e| format!("建不出 {}：{e}", dir.display()))?;
    let path = path_for(base, &sheet.id);
    let json = serde_json::to_string_pretty(sheet).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| format!("寫不進 {}：{e}", path.display()))
}

/// 真的刪掉檔案。使用者自己產生的資料刪掉沒得復原，呼叫端負責確認。
pub fn delete(base: &Path, id: &str) -> Result<(), String> {
    let path = path_for(base, id);
    fs::remove_file(&path).map_err(|e| format!("刪不掉 {}：{e}", path.display()))
}

/// 匯入：讀進來、換一個新 id、存起來。
///
/// 換 id 是刻意的 —— 匯入同一個檔案兩次應該得到兩張卡，而不是第二次
/// 悄悄蓋掉第一次。
pub fn import(base: &Path, source: &Path, new_id: String) -> Result<Sheet, String> {
    let mut sheet = read_file(source)?;
    sheet.id = new_id;
    save(base, &sheet)?;
    Ok(sheet)
}

pub fn export(base: &Path, id: &str, destination: &Path) -> Result<(), String> {
    let sheet = load(base, id)?;
    let json = serde_json::to_string_pretty(&sheet).map_err(|e| e.to_string())?;
    fs::write(destination, json)
        .map_err(|e| format!("寫不進 {}：{e}", destination.display()))
}

fn path_for(base: &Path, id: &str) -> PathBuf {
    characters_dir(base).join(format!("{id}.json"))
}

fn read_file(path: &Path) -> Result<Sheet, String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("讀不到 {}：{e}", path.display()))?;
    let sheet: Sheet = serde_json::from_str(&text)
        .map_err(|e| format!("{} 不是合法的角色卡：{e}", path.display()))?;

    // 讀到不認得的版本時明白拒絕，不要猜。硬讀一張未來版本的卡，最好的
    // 情況是缺欄位，最壞的情況是欄位意義變了而數字看起來仍然合理。
    if sheet.schema_version > SCHEMA_VERSION {
        return Err(format!(
            "這張角色卡的格式版本是 {}，本版應用只認得到 {}。請更新應用。",
            sheet.schema_version, SCHEMA_VERSION
        ));
    }
    Ok(sheet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheet::{Adjustment, SheetFeat, Track};

    fn temp_base(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("d100-sheet-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample() -> Sheet {
        let mut s = Sheet::new("abc123".into(), "測試角色".into());
        s.attributes = [("CON", 13), ("RES", 19)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        s.feats.push(SheetFeat {
            feat_id: "feat:武器使用".into(),
            level: 3,
            track: Track::Melee,
        });
        s.adjustments.insert(
            "感知".into(),
            Adjustment { delta: 5, note: "靈巧之靴".into() },
        );
        s
    }

    #[test]
    fn 存了再讀回來內容一致() {
        let base = temp_base("roundtrip");
        let sheet = sample();
        save(&base, &sheet).unwrap();

        let back = load(&base, "abc123").unwrap();
        assert_eq!(back.name, "測試角色");
        assert_eq!(back.feats.len(), 1);
        assert_eq!(back.feats[0].track, Track::Melee);
        assert_eq!(back.adjustments["感知"].delta, 5);
        assert_eq!(back.adjustments["感知"].note, "靈巧之靴");
    }

    /// 排序是依 Unicode 碼位而非字典序 —— 這裡用碼位差距明確的兩個字
    /// 把這件事釘住，免得日後有人「修正」成看起來對但其實不穩定的實作。
    #[test]
    fn 清單依名稱排序且空資料夾不是錯誤() {
        let base = temp_base("list");
        assert!(list(&base).unwrap().is_empty(), "還沒有卡不該是錯誤");

        let mut a = sample();
        a.id = "a".into();
        a.name = "乙".into(); // U+4E59
        save(&base, &a).unwrap();

        let mut b = sample();
        b.id = "b".into();
        b.name = "甲".into(); // U+7532
        save(&base, &b).unwrap();

        let names: Vec<String> = list(&base).unwrap().into_iter().map(|s| s.name).collect();
        assert_eq!(names, vec!["乙", "甲"], "依碼位，乙在甲之前");
    }

    /// 讀不起來的卡要列出來並說明，不要靜靜地消失 —— 那是使用者自己的資料。
    #[test]
    fn 壞掉的卡仍然列得出來並帶錯誤說明() {
        let base = temp_base("broken");
        fs::create_dir_all(characters_dir(&base)).unwrap();
        fs::write(characters_dir(&base).join("broken.json"), "{ 不是 JSON").unwrap();

        let rows = list(&base).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "broken");
        assert!(rows[0].error.is_some());
    }

    #[test]
    fn 未來版本的卡明白拒絕() {
        let base = temp_base("version");
        let mut sheet = sample();
        sheet.schema_version = SCHEMA_VERSION + 1;
        save(&base, &sheet).unwrap();

        let err = load(&base, "abc123").unwrap_err();
        assert!(err.contains("格式版本"), "實際訊息：{err}");
        assert!(err.contains("請更新應用"));
    }

    /// 匯入同一個檔案兩次應該得到兩張卡，而不是第二次悄悄蓋掉第一次。
    #[test]
    fn 匯入換新id() {
        let base = temp_base("import");
        let source = base.join("外來的卡.json");
        fs::write(&source, serde_json::to_string(&sample()).unwrap()).unwrap();

        let first = import(&base, &source, "new-1".into()).unwrap();
        let second = import(&base, &source, "new-2".into()).unwrap();

        assert_eq!(first.name, second.name);
        assert_ne!(first.id, second.id);
        assert_eq!(list(&base).unwrap().len(), 2);
    }

    #[test]
    fn 匯出的檔案讀得回來() {
        let base = temp_base("export");
        save(&base, &sample()).unwrap();

        let out = base.join("匯出.json");
        export(&base, "abc123", &out).unwrap();

        let text = fs::read_to_string(&out).unwrap();
        let back: Sheet = serde_json::from_str(&text).unwrap();
        assert_eq!(back.name, "測試角色");
    }

    #[test]
    fn 刪除之後就不在清單裡() {
        let base = temp_base("delete");
        save(&base, &sample()).unwrap();
        assert_eq!(list(&base).unwrap().len(), 1);

        delete(&base, "abc123").unwrap();
        assert!(list(&base).unwrap().is_empty());
    }
}
