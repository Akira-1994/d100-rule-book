#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""D100 規則書 — Phase 2 Tier A／B 解析器。

涵蓋十一張工作表：
  Tier A（欄位完全整齊）基本／一般／高級／超魔專長、種族、
          一般／高階／永恆聖器詞綴
  Tier B（有前言區塊、合併欄位或父子結構）製作專長、傳奇專長、素材詞綴

把它們從「一格一格的字串」轉成結構化紀錄，但維持一條底線：
**解析不出來就留 NULL 並記一筆 issue，絕不臆測內容**。原始字串一律保留。

每個解析器回傳 (records, issues)，issues 是 (row, code, detail) 三元組，
由 build_db.py 收集後寫進資料庫的 errata 表並交給 validate.py 檢查。
"""

from __future__ import annotations

import re

from rawio import (
    cell,
    clean_name,
    to_halfwidth,
    parse_attr_modifiers,
    parse_categories,
    parse_int,
    parse_number,
    parse_plus_costs,
    parse_slots,
    read_sheet,
)

# 專長類工作表的版面設定。
#   header       表頭列索引（0-based），之前的列都是前言，進 rule_text
#   cat/diff/prereq/effect  各欄的索引，None 代表這張表沒有這一欄
#   combined     難度欄同時寫了難度與分類，例如傳奇專長的「6（戰鬥）」
#   star_tag     名稱尾端的 ** 代表可由「賢者之觸」詞墜提升等級（製作專長）
FEAT_SHEETS = {
    "基本專長": {
        "group": "basic", "header": 2,
        "cat": 1, "diff": 2, "prereq": None, "effect": 3,
    },
    "一般專長": {
        "group": "general", "header": 0,
        "cat": 1, "diff": 2, "prereq": 3, "effect": 4,
    },
    "高級專長": {
        "group": "advanced", "header": 0,
        "cat": 1, "diff": 2, "prereq": 3, "effect": 4,
    },
    "超魔專長": {
        "group": "metamagic", "header": 2,
        "cat": 1, "diff": 2, "prereq": None, "effect": 3,
    },
    "製作專長": {
        "group": "crafting", "header": 2,
        "cat": 1, "diff": 2, "prereq": 3, "effect": 4,
        "star_tag": "賢者之觸可提升等級",
    },
    "傳奇專長": {
        "group": "legendary", "header": 1,
        "cat": None, "diff": 1, "prereq": None, "effect": 2,
        "combined": True,
    },
}

AFFIX_SHEETS = {
    "一般詞綴": ("general", 1, True),
    "高階詞綴": ("advanced", 1, True),
    "永恆聖器詞綴": ("eternal", 1, False),
}

_CN_DIGITS = {
    "一": 1, "二": 2, "三": 3, "四": 4, "五": 5,
    "六": 6, "七": 7, "八": 8, "九": 9, "十": 10,
}


def feat_id(group: str, name: str) -> str:
    return f"feat:{group}:{name}"


# 傳奇專長的難度欄把難度與分類寫在一起：「6（戰鬥）」「傳1（知識）」
# 「難度6(操作)」。其中「傳N」是傳奇技能點而非 CP 難度 —— 依該表前言，
# 一點傳奇技能點需消耗 10 點 CP 兌換，是另一套計價單位。
_COMBINED_DIFFICULTY = re.compile(r"^(?:難度)?\s*(傳)?\s*(\d+)\s*\((.+)\)\s*$")


def parse_combined_difficulty(text: str):
    """拆解「6（戰鬥）」這類把難度與分類寫在同一格的欄位。

    回傳 (difficulty, scale, categories, unknown)；解析失敗時 difficulty 為 None。
    """
    match = _COMBINED_DIFFICULTY.match(to_halfwidth(text).strip())
    if not match:
        return None, "cp", [], []
    scale = "legend" if match.group(1) else "cp"
    categories, unknown = parse_categories(match.group(3))
    return float(match.group(2)), scale, categories, unknown


def parse_feat_sheet(sheet: str, raw_dir=None):
    """解析一張專長表。"""
    config = FEAT_SHEETS[sheet]
    group = config["group"]
    header_row = config["header"]
    rows = read_sheet(sheet, raw_dir)
    records, issues = [], []

    for index in range(header_row + 1, len(rows)):
        source_row = index + 1  # 對齊 Excel 的 1-based 列號
        name_raw = cell(rows, index, 0)
        if not name_raw.strip():
            continue

        name = clean_name(name_raw)
        tags = []
        if config.get("star_tag") and name.endswith("**"):
            name = name[:-2].strip()
            tags.append(config["star_tag"])

        difficulty_raw = cell(rows, index, config["diff"]).strip()
        scale = "cp"

        if config.get("combined"):
            difficulty, scale, categories, unknown = parse_combined_difficulty(
                difficulty_raw
            )
            if difficulty_raw and difficulty is None:
                issues.append(
                    (source_row, "difficulty_not_numeric",
                     f"難度欄 {difficulty_raw!r} 無法拆出難度與分類")
                )
        else:
            difficulty = parse_number(difficulty_raw)
            if difficulty_raw and difficulty is None:
                issues.append(
                    (source_row, "difficulty_not_numeric", f"難度欄為 {difficulty_raw!r}")
                )
            categories, unknown = parse_categories(cell(rows, index, config["cat"]))

        if unknown:
            issues.append(
                (source_row, "category_unrecognized", f"無法辨識的分類：{unknown}")
            )
        if not categories:
            issues.append((source_row, "missing_category", "分類欄為空"))
        if not difficulty_raw:
            issues.append((source_row, "missing_difficulty", "難度欄為空"))

        effect = cell(rows, index, config["effect"]).strip()
        if not effect:
            issues.append((source_row, "missing_effect", "效果欄為空"))

        prereq_col = config["prereq"]
        prereq_raw = cell(rows, index, prereq_col) if prereq_col is not None else ""

        records.append(
            {
                "id": feat_id(group, name),
                "name": name,
                "feat_group": group,
                "difficulty": difficulty,
                "difficulty_raw": difficulty_raw or None,
                "difficulty_scale": scale,
                "parent_id": None,
                "effect": effect,
                "categories": categories,
                "tags": tags,
                "prereq_raw": prereq_raw,
                "source_sheet": sheet,
                "source_row": source_row,
            }
        )

    return records, issues


def parse_preamble(sheet: str, header_row: int, raw_dir=None):
    """把表頭之前的前言區塊收進 rule_text。

    這些段落寫的是該表的通用規則（製作耗時、傳奇專長的解鎖門檻、
    CP 計算公式等），跟條目一樣重要，不能因為它們不在表格裡就丟掉。
    """
    rows = read_sheet(sheet, raw_dir)
    blocks = []
    for index in range(header_row):
        for col, value in enumerate(rows[index] if index < len(rows) else []):
            text = value.strip()
            if text:
                blocks.append(
                    {
                        "sheet": sheet,
                        "source_row": index + 1,
                        "source_col": col + 1,
                        "body": text,
                    }
                )
    return blocks


_LEVEL_SUFFIX = re.compile(r"^(?P<name>.+?)\s*(?P<num>[0-9]+|[一二三四五六七八九十])\s*級$")
_LEVEL_PREFIX = re.compile(r"^(?P<name>.+?)\s*等級\s*(?P<num>[0-9]+)$")
_CASTER_RING = re.compile(
    r"(?:環數|施法能力|施法).*?([0-9]+|[一二三四五六七八九十])\s*環"
)
_STAT_THRESHOLD = re.compile(
    r"^(戰鬥|運動|操作|感知|知識|交涉)數值\s*[>＞≧≥]=?\s*([0-9]+)$"
)
# 條件末尾的括號補述，例如「武器使用等級4（雙手武器類別）」。
# 補述不參與比對，但完整原文仍保留在 raw_text 供顯示。
_TRAILING_QUALIFIER = re.compile(r"[（(][^（()）]*[）)]\s*$")
_BLOODLINE = re.compile(r"^綁定血脈[:：]\s*(.+)$")
_ATTR_THRESHOLD = re.compile(
    r"^(STR|DEX|SKI|CON|RES|INT|WIS|CHA|SPI)\s*([0-9]+)$", re.IGNORECASE
)
# 承接上一行的續行：以括號包住的補述，或以「及」「與」開頭的下半句。
_CONTINUATION = re.compile(r"^[（(]|^[及與]")


def _to_level(text: str):
    if text.isdigit():
        return int(text)
    return _CN_DIGITS.get(text)


_WRAPPED = re.compile(r"^[（(](.*)[）)]$", re.DOTALL)


def _unwrap(text: str) -> str:
    """整串被括號包住時取出內容，否則原樣回傳。"""
    match = _WRAPPED.match(text.strip())
    return match.group(1).strip() if match else text.strip()


def _looks_like_condition(text: str) -> bool:
    """這一行本身是不是一個認得出來的條件？"""
    inner = _unwrap(text)
    return bool(
        _CASTER_RING.search(inner)
        or _ATTR_THRESHOLD.match(inner)
        or _STAT_THRESHOLD.match(inner)
        or _LEVEL_SUFFIX.match(inner)
        or _LEVEL_PREFIX.match(inner)
    )


def split_prereq_lines(prereq_raw: str):
    """把前置欄拆成一條一條的條件。

    原表只用換行分隔條件，但作者常把一句話折成兩行，例如
        同時具備奧術、神術
        及自然/詩歌施法能力
    或把補述另起一行：
        武器使用等級4
        （雙手武器類別）
    這些續行必須併回上一條，否則會被切成沒有意義的碎片。

    但括號裡若本身就是一個看得懂的條件（例如「（且環數不低於4環）」），
    那是獨立要求而非補述，必須留成單獨一條，否則環數門檻會被吃掉。
    """
    lines = [line.strip() for line in prereq_raw.split("\n") if line.strip()]
    merged = []
    for line in lines:
        if merged and _CONTINUATION.match(line) and not _looks_like_condition(line):
            merged[-1] = merged[-1] + line
        else:
            merged.append(line)
    return merged


def parse_prereqs(prereq_raw: str, feat_index: dict):
    """把自由文字的前置條件拆成可驗證的結構。

    feat_index 是 {專長名稱: feat_id}，讓「跑步二級」這種寫法能連回實際條目。
    認不出來的一律標成 free 並保留原文，只顯示、不驗證 —— 這是刻意的：
    寧可少驗一條，也不要因為誤判而擋住合法的角色。
    """
    if not prereq_raw or not prereq_raw.strip():
        return []

    results = []
    for part in split_prereq_lines(prereq_raw):
        # 比對用的文字剝掉外層括號與尾端補述；raw_text 一律保留完整原文。
        subject = _unwrap(part)
        subject = _TRAILING_QUALIFIER.sub("", subject).strip() or subject

        stat = _STAT_THRESHOLD.match(subject)
        if stat:
            results.append(
                {"kind": "stat", "ref_feat_id": None, "ref_code": stat.group(1),
                 "min_level": int(stat.group(2)), "raw_text": part}
            )
            continue

        attr = _ATTR_THRESHOLD.match(subject)
        if attr:
            results.append(
                {"kind": "attribute", "ref_feat_id": None,
                 "ref_code": attr.group(1).upper(),
                 "min_level": int(attr.group(2)), "raw_text": part}
            )
            continue

        bloodline = _BLOODLINE.match(subject)
        if bloodline:
            results.append(
                {"kind": "bloodline", "ref_feat_id": None,
                 "ref_code": clean_name(bloodline.group(1)),
                 "min_level": None, "raw_text": part}
            )
            continue

        ring = _CASTER_RING.search(subject)
        if ring:
            results.append(
                {"kind": "caster_ring", "ref_feat_id": None, "ref_code": None,
                 "min_level": _to_level(ring.group(1)), "raw_text": part}
            )
            continue

        level_match = _LEVEL_SUFFIX.match(subject) or _LEVEL_PREFIX.match(subject)
        if level_match:
            base = clean_name(level_match.group("name"))
            ref = feat_index.get(base)
            results.append(
                {
                    # 認得名字才算結構化；認不得多半是這個專長還在 Tier B/C
                    # 尚未解析的表裡，等那些表接上來就會自動升級。
                    "kind": "feat" if ref else "free",
                    "ref_feat_id": ref,
                    "ref_code": None if ref else base,
                    "min_level": _to_level(level_match.group("num")),
                    "raw_text": part,
                }
            )
            continue

        ref = feat_index.get(clean_name(subject))
        if ref:
            results.append(
                {"kind": "feat", "ref_feat_id": ref, "ref_code": None,
                 "min_level": 1, "raw_text": part}
            )
            continue

        results.append(
            {"kind": "free", "ref_feat_id": None, "ref_code": None,
             "min_level": None, "raw_text": part}
        )

    return results


def parse_races(raw_dir=None):
    """解析種族與其調整。"""
    sheet = "種族與其調整"
    rows = read_sheet(sheet, raw_dir)
    records, issues = [], []

    for index in range(1, len(rows)):
        source_row = index + 1
        name_raw = cell(rows, index, 0)
        if not name_raw.strip():
            continue

        name = clean_name(name_raw)
        if name != name_raw:
            issues.append(
                (source_row, "name_normalized", f"名稱原為 {name_raw!r}，已正規化為 {name!r}")
            )

        cp_raw = cell(rows, index, 4).strip()
        cp_cost = parse_int(cp_raw)
        if cp_raw and cp_cost is None:
            issues.append((source_row, "cp_not_numeric", f"CP 調整欄為 {cp_raw!r}"))

        attr_text = cell(rows, index, 1).strip()
        modifiers = parse_attr_modifiers(attr_text)

        records.append(
            {
                "id": f"race:{name}",
                "name": name,
                "cp_cost": cp_cost,
                "cp_raw": cp_raw,
                "attr_text": attr_text or None,
                "racial_feat_text": cell(rows, index, 2).strip() or None,
                "skill_mod_text": cell(rows, index, 3).strip() or None,
                "special_text": cell(rows, index, 5).strip() or None,
                "attr_modifiers": modifiers,
                "source_sheet": sheet,
                "source_row": source_row,
            }
        )

    return records, issues


def parse_affix_sheet(sheet: str, raw_dir=None):
    """解析一張詞綴表。"""
    tier, header_row, has_cost = AFFIX_SHEETS[sheet]
    rows = read_sheet(sheet, raw_dir)
    records, issues = [], []

    effect_col = 3 if has_cost else 2

    for index in range(header_row + 1, len(rows)):
        source_row = index + 1
        name_raw = cell(rows, index, 0)
        if not name_raw.strip():
            continue

        name = clean_name(name_raw)
        slots = parse_slots(cell(rows, index, 1))
        if not slots:
            issues.append((source_row, "missing_slot", "部位欄為空或無法解析"))

        cost_raw = cell(rows, index, 2).strip() if has_cost else ""
        ranks = parse_plus_costs(cost_raw) if has_cost else []
        if has_cost and cost_raw and not ranks:
            issues.append(
                (source_row, "plus_cost_unparsed", f"所需加值兌換欄為 {cost_raw!r}")
            )

        effect = cell(rows, index, effect_col).strip()
        if not effect:
            issues.append((source_row, "missing_effect", "效果欄為空"))

        records.append(
            {
                "id": f"affix:{tier}:{name}",
                "name": name,
                "affix_tier": tier,
                "plus_cost_raw": cost_raw or None,
                "effect": effect,
                "slots": slots,
                "ranks": ranks,
                "source_sheet": sheet,
                "source_row": source_row,
            }
        )

    return records, issues


_ROLL_RANGE = re.compile(r"^(\d+)\s*(?:[~-]\s*(\d+))?$")


def parse_roll_range(text: str):
    """解析素材詞綴的機率欄：'1~70' 或單一的 '100'。"""
    match = _ROLL_RANGE.match(to_halfwidth(text).strip())
    if not match:
        return None
    low = int(match.group(1))
    high = int(match.group(2)) if match.group(2) else low
    return (low, high) if low <= high else None


def parse_materials(raw_dir=None):
    """解析素材詞綴。

    版面是父子結構：一列素材（素材／部位／階級／加工費用倍率）後面跟著
    數列詞綴（詞綴／稀有倍率／機率／效果），子列的前四欄因為合併儲存格
    而留空。

    另有一個變化：精金與密銀的同一個機率階有兩條詞綴 —— 一條給武器、
    一條給主要裝備與盾牌。後者的稀有倍率與機率欄留空，代表沿用上一列的
    階級，而不是它自己沒有機率。
    """
    sheet = "素材詞綴"
    rows = read_sheet(sheet, raw_dir)
    materials, affixes, issues = [], [], []

    current = None
    last_tier = None

    for index in range(1, len(rows)):
        source_row = index + 1
        material_name = clean_name(cell(rows, index, 0))
        affix_name = clean_name(cell(rows, index, 4))

        if material_name:
            tier = parse_int(cell(rows, index, 2))
            multiplier = parse_number(cell(rows, index, 3))
            if tier is None:
                issues.append((source_row, "missing_material_tier", "素材階級欄無法解析"))
            if multiplier is None:
                issues.append(
                    (source_row, "missing_cost_multiplier", "加工費用倍率欄無法解析")
                )
            current = {
                "id": f"material:{material_name}",
                "name": material_name,
                "material_tier": tier,
                "cost_multiplier": multiplier,
                "slots": parse_slots(cell(rows, index, 1)),
                "source_sheet": sheet,
                "source_row": source_row,
            }
            if not current["slots"]:
                issues.append((source_row, "missing_slot", "部位欄為空或無法解析"))
            materials.append(current)
            last_tier = None

        if not affix_name:
            continue
        if current is None:
            issues.append((source_row, "orphan_affix", f"詞綴「{affix_name}」沒有對應的素材"))
            continue

        rarity = parse_number(cell(rows, index, 5))
        rolls = parse_roll_range(cell(rows, index, 6))

        if rolls is None:
            # 機率留空代表沿用上一列的階級（同階的另一種裝備適用詞綴）。
            if last_tier is None:
                issues.append(
                    (source_row, "unresolved_roll_range",
                     f"詞綴「{affix_name}」的機率欄為空，且前面沒有可沿用的階級")
                )
                continue
            rolls, rarity = last_tier["rolls"], last_tier["rarity"]
            tier_rank = last_tier["rank"]
        else:
            tier_rank = (last_tier["rank"] + 1) if last_tier else 1
            last_tier = {"rolls": rolls, "rarity": rarity, "rank": tier_rank}

        effect = cell(rows, index, 7).strip()
        if not effect:
            issues.append((source_row, "missing_effect", f"詞綴「{affix_name}」效果欄為空"))

        affixes.append(
            {
                "id": f"material_affix:{current['name']}:{affix_name}",
                "material_id": current["id"],
                "name": affix_name,
                "tier_rank": tier_rank,
                "rarity_multiplier": rarity,
                "roll_min": rolls[0],
                "roll_max": rolls[1],
                "effect": effect,
                "source_sheet": sheet,
                "source_row": source_row,
            }
        )

    return materials, affixes, issues


def parse_all(raw_dir=None):
    """跑完 Tier A 的全部解析，回傳 (資料, issues)。

    注意：此時 feat 紀錄帶的是尚未解析的 prereq_raw。前置條件要等勘誤套用
    完畢、而且所有專長都認識了之後才解析得動，因此由呼叫端在套用勘誤後
    自行呼叫 resolve_prereqs()。
    """
    feats, affixes, issues, rule_texts = [], [], [], []

    for sheet, config in FEAT_SHEETS.items():
        records, sheet_issues = parse_feat_sheet(sheet, raw_dir)
        feats.extend(records)
        issues.extend((sheet,) + i for i in sheet_issues)
        rule_texts.extend(parse_preamble(sheet, config["header"], raw_dir))

    races, race_issues = parse_races(raw_dir)
    issues.extend(("種族與其調整",) + i for i in race_issues)

    for sheet, (_tier, header_row, _has_cost) in AFFIX_SHEETS.items():
        records, sheet_issues = parse_affix_sheet(sheet, raw_dir)
        affixes.extend(records)
        issues.extend((sheet,) + i for i in sheet_issues)
        rule_texts.extend(parse_preamble(sheet, header_row, raw_dir))

    materials, material_affixes, material_issues = parse_materials(raw_dir)
    issues.extend(("素材詞綴",) + i for i in material_issues)

    return {
        "feats": feats,
        "races": races,
        "affixes": affixes,
        "materials": materials,
        "material_affixes": material_affixes,
        "rule_texts": rule_texts,
    }, issues


def resolve_prereqs(data: dict):
    """把每個專長的 prereq_raw 解析成結構化的前置條件。

    必須在勘誤套用之後才呼叫，否則修正過的前置文字不會生效。
    """
    feat_index = {f["name"]: f["id"] for f in data["feats"]}
    for record in data["feats"]:
        record["prereqs"] = parse_prereqs(record.pop("prereq_raw"), feat_index)
