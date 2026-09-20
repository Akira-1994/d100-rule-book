#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""D100 規則書 — data/raw 的讀取與正規化工具。

extract.py 之後的每一支工具都從這裡讀資料，確保反跳脫與全形正規化的
規則只有一份實作。
"""

from __future__ import annotations

import re
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
RAW_DIR = REPO_ROOT / "data" / "raw"

# 六大技能分類（創角色須知定義）
CATEGORIES = {
    "戰鬥": "DEX＋SKI＋STR",
    "運動": "DEX＋SKI＋CON",
    "操作": "INT＋SKI＋WIS",
    "感知": "INT＋RES＋SPI",
    "知識": "（INT＋WIS）×1.5",
    "交涉": "CHA＋WIS＋SPI",
}

# 九大屬性
ATTRIBUTES = {
    "STR": "力量",
    "DEX": "敏捷",
    "SKI": "技巧",
    "CON": "體質",
    "RES": "抗力",
    "INT": "智力",
    "WIS": "智慧",
    "CHA": "魅力",
    "SPI": "精神",
}

# 全形 → 半形。規則書中加減號、括號、數字全半形混用得很隨性。
_FULLWIDTH = {
    "－": "-", "＋": "+", "×": "*", "～": "~",
    "（": "(", "）": ")", "，": ",",
}
_FULLWIDTH_DIGITS = {chr(0xFF10 + i): str(i) for i in range(10)}
_FULLWIDTH.update(_FULLWIDTH_DIGITS)

_UNESCAPE = {"n": "\n", "t": "\t", "r": "\r", "\\": "\\"}


def unescape(field: str) -> str:
    """還原 extract.py 的跳脫。必須是 extract.format_cell 的精確反函式。"""
    out = []
    index = 0
    while index < len(field):
        char = field[index]
        if char == "\\" and index + 1 < len(field) and field[index + 1] in _UNESCAPE:
            out.append(_UNESCAPE[field[index + 1]])
            index += 2
            continue
        out.append(char)
        index += 1
    return "".join(out)


def normalize_row(cells: list) -> list:
    """空白列在 TSV 中寫成一行空字串，讀回來會是 [""]，還原成 []。"""
    return [] if cells == [""] else cells


def read_sheet(name: str, raw_dir: Path = None) -> list:
    """讀入一張工作表，回傳反跳脫後的二維字串陣列（列不補齊）。"""
    raw_dir = raw_dir or RAW_DIR
    path = raw_dir / f"{name}.tsv"
    if not path.is_file():
        raise FileNotFoundError(f"找不到工作表檔案：{path}")
    text = path.read_text(encoding="utf-8")
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    return [normalize_row([unescape(f) for f in line.split("\t")]) for line in lines]


def cell(rows: list, row_index: int, col_index: int) -> str:
    """取得指定儲存格（0-based），超出範圍回傳空字串。"""
    if row_index >= len(rows):
        return ""
    row = rows[row_index]
    return row[col_index] if col_index < len(row) else ""


def to_halfwidth(text: str) -> str:
    for full, half in _FULLWIDTH.items():
        text = text.replace(full, half)
    return text


def clean_name(text: str) -> str:
    """整理名稱：去頭尾空白、把內部換行壓成單一空白。

    種族表有 '神魔裔\\n（潘神之子）' 與 ' 原初-星之幼體' 這類寫法，
    換行與前導空白純屬排版，不該進到識別碼或顯示名稱裡。
    """
    # 零寬空白與 BOM 在儲存格裡看不見，卻會讓名稱比對與 id 產生失敗。
    # 實際踩到過：「​迷惑學院」與「​劍宗」名稱前都藏著一個 U+200B。
    for invisible in ("​", "﻿", "‎", "‏"):
        text = text.replace(invisible, "")
    text = text.replace(" ", " ")
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    parts = [p.strip() for p in text.split("\n") if p.strip()]
    return " ".join(parts)


def parse_int(text: str):
    """把可能含全形符號的字串解析成整數，失敗回傳 None。"""
    if text is None:
        return None
    candidate = to_halfwidth(text).strip()
    if not candidate:
        return None
    match = re.fullmatch(r"[+-]?\d+", candidate)
    return int(match.group()) if match else None


def parse_number(text: str):
    """同 parse_int，但允許小數。"""
    if text is None:
        return None
    candidate = to_halfwidth(text).strip()
    if not candidate:
        return None
    match = re.fullmatch(r"[+-]?\d+(?:\.\d+)?", candidate)
    return float(match.group()) if match else None


_CATEGORY_SPLIT = re.compile(r"[/、,，]|或")


def parse_categories(text: str):
    """把分類欄拆成正規化後的分類集合，並回報無法辨識的片段。

    原表寫法包括 '戰鬥'、'戰鬥/運動'、'操作/知識'、'知識、\\n感知或交涉'。
    以集合表達後，'感知/知識' 與 '知識/感知' 會收斂成同一組。

    回傳 (已知分類的排序清單, 無法辨識的片段清單)。
    """
    if not text or not text.strip():
        return [], []
    flattened = text.replace("\n", "、")
    known, unknown = [], []
    for part in _CATEGORY_SPLIT.split(flattened):
        part = part.strip()
        if not part:
            continue
        if part in CATEGORIES:
            if part not in known:
                known.append(part)
        else:
            unknown.append(part)
    known.sort(key=lambda c: list(CATEGORIES).index(c))
    return known, unknown


_SLOT_SPLIT = re.compile(r"[/、,，\n]")


def parse_slots(text: str):
    """把裝備部位欄拆成部位清單。"""
    if not text:
        return []
    slots = []
    for part in _SLOT_SPLIT.split(text):
        part = part.strip()
        if part and part not in slots:
            slots.append(part)
    return slots


_RANK_SPLIT = re.compile(r"[、,，]")


def parse_plus_costs(text: str):
    """解析詞綴的「所需加值兌換」欄。

    '1'          -> [(1, 1, None)]
    '2、4、6'     -> [(1, 2, None), (2, 4, None), (3, 6, None)]
    '1~6'        -> [(1, 1, None), ..., (6, 6, None)]
    '3(鎧甲、盾牌)\\n2(武器)' -> 條件式，帶 condition

    回傳 [(rank, plus_cost, condition)]；完全無法解析時回傳空清單。
    """
    if not text or not text.strip():
        return []
    normalized = to_halfwidth(text).strip()

    # 條件式：每一行形如 '3(鎧甲、盾牌)'。
    # 這些是「依裝備類型而異的同一階」，不是由弱到強的多個階級，
    # 因此 rank 一律為 1，差別記在 condition。
    if "(" in normalized and re.search(r"\d\s*\(", normalized):
        results = []
        for line in [l.strip() for l in normalized.split("\n") if l.strip()]:
            match = re.match(r"([+-]?\d+)\s*\((.+)\)", line)
            if not match:
                return []
            results.append((1, int(match.group(1)), match.group(2)))
        return results

    # 區間：'1~6'
    range_match = re.fullmatch(r"(\d+)\s*~\s*(\d+)", normalized)
    if range_match:
        low, high = int(range_match.group(1)), int(range_match.group(2))
        if low > high:
            return []
        return [(i - low + 1, i, None) for i in range(low, high + 1)]

    # 列舉：'2、4、6' 或單一數字
    parts = [p.strip() for p in _RANK_SPLIT.split(normalized) if p.strip()]
    results = []
    for rank, part in enumerate(parts, start=1):
        if not re.fullmatch(r"\d+", part):
            return []
        results.append((rank, int(part), None))
    return results


_ATTR_MOD = re.compile(
    r"\b(STR|DEX|SKI|CON|RES|INT|WIS|CHA|SPI)\s*([+-])\s*(\d+)", re.IGNORECASE
)


def parse_attr_modifiers(text: str):
    """從種族的「數值調正」欄抽出屬性增減。

    例：'DEX＋2、CON－1' -> {'DEX': 2, 'CON': -1}
    抓不到的部分留給 attr_text 原文保存，不臆測。
    """
    if not text:
        return {}
    normalized = to_halfwidth(text)
    modifiers = {}
    for match in _ATTR_MOD.finditer(normalized):
        attr = match.group(1).upper()
        delta = int(match.group(3))
        if match.group(2) == "-":
            delta = -delta
        modifiers[attr] = modifiers.get(attr, 0) + delta
    return modifiers
