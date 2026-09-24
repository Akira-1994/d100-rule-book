//! 規則書的核心計算公式。
//!
//! 這一份必須與 `tools/rules.py` 對得起來 —— 那邊是資料管線在用的，
//! 這邊是應用在用的，Phase 4 的角色卡計算也會走這裡。兩份實作分歧時
//! 下面的回歸測試會抓到：基準值與 `tools/validate.py` 拿「法師範例」
//! 那張角色卡驗算的是同一組。
//!
//! 公式的出處是「創角色須知」與「傳奇專長」的前言原文，改動前請先回去
//! 讀那兩段，不要憑印象調整。

use std::collections::BTreeMap;

use serde::Serialize;

/// 九大屬性的數值，鍵是資料庫 `attribute` 表的代碼（STR、DEX⋯）。
///
/// 用 map 而不是九個具名欄位：公式是「DEX＋SKI＋STR」這種以代碼查表的
/// 形式，具名欄位會讓每條公式都變成一串 match。
pub type Scores = BTreeMap<String, i64>;

/// 六大技能判定的組成（創角色須知）。
///
/// 知識不在這裡 —— 它是 `(INT＋WIS)×1.5`，係數不同，單獨處理。
pub const SKILL_FORMULAS: &[(&str, [&str; 3])] = &[
    ("戰鬥", ["DEX", "SKI", "STR"]),
    ("運動", ["DEX", "SKI", "CON"]),
    ("操作", ["INT", "SKI", "WIS"]),
    ("感知", ["INT", "RES", "SPI"]),
    ("交涉", ["CHA", "WIS", "SPI"]),
];

pub const KNOWLEDGE: &str = "知識";

/// 五大抗性判定。抗轉化是 RES×2，寫成 RES＋RES 讓所有抗性同一個形狀。
pub const RESIST_FORMULAS: &[(&str, [&str; 2])] = &[
    ("抗毒素", ["RES", "CON"]),
    ("抗控制", ["RES", "WIS"]),
    ("抗轉化", ["RES", "RES"]),
    ("抗噴吐", ["RES", "DEX"]),
    ("抗魔法", ["RES", "INT"]),
];

/// 三種特殊判定：**不計屬性調整值**，直接以屬性值 ×5。
pub const SPECIAL_FORMULAS: &[(&str, (&str, i64))] = &[
    ("強韌", ("CON", 5)),
    ("精神", ("RES", 5)),
    ("靈魂", ("SPI", 5)),
];

/// 獎勵 HP/SP 的 CP 投資門檻（創角色須知）。
pub const BONUS_CP_THRESHOLDS: &[i64] = &[10, 30, 70, 150, 300, 620, 1020];

/// 屬性調整值總和的基準。超過每點扣 10 CP，低於每點加 10 CP。
pub const MODIFIER_BASELINE: i64 = 10;

/// 一點傳奇技能點的 CP 價碼。
///
/// 傳奇專長前言：「傳奇難度需消耗傳奇技能點，一點傳奇技能點需消耗
/// １０點ＣＰ兌換。」
///
/// 注意這是**推論而非原文**：前言只寫了兌換率，沒有明說 `2^等級 × 難度`
/// 這條公式同樣適用於傳奇技能點。我們的實作是「傳奇點花費照一般公式算，
/// 再乘以兌換率換成 CP」—— 例如〈時間扭曲－逆流〉（傳2）學到 3 級是
/// (2＋4＋8)×2 = 28 傳奇點 = 280 CP。若作者的原意是「傳奇技能不分等級、
/// 一級固定 N 點」，算出來會差很多，屆時改這裡與 `cp_plan` 即可。
pub const LEGEND_POINT_IN_CP: f64 = 10.0;

/// 成本表能算到第幾級。規則書的範例只列到 5，再高沒有依據。
pub const MAX_LEVEL: i64 = 5;

#[derive(Serialize)]
pub struct CpStep {
    pub level: i64,
    /// 這一級本身的花費。單位依 scale 而定：`cp` 是 CP，`legend` 是傳奇技能點。
    pub step_cost: f64,
    /// 從 1 級累計到本級。等級 0 不計入 —— 它是獨立選項而非升級的第一階。
    pub cumulative: f64,
    /// 累計換算成 CP。scale 為 `cp` 時與 `cumulative` 相同。
    pub cumulative_cp: f64,
}

#[derive(Serialize)]
pub struct CpPlan {
    pub difficulty: f64,
    pub scale: String,
    pub steps: Vec<CpStep>,
}

/// 學到「第 level 級」這一級本身所需的花費。
///
/// 創角色須知：「CP消耗公式為 2^（等級）×技能難度」。
pub fn cost_for_level(level: i64, difficulty: f64) -> f64 {
    (2f64).powi(level as i32) * difficulty
}

/// 等級 0 到 `MAX_LEVEL` 的成本表。
///
/// 等級 0 是獨立選項（只為免除該技能判定的 20% 減值），計價方式與其他等級
/// 相同（2^0 × 難度），但**不計入升級的累積成本** —— 原文的範例
/// 「武器使用學到等級 3 為 (2＋4＋8)×1 = 14 點」就沒有把等級 0 算進去。
pub fn cp_plan(difficulty: f64, scale: &str) -> CpPlan {
    let factor = if scale == "legend" { LEGEND_POINT_IN_CP } else { 1.0 };

    let mut cumulative = 0.0;
    let steps = (0..=MAX_LEVEL)
        .map(|level| {
            let step_cost = cost_for_level(level, difficulty);
            if level >= 1 {
                cumulative += step_cost;
            }
            let shown = if level == 0 { 0.0 } else { cumulative };
            CpStep {
                level,
                step_cost,
                cumulative: shown,
                cumulative_cp: shown * factor,
            }
        })
        .collect();

    CpPlan {
        difficulty,
        scale: scale.to_string(),
        steps,
    }
}

// 屬性與衍生判定 --------------------------------------------------------

/// 屬性調整值。
///
/// 創角色須知：13 的調整值為 0，每增減 2 點則 ±1。原文列舉
/// 7(-3) 8(-3) 9(-2) 10(-2) 11(-1) 12(-1) 13(0) 14(+1) 15(+1)
/// 16(+2) 17(+2) 18(+3) 19(+3)。
pub fn attribute_modifier(score: i64) -> i64 {
    let delta = score - 13;
    let magnitude = (delta.abs() + 1) / 2;
    if delta >= 0 { magnitude } else { -magnitude }
}

/// 屬性值加上其調整值 —— 技能與抗性判定都以這個數字為基礎。
pub fn effective(scores: &Scores, attr: &str) -> Result<i64, String> {
    let score = *scores
        .get(attr)
        .ok_or_else(|| format!("缺少屬性 {attr}"))?;
    Ok(score + attribute_modifier(score))
}

/// 六大技能判定數值。
pub fn skill_value(scores: &Scores, skill: &str) -> Result<i64, String> {
    if skill == KNOWLEDGE {
        // 知識 =（INT＋WIS）×1.5，**小數無條件捨去**。
        //
        // Patch note 1.1 的「≥0.5 進位」只適用於擲骰結果，不適用於建卡時
        // 算好的衍生數值 —— 這是規則書作者 2026-09-20 的裁示，與「法師
        // 範例」一致：(24+19)×1.5 = 64.5，卡上寫 64。
        let base = effective(scores, "INT")? + effective(scores, "WIS")?;
        return Ok(base * 3 / 2);
    }
    let parts = SKILL_FORMULAS
        .iter()
        .find(|(name, _)| *name == skill)
        .map(|(_, parts)| parts)
        .ok_or_else(|| format!("沒有這個技能判定：{skill}"))?;
    parts.iter().try_fold(0, |sum, attr| Ok(sum + effective(scores, attr)?))
}

/// 五大抗性判定數值。
pub fn resist_value(scores: &Scores, resist: &str) -> Result<i64, String> {
    let parts = RESIST_FORMULAS
        .iter()
        .find(|(name, _)| *name == resist)
        .map(|(_, parts)| parts)
        .ok_or_else(|| format!("沒有這個抗性判定：{resist}"))?;
    parts.iter().try_fold(0, |sum, attr| Ok(sum + effective(scores, attr)?))
}

/// 強韌／精神／靈魂。原文註明不計屬性調整值。
pub fn special_value(scores: &Scores, special: &str) -> Result<i64, String> {
    let (attr, multiplier) = SPECIAL_FORMULAS
        .iter()
        .find(|(name, _)| *name == special)
        .map(|(_, spec)| *spec)
        .ok_or_else(|| format!("沒有這個特殊判定：{special}"))?;
    let score = *scores
        .get(attr)
        .ok_or_else(|| format!("缺少屬性 {attr}"))?;
    Ok(score * multiplier)
}

// CP 與 HP/SP ------------------------------------------------------------

/// 九大屬性調整值總和造成的起始 CP 增減。
///
/// 創角色須知：總和超過 10，每多一點扣 10 CP；低於 10 則每少一點加 10 CP。
/// 回傳值為對起始 CP 的增減量（正數代表加 CP）。
pub fn attribute_cp_adjustment(scores: &Scores) -> i64 {
    let total: i64 = scores.values().map(|s| attribute_modifier(*s)).sum();
    (MODIFIER_BASELINE - total) * 10
}

/// 依投入 CP 算出獎勵 HP/SP 的骰數階級。
///
/// 階級決定要擲幾顆骰，**骰出來的點數程式算不出來** —— 那是原始輸入，
/// 由玩家填回角色卡。
pub fn bonus_dice_steps(invested_cp: i64) -> i64 {
    BONUS_CP_THRESHOLDS
        .iter()
        .take_while(|threshold| invested_cp >= **threshold)
        .count() as i64
}

/// 以 CP 加購 HP／SP 的累進費率。
///
/// 創角色須知：基礎值的兩倍內 1 CP 換 1 點，兩倍到三倍內 2 CP 換 1 點，
/// 三倍到四倍內 3 CP 換 1 點，依此類推。
pub fn extra_hp_cp_cost(base: i64, target: i64) -> i64 {
    if base <= 0 || target <= base {
        return 0;
    }
    (base + 1..=target)
        .map(|point| ((point - 1) / base).max(1))
        .sum()
}

/// 基礎 HP。
///
/// 創角色須知：`基礎HP＝CON＋CON調整值＋獎勵進戰CP＋獎勵法術CP＋
/// 專長表之技能＋魔法物品之詞綴`。
///
/// 後兩項是效果敘述裡的自由文字，程式讀不出來，由 `manual` 帶進來；
/// 獎勵那兩項是擲出來的點數，同樣是輸入而非計算。
pub fn base_hp(scores: &Scores, bonus: i64, manual: i64) -> Result<i64, String> {
    Ok(effective(scores, "CON")? + bonus + manual)
}

/// 基礎 SP。與 `base_hp` 同一條公式，只是主屬性換成 RES。
pub fn base_sp(scores: &Scores, bonus: i64, manual: i64) -> Result<i64, String> {
    Ok(effective(scores, "RES")? + bonus + manual)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 法師範例回歸 -------------------------------------------------------
    //
    // 規則書沒有測試，但它附了一張算好的角色卡。這是唯一能確認我們對規則
    // 的理解沒跑掉的辦法。
    //
    // `tools/validate.py` 也是在執行時讀同一個檔案而不是寫死數字 ——
    // 兩份實作釘在同一個真相上，上游試算表改了兩邊都會發現。

    use std::collections::BTreeMap;
    use std::path::PathBuf;

    const ATTRIBUTES: &[&str] = &[
        "STR", "DEX", "SKI", "CON", "RES", "INT", "WIS", "CHA", "SPI",
    ];

    fn fixture() -> Vec<Vec<String>> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("data")
            .join("raw")
            .join("法師範例.tsv");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "讀不到 {}：{e}\n測試需要它做回歸，請先執行 python tools/extract.py",
                path.display()
            )
        });
        text.lines()
            .map(|line| line.split('\t').map(|c| c.trim().to_string()).collect())
            .collect()
    }

    fn cell(rows: &[Vec<String>], row: usize, col: usize) -> String {
        rows.get(row)
            .and_then(|r| r.get(col))
            .cloned()
            .unwrap_or_default()
    }

    /// 九大屬性在 C3:C11（0-based 的第 2–10 列、第 2 欄），與 validate.py 一致。
    fn sheet_scores(rows: &[Vec<String>]) -> Scores {
        let mut scores: Scores = BTreeMap::new();
        for index in 2..11 {
            let code = cell(rows, index, 0);
            let raw = cell(rows, index, 2);
            if ATTRIBUTES.contains(&code.as_str()) && !raw.is_empty() {
                scores.insert(code, raw.parse::<f64>().unwrap() as i64);
            }
        }
        scores
    }

    #[test]
    fn 範例卡讀得出九個屬性() {
        let rows = fixture();
        let scores = sheet_scores(&rows);
        assert_eq!(scores.len(), 9, "應讀到 9 個屬性，實際 {}", scores.len());
    }

    #[test]
    fn 調整值與範例卡的d欄一致() {
        let rows = fixture();
        let scores = sheet_scores(&rows);

        let mut checked = 0;
        for index in 2..11 {
            let code = cell(&rows, index, 0);
            let expected = cell(&rows, index, 3);
            if ATTRIBUTES.contains(&code.as_str()) && !expected.is_empty() {
                let actual = attribute_modifier(scores[&code]);
                assert_eq!(
                    actual,
                    expected.parse::<f64>().unwrap() as i64,
                    "{code}={} 的調整值",
                    scores[&code]
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 9, "九個屬性的調整值都要驗到");
    }

    /// 六大技能、五抗性、三特殊 —— 範例卡上算好的十六項。
    #[test]
    fn 十六項衍生數值與範例卡相符() {
        let rows = fixture();
        let scores = sheet_scores(&rows);

        // 技能與特殊在 E/H 欄，抗性在 I/L 欄，與 validate.py 的讀法一致。
        let mut expected: Vec<(String, i64)> = Vec::new();
        for index in 1..12 {
            let label = cell(&rows, index, 4);
            let value = cell(&rows, index, 7);
            if !label.is_empty() {
                if let Ok(v) = value.parse::<f64>() {
                    expected.push((label, v as i64));
                }
            }
        }
        for index in 1..7 {
            let label = cell(&rows, index, 8);
            let value = cell(&rows, index, 11);
            if !label.is_empty() {
                if let Ok(v) = value.parse::<f64>() {
                    expected.push((label, v as i64));
                }
            }
        }

        let mut checked = 0;
        for (label, want) in &expected {
            let got = if label == KNOWLEDGE || SKILL_FORMULAS.iter().any(|(n, _)| n == label) {
                skill_value(&scores, label)
            } else if RESIST_FORMULAS.iter().any(|(n, _)| n == label) {
                resist_value(&scores, label)
            } else if SPECIAL_FORMULAS.iter().any(|(n, _)| n == label) {
                special_value(&scores, label)
            } else {
                continue;
            };
            assert_eq!(got.unwrap(), *want, "範例卡「{label}」");
            checked += 1;
        }

        assert_eq!(checked, 14, "範例卡上應有 14 項可驗的衍生數值，實際 {checked}");
    }

    /// 知識是唯一會產生小數的一項，也是唯一需要裁示才能定案的一項：
    /// 範例卡的 (24+19)×1.5 = 64.5 寫成 64 —— 無條件捨去，不是四捨五入。
    ///
    /// 這裡刻意用規則書實際列舉過的 7–19 範圍內的屬性值，不依賴調整值
    /// 公式在該範圍外的外插。
    #[test]
    fn 知識無條件捨去() {
        // INT 18(+3)=21、WIS 16(+2)=18 → (21+18)×1.5 = 58.5
        let scores: Scores = [("INT", 18), ("WIS", 16)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        assert_eq!(skill_value(&scores, KNOWLEDGE).unwrap(), 58, "58.5 要捨去成 58");

        // INT 18(+3)=21、WIS 17(+2)=19 → (21+19)×1.5 = 60，整數不受影響
        let scores: Scores = [("INT", 18), ("WIS", 17)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        assert_eq!(skill_value(&scores, KNOWLEDGE).unwrap(), 60);
    }

    #[test]
    fn 缺少屬性時明白失敗() {
        let scores: Scores = BTreeMap::new();
        let err = skill_value(&scores, "戰鬥").unwrap_err();
        assert!(err.contains("缺少屬性"), "實際訊息：{err}");
    }

    // CP 與 HP/SP --------------------------------------------------------

    /// 創角色須知：調整值總和超過 10 每點扣 10 CP，低於 10 每點加 10 CP。
    #[test]
    fn 屬性調整值影響起始cp() {
        let flat: Scores = ATTRIBUTES.iter().map(|a| (a.to_string(), 13)).collect();
        assert_eq!(attribute_cp_adjustment(&flat), 100, "全部 13（總和 0）應加 100 CP");

        let mut scores = flat.clone();
        scores.insert("STR".into(), 19); // +3
        assert_eq!(attribute_cp_adjustment(&scores), 70, "總和 3 時應加 70 CP");
    }

    #[test]
    fn 獎勵骰數在七個門檻各跳一階() {
        assert_eq!(bonus_dice_steps(9), 0, "未達 10 CP 沒有獎勵");
        assert_eq!(bonus_dice_steps(10), 1);
        assert_eq!(bonus_dice_steps(29), 1);
        assert_eq!(bonus_dice_steps(30), 2);
        assert_eq!(bonus_dice_steps(1020), 7, "最高階");
        assert_eq!(bonus_dice_steps(99999), 7, "超過最高門檻不再增加");
    }

    /// 創角色須知的原文例子：基礎 HP 30 時，30~60 為 1:1、60~90 為 2:1、
    /// 90~120 為 3:1。
    #[test]
    fn 額外hp的累進換匯符合原文例子() {
        assert_eq!(extra_hp_cp_cost(30, 30), 0, "沒有加購就不花 CP");
        assert_eq!(extra_hp_cp_cost(30, 60), 30, "31–60 共 30 點，1:1");
        assert_eq!(extra_hp_cp_cost(30, 90), 30 + 60, "61–90 共 30 點，2:1");
        assert_eq!(extra_hp_cp_cost(30, 120), 30 + 60 + 90, "91–120 共 30 點，3:1");
    }

    #[test]
    fn 基礎hp與sp含獎勵與手填() {
        let scores: Scores = [("CON", 16), ("RES", 18)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        // CON 16 → 調整值 +2 → 18
        assert_eq!(base_hp(&scores, 12, 5).unwrap(), 18 + 12 + 5);
        // RES 18 → 調整值 +3 → 21
        assert_eq!(base_sp(&scores, 9, 0).unwrap(), 21 + 9);
    }

    /// 創角色須知的原文範例：武器使用（難度1）學到等級 3 為
    /// (2＋4＋8)×1 = 14 點，等級五單級為 32 點。
    #[test]
    fn cp表符合規則書範例() {
        let plan = cp_plan(1.0, "cp");
        assert_eq!(plan.steps.len(), 6, "應涵蓋等級 0 到 5");

        assert_eq!(plan.steps[0].step_cost, 1.0, "等級 0 的花費是 2^0 × 難度");
        assert_eq!(plan.steps[0].cumulative, 0.0, "等級 0 不計入升級累計");

        assert_eq!(plan.steps[3].level, 3);
        assert_eq!(plan.steps[3].cumulative, 14.0, "難度1學到等級3應為 14 點");
        assert_eq!(plan.steps[5].step_cost, 32.0, "難度1的等級5單級為 32 點");
    }

    /// 原文把難度 1 的各級列了出來：等級一(2)、二(4)、三(8)、四(16)、五(32)。
    #[test]
    fn 各級單價與原文列舉一致() {
        let plan = cp_plan(1.0, "cp");
        let steps: Vec<f64> = plan.steps.iter().map(|s| s.step_cost).collect();
        assert_eq!(steps, vec![1.0, 2.0, 4.0, 8.0, 16.0, 32.0]);
    }

    /// 傳奇難度算的是傳奇技能點，一點需以 10 點 CP 兌換。
    /// 兩種單位都要給得出來 —— 玩家記帳用的是 CP，但規則書寫的是傳奇點。
    #[test]
    fn 傳奇計價同時給出傳奇點與等值cp() {
        let plan = cp_plan(1.0, "legend");
        assert_eq!(plan.steps[3].cumulative, 14.0, "傳奇點的累計與一般公式相同");
        assert_eq!(
            plan.steps[3].cumulative_cp, 140.0,
            "一點傳奇技能點需 10 點 CP 兌換"
        );

        // 一般計價時兩個欄位相同，前端才不必分兩種版面。
        let plain = cp_plan(1.0, "cp");
        assert_eq!(plain.steps[3].cumulative, plain.steps[3].cumulative_cp);
    }

    /// 難度不是整數的條目（例如 0.5）也要算得出來，不能假設是整數。
    #[test]
    fn 非整數難度不被截斷() {
        let plan = cp_plan(0.5, "cp");
        assert_eq!(plan.steps[1].step_cost, 1.0);
        assert_eq!(plan.steps[3].cumulative, 7.0, "(2+4+8)×0.5");
    }
}
