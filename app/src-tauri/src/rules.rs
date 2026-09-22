//! 規則書的核心計算公式。
//!
//! 這一份必須與 `tools/rules.py` 對得起來 —— 那邊是資料管線在用的，
//! 這邊是應用在用的，Phase 4 的角色卡計算也會走這裡。兩份實作分歧時
//! 下面的回歸測試會抓到：基準值與 `tools/validate.py` 拿「法師範例」
//! 那張角色卡驗算的是同一組。
//!
//! 公式的出處是「創角色須知」與「傳奇專長」的前言原文，改動前請先回去
//! 讀那兩段，不要憑印象調整。

use serde::Serialize;

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

#[cfg(test)]
mod tests {
    use super::*;

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
