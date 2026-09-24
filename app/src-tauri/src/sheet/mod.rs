//! 角色卡的模型與衍生值計算。
//!
//! 這是本專案第一次有「使用者資料」—— 在此之前所有狀態都是唯讀的建置
//! 產物（`dist/d100.db`）或 git 裡的文字檔（`data/errata`）。角色卡兩者
//! 都不是：它屬於玩家、在玩家的機器上、不進 repo。
//!
//! **能算的一律不存。** 衍生值每次重算，存了就會跟公式不同步 —— 這跟
//! 「資料庫是建置產物、刪掉可以重建」是同一條原則。

pub mod context;
pub mod store;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::rules::{
    self, KNOWLEDGE, RESIST_FORMULAS, SKILL_FORMULAS, SPECIAL_FORMULAS, Scores,
};

/// 模型版本。
///
/// 模型日後一定會改（施法者分支要加環數與法術位，跑團模式要加狀態），
/// 沒有這個欄位就沒辦法分辨「舊卡」與「壞掉的卡」。讀到不認得的版本時
/// 明白拒絕，不要猜。
pub const SCHEMA_VERSION: u32 = 1;

/// 一條專長屬於哪一軌。
///
/// 規則書沒有定義哪些專長算「進戰相關」、哪些算「法術相關」，而基礎
/// HP/SP 直接依賴這兩軌各投資了多少 CP。專長表的分類欄（六大技能判定）
/// 與兩軌對不起來，程式推不出來，因此由玩家逐條標記。
/// 見 `docs/規則裁示紀錄.md` 第 12 項。
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum Track {
    Melee,
    Spell,
    #[default]
    None,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SheetFeat {
    pub feat_id: String,
    pub level: i64,
    #[serde(default)]
    pub track: Track,
}

/// 獎勵 HP/SP 的**擲骰結果**。
///
/// 規則書的獎勵是骰子（達 10CP 送 `1d8HP＋1d4SP`⋯）。骰數階級算得出來，
/// 骰出來的點數算不出來 —— 程式的職責是告訴玩家該擲幾顆，結果由玩家填。
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct BonusRolls {
    pub melee_hp: i64,
    pub melee_sp: i64,
    pub spell_hp: i64,
    pub spell_sp: i64,
}

/// 衍生值的修正。
///
/// 實際跑團一定會有公式沒涵蓋的加值（裝備、buff、DM 裁定）。作法不是把
/// 衍生值覆寫成任意數字，而是把差額與理由並存 —— 直接覆寫會遺失「本來
/// 應該是多少」，屬性改了之後也不知道那個覆寫還算不算數。
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Adjustment {
    pub delta: i64,
    #[serde(default)]
    pub note: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Sheet {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub starting_cp: i64,
    pub attributes: Scores,
    #[serde(default)]
    pub race_id: Option<String>,
    #[serde(default)]
    pub feats: Vec<SheetFeat>,
    #[serde(default)]
    pub bonus: BonusRolls,
    /// 專長與詞綴提供的 HP/SP。規則書寫「專長表之技能＋魔法物品之詞綴」，
    /// 那是效果敘述裡的自由文字，程式讀不出來。
    #[serde(default)]
    pub manual_hp: i64,
    #[serde(default)]
    pub manual_sp: i64,
    /// 以 CP 加購後的目標值。0 代表沒有加購。
    #[serde(default)]
    pub extra_hp: i64,
    #[serde(default)]
    pub extra_sp: i64,
    /// 鍵是衍生值的名稱（「感知」「抗魔法」⋯）。
    #[serde(default)]
    pub adjustments: BTreeMap<String, Adjustment>,
    #[serde(default)]
    pub notes: String,
}

/// 新卡的 id。
///
/// 用時間戳而不是 uuid：不想為了一個「夠不會撞」的字串多一個相依，而卡片
/// 是人手動建立的，毫秒級的時間戳綽綽有餘。
pub fn new_id() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("sheet-{now:x}")
}

impl Sheet {
    pub fn new(id: String, name: String) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            id,
            name,
            starting_cp: 200,
            attributes: BTreeMap::new(),
            race_id: None,
            feats: Vec::new(),
            bonus: BonusRolls::default(),
            manual_hp: 0,
            manual_sp: 0,
            extra_hp: 0,
            extra_sp: 0,
            adjustments: BTreeMap::new(),
            notes: String::new(),
        }
    }
}

// 計算所需的外部資料 ------------------------------------------------------

/// `derive` 需要但不該自己去查的東西。
///
/// 種族的 CP 調整與專長的難度都在資料庫裡，但那些是**輸入**而非計算。
/// 由呼叫端查好傳進來，`derive` 本身才是純函式 —— 否則測不動。
#[derive(Deserialize, Default)]
pub struct SheetContext {
    /// 種族的 CP 調整（負值代表扣除）。非數值（如「劇情取得」）時為 None。
    pub race_cp: Option<i64>,
    /// 種族的 CP 原文，用來在非數值時說明原因。
    #[serde(default)]
    pub race_cp_raw: Option<String>,
    pub feat_costs: BTreeMap<String, FeatCost>,
}

#[derive(Deserialize, Clone)]
pub struct FeatCost {
    pub name: String,
    pub difficulty: Option<f64>,
    #[serde(default)]
    pub difficulty_raw: Option<String>,
    /// `cp` 或 `legend`
    pub scale: String,
}

// 衍生值 ------------------------------------------------------------------

#[derive(Serialize)]
pub struct DerivedValue {
    pub label: String,
    /// 公式算出來的值
    pub formula: i64,
    /// 修正欄
    pub adjustment: i64,
    pub note: String,
    /// 顯示值 = 公式值 + 修正
    pub total: i64,
}

#[derive(Serialize)]
pub struct CpSummary {
    pub starting: i64,
    /// 屬性調整值總和造成的增減（正數代表加 CP）
    pub attribute_adjustment: i64,
    /// 種族的 CP 調整（負值代表扣除）
    pub race: i64,
    /// 專長花費（一般計價）
    pub feats: i64,
    /// 傳奇技能點花費換算成的 CP
    pub legend_as_cp: i64,
    pub extra_hp: i64,
    pub extra_sp: i64,
    pub available: i64,
    pub spent: i64,
    pub remaining: i64,
    /// 兩軌各自的投資，決定獎勵 HP/SP 的骰數階級
    pub melee_invested: i64,
    pub spell_invested: i64,
}

#[derive(Serialize)]
pub struct PoolSummary {
    /// 主屬性含調整值
    pub attribute_part: i64,
    /// 擲出來的獎勵點數
    pub rolled: i64,
    /// 專長與詞綴提供的（手填）
    pub manual: i64,
    pub base: i64,
    /// 以 CP 加購後的目標值。0 代表沒有加購。
    pub extra_target: i64,
    pub extra_cost: i64,
    pub total: i64,
}

#[derive(Serialize)]
pub struct Derived {
    pub modifiers: BTreeMap<String, i64>,
    pub skills: Vec<DerivedValue>,
    pub resists: Vec<DerivedValue>,
    pub specials: Vec<DerivedValue>,
    pub cp: CpSummary,
    pub hp: PoolSummary,
    pub sp: PoolSummary,
    /// 兩軌應該擲的骰數階級。程式算得出階級，算不出點數。
    pub melee_dice_steps: i64,
    pub spell_dice_steps: i64,
    /// 軟性警告。一律只提示、不擋存檔 —— 與 PLAN.md 的「違規可覆寫並
    /// 標記」一致。
    pub warnings: Vec<String>,
}

/// 算出一張卡的所有衍生值。
///
/// **純計算，不碰檔案也不碰資料庫。** 表單每改一個字就會呼叫一次。
pub fn derive(sheet: &Sheet, context: &SheetContext) -> Derived {
    let mut warnings = Vec::new();
    let scores = &sheet.attributes;

    let missing: Vec<&str> = ["STR", "DEX", "SKI", "CON", "RES", "INT", "WIS", "CHA", "SPI"]
        .into_iter()
        .filter(|a| !scores.contains_key(*a))
        .collect();
    if !missing.is_empty() {
        warnings.push(format!("尚未填寫的屬性：{}", missing.join("、")));
    }

    let modifiers: BTreeMap<String, i64> = scores
        .iter()
        .map(|(code, score)| (code.clone(), rules::attribute_modifier(*score)))
        .collect();

    let value_of = |label: &str, formula: Result<i64, String>| -> DerivedValue {
        let adjustment = sheet.adjustments.get(label);
        let formula = formula.unwrap_or(0);
        let delta = adjustment.map(|a| a.delta).unwrap_or(0);
        DerivedValue {
            label: label.to_string(),
            formula,
            adjustment: delta,
            note: adjustment.map(|a| a.note.clone()).unwrap_or_default(),
            total: formula + delta,
        }
    };

    let mut skills: Vec<DerivedValue> = SKILL_FORMULAS
        .iter()
        .map(|(label, _)| value_of(label, rules::skill_value(scores, label)))
        .collect();
    // 知識不在 SKILL_FORMULAS 裡（係數不同），但顯示時屬於六大技能。
    skills.push(value_of(KNOWLEDGE, rules::skill_value(scores, KNOWLEDGE)));

    let resists: Vec<DerivedValue> = RESIST_FORMULAS
        .iter()
        .map(|(label, _)| value_of(label, rules::resist_value(scores, label)))
        .collect();

    let specials: Vec<DerivedValue> = SPECIAL_FORMULAS
        .iter()
        .map(|(label, _)| value_of(label, rules::special_value(scores, label)))
        .collect();

    // CP 收支 ------------------------------------------------------------

    let mut feat_cp = 0i64;
    let mut legend_points = 0i64;
    let mut melee_invested = 0i64;
    let mut spell_invested = 0i64;

    for entry in &sheet.feats {
        let Some(cost) = context.feat_costs.get(&entry.feat_id) else {
            warnings.push(format!("找不到專長 {} 的難度資料。", entry.feat_id));
            continue;
        };
        let Some(difficulty) = cost.difficulty else {
            // 〈知識〉〈語言〉的難度是 `1or2`，依 2026-09-20 的裁示由 DM
            // 個案裁定。不替它挑一個數字，只提示。
            warnings.push(format!(
                "〈{}〉的難度是「{}」，需 DM 裁定，未計入 CP。",
                cost.name,
                cost.difficulty_raw.clone().unwrap_or_else(|| "未定".into())
            ));
            continue;
        };

        let spent = rules::cumulative_cost(entry.level, difficulty).round() as i64;
        let as_cp = if cost.scale == "legend" {
            legend_points += spent;
            spent * rules::LEGEND_POINT_IN_CP as i64
        } else {
            feat_cp += spent;
            spent
        };

        match entry.track {
            Track::Melee => melee_invested += as_cp,
            Track::Spell => spell_invested += as_cp,
            Track::None => {}
        }
    }

    let legend_as_cp = legend_points * rules::LEGEND_POINT_IN_CP as i64;
    let attribute_adjustment = rules::attribute_cp_adjustment(scores);

    let race_cp = context.race_cp.unwrap_or(0);
    if context.race_cp.is_none() {
        if let Some(raw) = &context.race_cp_raw {
            warnings.push(format!(
                "種族的 CP 調整是「{raw}」而非數字，依 2026-09-20 的裁示由 DM 建卡時裁定，未計入。"
            ));
        }
    }

    // HP/SP --------------------------------------------------------------

    let melee_dice_steps = rules::bonus_dice_steps(melee_invested);
    let spell_dice_steps = rules::bonus_dice_steps(spell_invested);

    let hp_rolled = sheet.bonus.melee_hp + sheet.bonus.spell_hp;
    let sp_rolled = sheet.bonus.melee_sp + sheet.bonus.spell_sp;

    let hp_attr = rules::effective(scores, "CON").unwrap_or(0);
    let sp_attr = rules::effective(scores, "RES").unwrap_or(0);
    let hp_base = hp_attr + hp_rolled + sheet.manual_hp;
    let sp_base = sp_attr + sp_rolled + sheet.manual_sp;

    let hp_extra_cost = rules::extra_hp_cp_cost(hp_base, sheet.extra_hp);
    let sp_extra_cost = rules::extra_hp_cp_cost(sp_base, sheet.extra_sp);

    if (melee_dice_steps > 0 || spell_dice_steps > 0) && hp_rolled == 0 && sp_rolled == 0 {
        warnings.push(format!(
            "兩軌的投資已達獎勵門檻（進戰 {melee_dice_steps} 階、法術 {spell_dice_steps} 階），\
             但尚未填入擲骰結果。"
        ));
    }

    let spent = -race_cp + feat_cp + legend_as_cp + hp_extra_cost + sp_extra_cost;
    let available = sheet.starting_cp + attribute_adjustment;
    let remaining = available - spent;
    if remaining < 0 {
        warnings.push(format!("CP 超支 {} 點。", -remaining));
    }

    Derived {
        modifiers,
        skills,
        resists,
        specials,
        cp: CpSummary {
            starting: sheet.starting_cp,
            attribute_adjustment,
            race: race_cp,
            feats: feat_cp,
            legend_as_cp,
            extra_hp: hp_extra_cost,
            extra_sp: sp_extra_cost,
            available,
            spent,
            remaining,
            melee_invested,
            spell_invested,
        },
        hp: PoolSummary {
            attribute_part: hp_attr,
            rolled: hp_rolled,
            manual: sheet.manual_hp,
            base: hp_base,
            extra_target: sheet.extra_hp,
            extra_cost: hp_extra_cost,
            total: sheet.extra_hp.max(hp_base),
        },
        sp: PoolSummary {
            attribute_part: sp_attr,
            rolled: sp_rolled,
            manual: sheet.manual_sp,
            base: sp_base,
            extra_target: sheet.extra_sp,
            extra_cost: sp_extra_cost,
            total: sheet.extra_sp.max(sp_base),
        },
        melee_dice_steps,
        spell_dice_steps,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scores() -> Scores {
        [
            ("STR", 9), ("DEX", 16), ("SKI", 13), ("CON", 13), ("RES", 19),
            ("INT", 19), ("WIS", 17), ("CHA", 13), ("SPI", 13),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect()
    }

    fn sheet() -> Sheet {
        let mut s = Sheet::new("test".into(), "測試角色".into());
        s.attributes = scores();
        s
    }

    fn context() -> SheetContext {
        SheetContext {
            race_cp: Some(-30),
            race_cp_raw: None,
            feat_costs: [
                (
                    "feat:武器使用".to_string(),
                    FeatCost {
                        name: "武器使用".into(),
                        difficulty: Some(1.0),
                        difficulty_raw: None,
                        scale: "cp".into(),
                    },
                ),
                (
                    "feat:傳奇".to_string(),
                    FeatCost {
                        name: "某傳奇專長".into(),
                        difficulty: Some(1.0),
                        difficulty_raw: None,
                        scale: "legend".into(),
                    },
                ),
            ]
            .into_iter()
            .collect(),
        }
    }

    #[test]
    fn 十四項衍生值都算得出來且套用修正() {
        let mut s = sheet();
        s.adjustments.insert(
            "感知".into(),
            Adjustment { delta: 5, note: "靈巧之靴".into() },
        );

        let d = derive(&s, &context());
        assert_eq!(d.skills.len(), 6, "六大技能");
        assert_eq!(d.resists.len(), 5);
        assert_eq!(d.specials.len(), 3);

        let perception = d.skills.iter().find(|v| v.label == "感知").unwrap();
        assert_eq!(perception.adjustment, 5);
        assert_eq!(perception.note, "靈巧之靴");
        assert_eq!(
            perception.total,
            perception.formula + 5,
            "顯示值＝公式值＋修正，三個數字都要拿得到"
        );
    }

    /// 創角色須知的範例：難度 1 學到等級 3 是 14 點。
    #[test]
    fn 專長花費與兩軌投資() {
        let mut s = sheet();
        s.feats.push(SheetFeat {
            feat_id: "feat:武器使用".into(),
            level: 3,
            track: Track::Melee,
        });

        let d = derive(&s, &context());
        assert_eq!(d.cp.feats, 14);
        assert_eq!(d.cp.melee_invested, 14);
        assert_eq!(d.cp.spell_invested, 0);
    }

    /// 傳奇專長算的是傳奇技能點，一點需以 10 點 CP 兌換。
    #[test]
    fn 傳奇專長以十倍計入cp() {
        let mut s = sheet();
        s.feats.push(SheetFeat {
            feat_id: "feat:傳奇".into(),
            level: 3,
            track: Track::Spell,
        });

        let d = derive(&s, &context());
        assert_eq!(d.cp.feats, 0, "傳奇不計入一般專長花費");
        assert_eq!(d.cp.legend_as_cp, 140, "14 傳奇點 × 10");
        assert_eq!(d.cp.spell_invested, 140, "兩軌投資以 CP 計");
    }

    #[test]
    fn cp收支含種族與屬性調整() {
        let s = sheet();
        let d = derive(&s, &context());

        assert_eq!(d.cp.starting, 200);
        assert_eq!(d.cp.race, -30);
        // 種族是扣除，所以算進花費而不是可用額度
        assert_eq!(d.cp.spent, 30);
        assert_eq!(d.cp.available, 200 + d.cp.attribute_adjustment);
        assert_eq!(d.cp.remaining, d.cp.available - d.cp.spent);
    }

    #[test]
    fn cp超支只是警告() {
        let mut s = sheet();
        s.starting_cp = 0;
        s.feats.push(SheetFeat {
            feat_id: "feat:武器使用".into(),
            level: 5,
            track: Track::Melee,
        });

        let d = derive(&s, &context());
        assert!(d.cp.remaining < 0);
        assert!(
            d.warnings.iter().any(|w| w.contains("超支")),
            "應該有超支警告：{:?}",
            d.warnings
        );
    }

    /// 難度是 `1or2` 的元技能不替它挑數字，只提示。
    #[test]
    fn 非數值難度不計入且提示() {
        let mut s = sheet();
        s.feats.push(SheetFeat {
            feat_id: "feat:知識".into(),
            level: 3,
            track: Track::None,
        });
        let mut ctx = context();
        ctx.feat_costs.insert(
            "feat:知識".into(),
            FeatCost {
                name: "知識".into(),
                difficulty: None,
                difficulty_raw: Some("1or2".into()),
                scale: "cp".into(),
            },
        );

        let d = derive(&s, &ctx);
        assert_eq!(d.cp.feats, 0);
        assert!(
            d.warnings.iter().any(|w| w.contains("1or2")),
            "應提示需 DM 裁定：{:?}",
            d.warnings
        );
    }

    #[test]
    fn 達到獎勵門檻但沒填擲骰結果會提示() {
        let mut s = sheet();
        s.feats.push(SheetFeat {
            feat_id: "feat:武器使用".into(),
            level: 4,
            track: Track::Melee,
        });

        let d = derive(&s, &context());
        assert_eq!(d.cp.melee_invested, 30);
        assert_eq!(d.melee_dice_steps, 2, "達 30 CP 為第 2 階");
        assert!(
            d.warnings.iter().any(|w| w.contains("擲骰結果")),
            "{:?}",
            d.warnings
        );
    }

    #[test]
    fn 基礎hp含屬性獎勵與手填() {
        let mut s = sheet();
        s.bonus.melee_hp = 12;
        s.manual_hp = 5;

        let d = derive(&s, &context());
        // CON 13 → 調整值 0 → 13
        assert_eq!(d.hp.attribute_part, 13);
        assert_eq!(d.hp.rolled, 12);
        assert_eq!(d.hp.manual, 5);
        assert_eq!(d.hp.base, 30);
    }

    #[test]
    fn 額外hp的花費計入cp() {
        let mut s = sheet();
        s.bonus.melee_hp = 12;
        s.manual_hp = 5; // 基礎 30
        s.extra_hp = 60;

        let d = derive(&s, &context());
        assert_eq!(d.hp.extra_cost, 30, "31–60 為 1:1");
        assert_eq!(d.hp.total, 60);
        assert!(d.cp.spent >= 30);
    }

    #[test]
    fn 屬性沒填齊會提示而不是當成零() {
        let mut s = sheet();
        s.attributes.remove("SPI");

        let d = derive(&s, &context());
        assert!(
            d.warnings.iter().any(|w| w.contains("SPI")),
            "{:?}",
            d.warnings
        );
    }
}
