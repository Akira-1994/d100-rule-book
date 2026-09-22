//! 勘誤編輯。
//!
//! **只在開發模式存在。** 編輯必然要寫 repo 裡的 `data/errata/*.yaml`、
//! 必然要重跑 `tools/build_db.py`，這兩件事只有在有 repo、有 Python 的
//! 機器上才成立。打包版沒有這些東西，所以功能根本不編譯進去。
//!
//! 流程一步都不改，只是有了介面：追加一筆勘誤 → 重建 → 重新開啟資料庫。

pub mod history;
pub mod rebuild;
pub mod yaml;

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// repo 根目錄。從 src-tauri 往上兩層，與 `db::dev_path` 同一個假設。
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// 編輯只在開發模式開啟。`tauri dev` 為真，`tauri build` 為假。
pub fn editing_enabled() -> bool {
    cfg!(debug_assertions)
}

/// 編輯功能目前能不能用。
///
/// 刻意不塞進 `build_info` —— 那是資料庫的建置資訊，編輯能不能用跟資料庫
/// 無關，混在一起會讓 db 層知道它不該知道的事。
#[derive(Serialize)]
pub struct EditingStatus {
    pub enabled: bool,
    pub python_ok: bool,
    /// Python 不可用時的說明，讓按鈕在按下去之前就知道自己不能用。
    pub python_hint: Option<String>,
}

pub fn status() -> EditingStatus {
    if !editing_enabled() {
        return EditingStatus {
            enabled: false,
            python_ok: false,
            python_hint: None,
        };
    }
    let (python_ok, python_hint) = rebuild::python_available();
    EditingStatus {
        enabled: true,
        python_ok,
        python_hint,
    }
}

#[derive(Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    /// 單行或多行文字
    Text,
    /// 數值
    Number,
    /// 字串陣列（分類、標記）
    StringList,
}

#[derive(Serialize)]
pub struct EditableField {
    pub name: &'static str,
    pub label: &'static str,
    pub kind: FieldKind,
    /// 有值時前端要做成下拉而不是自由輸入
    pub options: Option<&'static [&'static str]>,
    pub hint: Option<&'static str>,
}

/// 六大技能分類是權威清單（資料庫的 `category` 表），不能自由輸入。
const CATEGORIES: &[&str] = &["戰鬥", "運動", "操作", "感知", "知識", "交涉"];

/// 可編輯的欄位。
///
/// 以現有 errata 已經用過的欄位為準，不自行擴充 —— 能寫進 YAML 不代表
/// `build_db.py` 認得，寫一個它不認得的欄位只會讓建置中止。
const FEAT_FIELDS: &[EditableField] = &[
    EditableField {
        name: "difficulty",
        label: "難度",
        kind: FieldKind::Number,
        options: None,
        hint: Some("CP 成本走 2^等級 × 難度"),
    },
    EditableField {
        name: "difficulty_raw",
        label: "難度原文",
        kind: FieldKind::Text,
        options: None,
        hint: Some("原始字串。改難度時通常要一起改，兩者並存是刻意的"),
    },
    EditableField {
        name: "effect",
        label: "效果",
        kind: FieldKind::Text,
        options: None,
        hint: None,
    },
    EditableField {
        name: "categories",
        label: "分類",
        kind: FieldKind::StringList,
        options: Some(CATEGORIES),
        hint: None,
    },
    EditableField {
        name: "tags",
        label: "標記",
        kind: FieldKind::StringList,
        options: None,
        hint: Some("例如「入職門檻」"),
    },
    EditableField {
        name: "prereq_raw",
        label: "前置原文",
        kind: FieldKind::Text,
        options: None,
        hint: Some("重建時會重新解析成前置鏈"),
    },
    EditableField {
        name: "parent",
        label: "父項名稱",
        kind: FieldKind::Text,
        options: None,
        hint: Some("建置時會解析成 parent_id，名稱必須存在"),
    },
];

const AFFIX_FIELDS: &[EditableField] = &[EditableField {
    name: "effect",
    label: "效果",
    kind: FieldKind::Text,
    options: None,
    hint: None,
}];

const MATERIAL_AFFIX_FIELDS: &[EditableField] = &[
    EditableField {
        name: "roll_min",
        label: "D100 下限",
        kind: FieldKind::Number,
        options: None,
        hint: None,
    },
    EditableField {
        name: "roll_max",
        label: "D100 上限",
        kind: FieldKind::Number,
        options: None,
        hint: None,
    },
    EditableField {
        name: "effect",
        label: "效果",
        kind: FieldKind::Text,
        options: None,
        hint: None,
    },
];

pub fn fields_for(entry_kind: &str) -> &'static [EditableField] {
    match entry_kind {
        "feat" => FEAT_FIELDS,
        "affix" => AFFIX_FIELDS,
        "material_affix" => MATERIAL_AFFIX_FIELDS,
        _ => &[],
    }
}

/// 同一列可能對應多種紀錄（素材列後面跟著它的詞綴），此時 `build_db.py`
/// 要求以 `target:` 指明。
pub fn target_for(entry_kind: &str) -> Option<&'static str> {
    match entry_kind {
        "material_affix" => Some("material_affix"),
        _ => None,
    }
}

#[derive(Deserialize)]
pub struct EditRequest {
    /// feat / affix / material_affix
    pub entry_kind: String,
    pub sheet: String,
    pub row: i64,
    /// Tier C 的職業表並排多個區塊，同一列可能有數個條目。一律帶上，
    /// 不去判斷「這次需不需要」—— 判斷錯會把勘誤套到隔壁欄的條目上。
    pub col: i64,
    pub reason: String,
    /// 只含**有改動**的欄位。沒動的欄位不該出現在勘誤裡，否則清單上會是
    /// 一堆「把 X 改成 X」的雜訊，而那份清單是要拿去跟作者對帳的。
    pub changes: BTreeMap<String, serde_json::Value>,
}

/// 型別檢查而已，不做規則驗證 —— 與 Phase 4 的「軟性警告、可覆寫」一致。
/// 我們擋的是「寫進去會讓建置中止」的東西，不是「這個數值不平衡」。
pub fn validate(request: &EditRequest) -> Result<(), String> {
    if request.reason.trim().is_empty() {
        return Err("必須寫明理由。這些檔案是要拿去跟規則書作者對帳的。".into());
    }
    if request.changes.is_empty() {
        return Err("沒有任何改動。".into());
    }

    let allowed = fields_for(&request.entry_kind);
    if allowed.is_empty() {
        return Err(format!("不支援編輯的條目種類：{}", request.entry_kind));
    }

    for (name, value) in &request.changes {
        let field = allowed
            .iter()
            .find(|f| f.name == name)
            .ok_or_else(|| format!("「{name}」不是可編輯的欄位。"))?;

        match field.kind {
            FieldKind::Number => {
                if !value.is_number() {
                    return Err(format!("{}必須是數字。", field.label));
                }
            }
            FieldKind::Text => {
                if !value.is_string() {
                    return Err(format!("{}必須是文字。", field.label));
                }
            }
            FieldKind::StringList => {
                let items = value
                    .as_array()
                    .ok_or_else(|| format!("{}必須是清單。", field.label))?;
                for item in items {
                    let text = item
                        .as_str()
                        .ok_or_else(|| format!("{}的每一項都必須是文字。", field.label))?;
                    if let Some(options) = field.options {
                        if !options.contains(&text) {
                            return Err(format!(
                                "{}只能是 {} 其中之一，「{text}」不在其中。",
                                field.label,
                                options.join("、")
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(changes: serde_json::Value) -> EditRequest {
        EditRequest {
            entry_kind: "feat".into(),
            sheet: "高級專長".into(),
            row: 19,
            col: 1,
            reason: "測試".into(),
            changes: serde_json::from_value(changes).unwrap(),
        }
    }

    #[test]
    fn 白名單拒絕表外的欄位() {
        let err = validate(&request(json!({ "source_row": 5 }))).unwrap_err();
        assert!(err.contains("不是可編輯的欄位"), "實際訊息：{err}");

        // prereq_raw 在表上 —— 走 Python 管線才有辦法支援的那一個。
        assert!(validate(&request(json!({ "prereq_raw": "跑步二級" }))).is_ok());
    }

    #[test]
    fn 難度必須是數字() {
        let err = validate(&request(json!({ "difficulty": "四" }))).unwrap_err();
        assert!(err.contains("必須是數字"), "實際訊息：{err}");
        assert!(validate(&request(json!({ "difficulty": 4 }))).is_ok());
    }

    #[test]
    fn 分類限定在六大技能分類內() {
        let err = validate(&request(json!({ "categories": ["潛行"] }))).unwrap_err();
        assert!(err.contains("不在其中"), "實際訊息：{err}");
        assert!(validate(&request(json!({ "categories": ["操作", "知識"] }))).is_ok());
    }

    #[test]
    fn 理由與改動都不可為空() {
        let mut r = request(json!({ "difficulty": 4 }));
        r.reason = "   ".into();
        assert!(validate(&r).unwrap_err().contains("理由"));

        assert!(validate(&request(json!({}))).unwrap_err().contains("沒有任何改動"));
    }

    #[test]
    fn 素材詞綴需要指明target() {
        assert_eq!(target_for("material_affix"), Some("material_affix"));
        assert_eq!(target_for("feat"), None);
    }
}
