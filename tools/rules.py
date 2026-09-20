#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""D100 規則書 — 核心計算規則。

這裡是規則書「創角色須知」與「Patch note」裡那些公式的程式化版本。
Phase 4 的角色卡計算會直接用這一份，因此每一條公式都必須能對得上
規則書的原文，而且有實際角色卡（法師範例）可以驗證。
"""

from __future__ import annotations

from decimal import ROUND_HALF_UP, Decimal

# 六大技能判定的組成（創角色須知）
SKILL_FORMULAS = {
    "戰鬥": ("DEX", "SKI", "STR"),
    "運動": ("DEX", "SKI", "CON"),
    "操作": ("INT", "SKI", "WIS"),
    "感知": ("INT", "RES", "SPI"),
    "交涉": ("CHA", "WIS", "SPI"),
    # 知識另有 ×1.5 的係數，單獨處理
}

# 五大抗性判定
RESIST_FORMULAS = {
    "抗毒素": ("RES", "CON"),
    "抗控制": ("RES", "WIS"),
    "抗轉化": ("RES", "RES"),
    "抗噴吐": ("RES", "DEX"),
    "抗魔法": ("RES", "INT"),
}

# 三種特殊判定：不計屬性調整值，直接以屬性值 ×5
SPECIAL_FORMULAS = {
    "強韌": ("CON", 5),
    "精神": ("RES", 5),
    "靈魂": ("SPI", 5),
}

# 獎勵 HP/SP 的 CP 投資門檻（Patch note 1.1 第 2、3 條）
BONUS_CP_THRESHOLDS = (10, 30, 70, 150, 300, 620, 1020)


def attribute_modifier(score: int) -> int:
    """屬性調整值。

    創角色須知：13 的調整值為 0，每增減 2 點則 ±1。
    原文列舉：7(-3) 8(-3) 9(-2) 10(-2) 11(-1) 12(-1) 13(0)
              14(+1) 15(+1) 16(+2) 17(+2) 18(+3) 19(+3)
    """
    delta = score - 13
    magnitude = (abs(delta) + 1) // 2
    return magnitude if delta >= 0 else -magnitude


def round_half_up(value) -> int:
    """Patch note 1.1 第 6 條：小數大於等於 0.5 一律進位。"""
    return int(Decimal(str(value)).quantize(Decimal("1"), rounding=ROUND_HALF_UP))


def effective(scores: dict, attr: str) -> int:
    """屬性值加上其調整值 —— 技能與抗性判定都以這個數字為基礎。"""
    score = scores[attr]
    return score + attribute_modifier(score)


def skill_value(scores: dict, skill: str) -> int:
    """六大技能判定數值。"""
    if skill == "知識":
        # 知識 =（INT＋WIS）×1.5
        base = effective(scores, "INT") + effective(scores, "WIS")
        return round_half_up(base * 1.5)
    return sum(effective(scores, a) for a in SKILL_FORMULAS[skill])


def resist_value(scores: dict, resist: str) -> int:
    """五大抗性判定數值。"""
    return sum(effective(scores, a) for a in RESIST_FORMULAS[resist])


def special_value(scores: dict, special: str) -> int:
    """強韌／精神／靈魂。原文註明不計屬性調整值。"""
    attr, multiplier = SPECIAL_FORMULAS[special]
    return scores[attr] * multiplier


def cp_for_level(level: int, difficulty) -> float:
    """學到「第 level 級」這一級本身所需的 CP。

    創角色須知：CP 消耗公式為 2^(等級) × 技能難度。
    """
    return (2 ** level) * float(difficulty)


def cp_cumulative(level: int, difficulty) -> float:
    """從 1 級一路學到 level 級的總 CP。

    原文範例：武器使用（難度1）學到等級 3 為 (2＋4＋8)×1 = 14 點。
    等級 0 是獨立選項（只為免除 20% 減值），不計入升級的累積成本。
    """
    return sum(cp_for_level(l, difficulty) for l in range(1, level + 1))


def attribute_cp_adjustment(scores: dict) -> int:
    """九大屬性調整值總和造成的起始 CP 增減。

    創角色須知：總和超過 10，每多一點扣 10 CP；低於 10 則每少一點加 10 CP。
    回傳值為對起始 CP 的增減量（正數代表加 CP）。
    """
    total = sum(attribute_modifier(score) for score in scores.values())
    return (10 - total) * 10


def bonus_dice_steps(invested_cp: int) -> int:
    """依投入 CP 算出獎勵 HP/SP 的骰數階級。"""
    steps = 0
    for threshold in BONUS_CP_THRESHOLDS:
        if invested_cp >= threshold:
            steps += 1
        else:
            break
    return steps


def extra_hp_cp_cost(base: int, target: int) -> int:
    """以 CP 加購 HP／SP 的累進費率。

    創角色須知：基礎值的兩倍內 1 CP 換 1 點，兩倍到三倍內 2 CP 換 1 點，
    三倍到四倍內 3 CP 換 1 點，依此類推。
    """
    if target <= base:
        return 0
    cost = 0
    for point in range(base + 1, target + 1):
        # 位於第幾個「倍數區間」決定單價
        tier = (point - 1) // base if base else 1
        cost += max(tier, 1)
    return cost
