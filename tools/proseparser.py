#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""D100 規則書 — Tier C 第二批：散文規則與小型對照表。

規則書裡有兩種不屬於「條目表」的東西：

1. **散文規則** —— 創角流程、戰鬥流程、世界觀、FATE 特規。版面是
   「標題欄 + 內容欄」，戰鬥流程還有兩層（大步驟底下再分動作類型）。
2. **小型對照表** —— 等級→CP、環數→價格、地形→額外法術。欄位各不相同，
   規則書裡有數十張，一張張建資料表並不划算，統一收進 ref_table。

兩者的區塊位置同樣由 data/layout/*.yaml 宣告，程式不做任何推斷。
"""

from __future__ import annotations

import json
import re

from rawio import cell, clean_name, read_sheet


def parse_prose(layout: dict, raw_dir=None):
    """解析散文型工作表。

    layout 的 levels 由外而內宣告每一層的「標題欄」與「內容欄」，例如
    戰鬥流程是：
        levels:
          - {name_col: 6, body_col: 7}   # 大步驟
          - {name_col: 7, body_col: 8}   # 步驟底下的動作類型
    判斷方式是由外層往內找：哪一層的標題欄有值，這一列就屬於那一層。
    """
    sheet = layout["sheet"]
    levels = layout["levels"]
    # 有些表用「只有標題、沒有內容」的列當群組標題（FATE 特規的
    # 「從者五圍能力」、Patch note 的版本號）。但同樣的版面在大陸簡史
    # 只是一列雜訊，所以由 layout 明說，不自作主張。
    promote_headerless = layout.get("promote_headerless", False)
    rows = read_sheet(sheet, raw_dir)
    row_start, row_end = layout.get("rows", [1, len(rows)])

    blocks, issues = [], []
    current = [None] * len(levels)
    group = None      # 只有標題沒有內容的列所開啟的群組
    order = 0

    for index in range(row_start - 1, min(row_end, len(rows))):
        source_row = index + 1
        row = rows[index] if index < len(rows) else []

        matched = None
        for depth, level in enumerate(levels):
            if cell(rows, index, level["name_col"] - 1).strip():
                matched = depth
                break

        if matched is None:
            # 沒有任何標題欄有值：這是上一段的續行（Patch note 的補述列），
            # 併回前一個段落而不是丟掉。
            trailing = ""
            for level in levels:
                trailing = cell(rows, index, level["body_col"] - 1).strip()
                if trailing:
                    break
            if trailing and blocks:
                blocks[-1]["body"] += "\n" + trailing
            elif any(c.strip() for c in row):
                issues.append(
                    (source_row, "unparsed_row", "這一列有內容但沒有任何層級的標題")
                )
            continue

        # 收下這一列所有有值的層級。但只認名稱欄與上一層內容欄不同的層級 ——
        # 戰鬥流程的第二層名稱欄就是第一層的內容欄，若不排除，
        # 大步驟的說明文字會被誤當成子項名稱。
        deepest = matched
        current[matched] = cell(rows, index, levels[matched]["name_col"] - 1).strip()
        for depth in range(matched + 1, len(levels)):
            if levels[depth]["name_col"] == levels[deepest]["body_col"]:
                break
            name = cell(rows, index, levels[depth]["name_col"] - 1).strip()
            if not name:
                break
            current[depth] = name
            deepest = depth
        for depth in range(deepest + 1, len(levels)):
            current[depth] = None

        body = cell(rows, index, levels[deepest]["body_col"] - 1).strip()

        if not body:
            # 只有標題沒有內容：這是一個群組標題（FATE 特規的「從者五圍能力」、
            # Patch note 的「1.2nerf」），底下的條目都掛在它下面。
            if promote_headerless and matched == 0:
                group = current[0]
            continue

        section = group or current[0]
        subsection = current[0] if group else (
            current[1] if len(current) > 1 else None
        )
        if group and len(current) > 1 and current[1]:
            subsection = f"{current[0]} / {current[1]}"

        blocks.append(
            {
                "sheet": sheet,
                "section": section,
                "subsection": subsection,
                "body": body,
                "sort_order": order,
                "source_row": source_row,
                "source_col": levels[deepest]["body_col"],
            }
        )
        order += 1

    return blocks, issues


def parse_prose_explicit(layout: dict, raw_dir=None):
    """標題與內容上下交錯、又擠在同一欄時，改由人明確宣告每一段的範圍。

    「施法者創角須知」就是這種版面：第 14 列是標題「總施法者等級」、
    第 15 列是內容，第 16 列又是標題。標題與內容的長度、標點都區分不開
    （「奧術知識（知識）難度2」比「法師應先決技能及應注意事項」還短），
    與其寫一個必定會誤判的啟發法，不如把列號寫清楚。
    """
    sheet = layout["sheet"]
    rows = read_sheet(sheet, raw_dir)
    blocks, issues = [], []

    for order, spec in enumerate(layout["blocks"]):
        col = spec["col"]
        title = cell(rows, spec["title_row"] - 1, col - 1).strip()
        if not title:
            issues.append(
                (spec["title_row"], "missing_section_title",
                 f"C{col} 第 {spec['title_row']} 列宣告為標題但該格是空的")
            )
            continue

        body_start, body_end = spec["rows"]
        lines = []
        for index in range(body_start - 1, min(body_end, len(rows))):
            text = cell(rows, index, col - 1).strip()
            if text:
                lines.append(text)

        if not lines:
            issues.append(
                (spec["title_row"], "empty_section",
                 f"章節「{title}」的內容範圍 {body_start}~{body_end} 是空的")
            )
            continue

        blocks.append(
            {
                "sheet": sheet,
                "section": spec.get("group") or title,
                "subsection": title if spec.get("group") else None,
                "body": "\n".join(lines),
                "sort_order": order,
                "source_row": spec["title_row"],
                "source_col": col,
            }
        )

    return blocks, issues


def parse_tables(layout: dict, raw_dir=None):
    """解析一張工作表上宣告的所有小型對照表。

    每張表宣告的內容：
        name        表名
        header_row  欄名所在的列（省略代表這張表沒有表頭，欄名自動編號）
        rows        資料列範圍（含頭含尾）
        cols        欄位範圍（含頭含尾）
        note_row    選用，表格上方的說明文字所在列
    """
    sheet = layout["sheet"]
    rows = read_sheet(sheet, raw_dir)
    tables, table_rows, issues = [], [], []

    for order, spec in enumerate(layout["tables"]):
        col_start, col_end = spec["cols"]
        row_start, row_end = spec["rows"]

        if "columns" in spec:
            # 表頭橫跨兩列、或第一格根本沒寫欄名時（魔法物品價格表的
            # 「加值」那一欄就是空的），直接由人把欄名寫清楚。
            columns = list(spec["columns"])
            anchor_row = spec.get("header_row", spec["rows"][0])
        elif "header_row" in spec:
            header_index = spec["header_row"] - 1
            columns = [
                cell(rows, header_index, col - 1).strip()
                for col in range(col_start, col_end + 1)
            ]
            anchor_row = spec["header_row"]
        else:
            columns = [f"欄{i}" for i in range(1, col_end - col_start + 2)]
            anchor_row = row_start

        note = None
        if "note_row" in spec:
            note = cell(rows, spec["note_row"] - 1, col_start - 1).strip() or None

        table_id = f"ref_table:{sheet}:{spec['name']}"
        tables.append(
            {
                "id": table_id,
                "sheet": sheet,
                "name": spec["name"],
                "note": note,
                "columns_json": json.dumps(columns, ensure_ascii=False),
                "sort_order": order,
                "source_row": anchor_row,
                "source_col": col_start,
            }
        )

        # 有些對照表是橫著排的（「詞綴條數量分布」的三個項目各佔一列，
        # 每一個欄位才是一筆資料），此時把列與欄對調再輸出。
        if spec.get("transpose"):
            grid = [
                [cell(rows, index, col - 1).strip()
                 for index in range(row_start - 1, min(row_end, len(rows)))]
                for col in range(col_start, col_end + 1)
            ]
        else:
            grid = [
                [cell(rows, index, col - 1).strip()
                 for col in range(col_start, col_end + 1)]
                for index in range(row_start - 1, min(row_end, len(rows)))
            ]

        emitted = 0
        for offset, cells in enumerate(grid):
            index = (col_start - 1 + offset) if spec.get("transpose") else (
                row_start - 1 + offset
            )
            if not any(cells):
                continue
            table_rows.append(
                {
                    "table_id": table_id,
                    "row_index": emitted,
                    "cells_json": json.dumps(cells, ensure_ascii=False),
                    "source_row": (row_start if spec.get("transpose")
                                   else index + 1),
                }
            )
            emitted += 1

        if emitted == 0:
            issues.append(
                (row_start, "empty_table", f"對照表「{spec['name']}」沒有抓到任何資料列")
            )

    return tables, table_rows, issues


_PLUS_LABEL = re.compile(r"^(?:加[一二三四五六七八九十百]+|N/A)$")


def parse_affix_distribution(layout: dict, aliases=None, raw_dir=None):
    """解析「詞墜(依物品分類)」的各部位詞綴分布。

    每個部位是一個區塊：一列表頭寫著「加一 加二 加三…」，每個加值等級
    橫跨兩欄，底下列出該等級可以擲出的詞綴。

    有兩處不規則，都在資料列裡：法杖與項鍊的欄位中途換了加值等級
    （例如「加八」直接寫在資料列中間），因此資料格若本身長得像加值標籤，
    就視為該欄從這一列起改用新的等級，而不是一條詞綴。
    """
    sheet = layout["sheet"]
    # 同一個寫錯的名稱會在表中出現很多次（「重拳」就有 5 處），逐列開勘誤
    # 並不實際，因此以名稱別名一次對正，對照與理由記在 data/errata。
    aliases = aliases or {}
    rows = read_sheet(sheet, raw_dir)
    records, issues = [], []

    for block in layout["blocks"]:
        slot = block["slot"]
        header_index = block["header_row"] - 1
        row_start, row_end = block["rows"]
        col_start, col_end = block["cols"]
        stride = block.get("stride", 2)

        # 表頭上每隔 stride 欄出現一個加值標籤，標籤管轄它自己與後面 stride-1 欄。
        labels = {}
        for col in range(col_start, col_end + 1, stride):
            label = cell(rows, header_index, col - 1).strip()
            if label:
                for offset in range(stride):
                    labels[col + offset] = label

        if not labels:
            issues.append(
                (block["header_row"], "missing_header",
                 f"部位「{slot}」的表頭列沒有任何加值標籤")
            )
            continue

        for index in range(row_start - 1, min(row_end, len(rows))):
            for col in range(col_start, col_end + 1):
                value = cell(rows, index, col - 1).strip()
                if not value:
                    continue
                if _PLUS_LABEL.match(value):
                    # 欄位中途換加值等級，從這一列起沿用新的標籤。
                    for offset in range(stride):
                        labels[col + offset] = value
                    continue
                label = labels.get(col)
                if label is None:
                    issues.append(
                        (index + 1, "unlabelled_affix",
                         f"部位「{slot}」第 {index + 1} 列第 {col} 欄的"
                         f"「{value}」找不到對應的加值等級")
                    )
                    continue
                canonical = clean_name(value)
                canonical = aliases.get(canonical, canonical)
                records.append(
                    {
                        "slot": slot,
                        "plus_label": label,
                        "affix_name": canonical,
                        "source_sheet": sheet,
                        "source_row": index + 1,
                        "source_col": col,
                    }
                )

    return records, issues
