#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""D100 規則書 — Phase 2 Tier A 解析器。

Tier A 指經稽核後欄位完全整齊的八張工作表。這裡把它們從「一格一格的
字串」轉成結構化紀錄，但維持一條底線：**解析不出來就留 NULL 並記一筆
issue，絕不臆測內容**。原始字串一律保留。

每個解析器回傳 (records, issues)，issues 是 (row, code, detail) 三元組，
由 build_db.py 收集後寫進資料庫的 errata 表並交給 validate.py 檢查。
"""

from __future__ import annotations

import re

from rawio import (
    cell,
    clean_name,
    parse_attr_modifiers,
    parse_categories,
    parse_int,
    parse_number,
    parse_plus_costs,
    parse_slots,
    read_sheet,
)

# 工作表 -> (feat_group, 表頭列索引 0-based, 是否有前置欄)
FEAT_SHEETS = {
    "基本專長": ("basic", 2, False),
    "一般專長": ("general", 0, True),
    "高級專長": ("advanced", 0, True),
    "超魔專長": ("metamagic", 2, False),
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


def parse_feat_sheet(sheet: str, raw_dir=None):
    """解析一張專長表。"""
    group, header_row, has_prereq = FEAT_SHEETS[sheet]
    rows = read_sheet(sheet, raw_dir)
    records, issues = [], []

    effect_col = 4 if has_prereq else 3

    for index in range(header_row + 1, len(rows)):
        source_row = index + 1  # 對齊 Excel 的 1-based 列號
        name_raw = cell(rows, index, 0)
        if not name_raw.strip():
            continue

        name = clean_name(name_raw)
        difficulty_raw = cell(rows, index, 2).strip()
        difficulty = parse_number(difficulty_raw)
        if difficulty_raw and difficulty is None:
            issues.append(
                (source_row, "difficulty_not_numeric", f"難度欄為 {difficulty_raw!r}")
            )

        category_raw = cell(rows, index, 1)
        categories, unknown = parse_categories(category_raw)
        if unknown:
            issues.append(
                (source_row, "category_unrecognized", f"無法辨識的分類：{unknown}")
            )
        if not categories:
            issues.append((source_row, "missing_category", "分類欄為空"))
        if not difficulty_raw:
            issues.append((source_row, "missing_difficulty", "難度欄為空"))

        effect = cell(rows, index, effect_col).strip()
        if not effect:
            issues.append((source_row, "missing_effect", "效果欄為空"))

        prereq_raw = cell(rows, index, 3) if has_prereq else ""

        records.append(
            {
                "id": feat_id(group, name),
                "name": name,
                "feat_group": group,
                "difficulty": difficulty,
                "difficulty_raw": difficulty_raw or None,
                "parent_id": None,
                "effect": effect,
                "categories": categories,
                "prereq_raw": prereq_raw,
                "source_sheet": sheet,
                "source_row": source_row,
            }
        )

    return records, issues


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


def parse_all(raw_dir=None):
    """跑完 Tier A 的全部解析，回傳 (資料, issues)。

    注意：此時 feat 紀錄帶的是尚未解析的 prereq_raw。前置條件要等勘誤套用
    完畢、而且所有專長都認識了之後才解析得動，因此由呼叫端在套用勘誤後
    自行呼叫 resolve_prereqs()。
    """
    feats, affixes, issues = [], [], []

    for sheet in FEAT_SHEETS:
        records, sheet_issues = parse_feat_sheet(sheet, raw_dir)
        feats.extend(records)
        issues.extend((sheet,) + i for i in sheet_issues)

    races, race_issues = parse_races(raw_dir)
    issues.extend(("種族與其調整",) + i for i in race_issues)

    for sheet in AFFIX_SHEETS:
        records, sheet_issues = parse_affix_sheet(sheet, raw_dir)
        affixes.extend(records)
        issues.extend((sheet,) + i for i in sheet_issues)

    return {"feats": feats, "races": races, "affixes": affixes}, issues


def resolve_prereqs(data: dict):
    """把每個專長的 prereq_raw 解析成結構化的前置條件。

    必須在勘誤套用之後才呼叫，否則修正過的前置文字不會生效。
    """
    feat_index = {f["name"]: f["id"] for f in data["feats"]}
    for record in data["feats"]:
        record["prereqs"] = parse_prereqs(record.pop("prereq_raw"), feat_index)
