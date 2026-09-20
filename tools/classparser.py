#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""D100 規則書 — Phase 2 Tier C 職業表解析器。

Tier C 的職業表是排版而不是資料：同一張工作表上並排了好幾張卡片，
沒有可靠的表頭，欄位位置各表不同。與其讓程式去猜，不如由人看過一次、
把區塊位置寫進 data/layout/<工作表>.yaml，程式只負責機械地切割與擷取。

每張表的結構都是「章節標題 + 條目」交錯：
    防護                          <- 只有第一欄有值，是章節（學派／血脈／領域）
    奧術防禦（知識） 1 你可以…      <- 難度欄有值，是條目
章節標題後面常跟著一段說明文字，同樣只有第一欄有值。區分方式是長度：
標題短、說明長。這個啟發法在八張表上都成立，但仍由 validate.py 覆核。
"""

from __future__ import annotations

import re

from rawio import (
    CATEGORIES,
    clean_name,
    parse_categories,
    parse_number,
    read_sheet,
    to_halfwidth,
)

# 章節標題的長度上限。名稱常帶英文原名（「生命領域 Life Domain」），
# 所以不能訂得太緊；真正的說明文字都遠長於此，另以標點再過濾一次。
SECTION_NAME_MAX = 24

# 出現句讀就不是標題而是敘述。「知識之神----Oghma, Boccob,」這種
# 長度雖短卻明顯是說明的列，靠逗號擋掉。
_SENTENCE_MARKS = "。，、；：！？,;"


def split_section_name(text: str):
    """判斷一個單欄列是不是章節標題，是的話拆出 (名稱, 附帶說明)。

    章節名偶爾會把條件註記折到第二行，例如
        至高妖精-少女的超凡型態
        (你的邪術師環數須大於你的進階契約…)
    所以只用第一行判斷，其餘行當成該章節的說明。

    回傳 (None, None) 代表這不是標題而是敘述。
    """
    lines = [line for line in text.split("\n") if line.strip()]
    if not lines:
        return None, None
    name = clean_name(lines[0])
    if not name or len(name) > SECTION_NAME_MAX:
        return None, None
    if any(mark in name for mark in _SENTENCE_MARKS):
        return None, None
    return name, "\n".join(lines[1:]).strip() or None

_DIFFICULTY = re.compile(r"^(?:難度)?\s*([0-9]+(?:\.[0-9]+)?)\s*$")


def split_difficulty_cell(text: str):
    """難度欄偶爾會把前置註記折到第二行，例如「難度4\\n（前置法術瞬唱）」。

    回傳 (難度字串, 附帶的前置文字)。
    """
    lines = [line.strip() for line in text.split("\n") if line.strip()]
    if not lines:
        return "", ""
    return lines[0], "\n".join(lines[1:])
# 名稱尾端的分類註記，例如「奧術防禦（知識）」
_CATEGORY_IN_NAME = re.compile(r"^(?P<name>.+?)\s*[（(](?P<cat>[^（()）]+)[）)]\s*$")
_WRAPPED = re.compile(r"^[（(]([^（()）]+)[）)]$")


def looks_like_difficulty(text: str) -> bool:
    head, _rest = split_difficulty_cell(text)
    return bool(_DIFFICULTY.match(to_halfwidth(head)))


def parse_difficulty(text: str):
    match = _DIFFICULTY.match(to_halfwidth(text).strip())
    return float(match.group(1)) if match else parse_number(text)


def split_category_from_name(name: str):
    """把「奧術防禦（知識）」拆成 ('奧術防禦', ['知識'])。

    括號內不是已知分類時（例如「專攻武器（咒火）」）就原樣保留名稱，
    不要把武器種類誤當成技能分類。
    """
    # 整串就是一個括號時（分類自己折成一行的「（感知）」），名稱部分為空。
    wrapped = _WRAPPED.match(name.strip())
    if wrapped:
        categories, unknown = parse_categories(wrapped.group(1))
        return ("", categories) if categories and not unknown else (name, [])

    match = _CATEGORY_IN_NAME.match(name)
    if not match:
        return name, []
    categories, unknown = parse_categories(match.group("cat"))
    if categories and not unknown:
        return match.group("name").strip(), categories
    return name, []


def slice_block(row: list, start_col: int, width: int) -> list:
    """取出某一列在某個區塊範圍內的儲存格（start_col 為 1-based）。"""
    begin = start_col - 1
    return [
        (row[i].strip() if i < len(row) else "")
        for i in range(begin, begin + width)
    ]


def parse_layout(layout: dict, raw_dir=None):
    """依版面宣告解析一張職業表，回傳 (流派, 專長, issues)。"""
    sheet = layout["sheet"]
    class_name = layout["class_name"]
    path_kind = layout["path_kind"]
    category_source = layout.get("category_source", "column")

    banners = set(layout.get("banner_sections") or [])

    rows = read_sheet(sheet, raw_dir)
    paths, feats, traits, notes, issues = [], [], [], [], []

    for region in layout["regions"]:
        row_start, row_end = region.get("rows", [1, len(rows)])
        # 區塊之間的欄位配置可能不同（特殊職業2 的三個職業就是），
        # 因此分類來源與跳過列都允許逐區域覆寫。
        region_category_source = region.get("category_source", category_source)
        skip_rows = set()
        for item in region.get("skip_rows", []) or []:
            if isinstance(item, list):
                skip_rows.update(range(item[0], item[1] + 1))
            else:
                skip_rows.add(item)
        columns = region["columns"]
        extended = region.get("columns_extended")
        width = len(extended or columns)
        diff_index = columns.index("difficulty")

        for block in region["blocks"]:
            # 區塊可以只寫欄號，也可以寫成 {col: 1, class_name: 賢者} ——
            # 「特殊職業1」這種一張表並排三個不同職業的情況需要後者。
            if isinstance(block, dict):
                block_col = block["col"]
                block_class = block.get("class_name", class_name)
                block_kind = block.get("path_kind", path_kind)
            else:
                block_col, block_class, block_kind = block, class_name, path_kind

            pending_headers = []
            current_path = None

            def flush(order):
                """把累積的標題轉成流派（或橫幅註記），回傳新的 current_path。"""
                nonlocal pending_headers
                if not pending_headers:
                    return current_path
                new_paths, new_notes, header_issues = _flush_headers(
                    pending_headers, sheet, block_class, block_kind,
                    block_col, order, banners,
                )
                issues.extend(header_issues)
                pending_headers = []
                paths.extend(new_paths)
                notes.extend(new_notes)
                # 最後一個章節才是接下來條目的歸屬。
                return new_paths[-1] if new_paths else current_path

            for index in range(row_start - 1, min(row_end, len(rows))):
                source_row = index + 1
                if source_row in skip_rows:
                    continue
                cells = slice_block(rows[index], block_col, width)
                if not any(cells):
                    continue

                is_entry = diff_index < len(cells) and looks_like_difficulty(
                    cells[diff_index]
                )

                if not is_entry:
                    if cells[0] and not any(cells[1:]):
                        pending_headers.append((source_row, cells[0]))
                    elif cells[0] and len([c for c in cells[1:] if c]) == 1:
                        # 名稱 + 一段敘述、沒有難度：職業的被動特性或招式，
                        # 不是可以花 CP 學的技能。敘述不一定緊鄰名稱
                        # （如來神掌的九式就是名稱在 C17、敘述在 C19）。
                        current_path = flush(len(paths))
                        description = next(c for c in cells[1:] if c)
                        traits.append(
                            {
                                "id": f"class_trait:{block_class}:"
                                      f"{clean_name(cells[0])}",
                                "class_path_id":
                                    current_path["id"] if current_path else None,
                                "name": clean_name(cells[0]),
                                "description": description,
                                "source_sheet": sheet,
                                "source_row": source_row,
                                "source_col": block_col,
                            }
                        )
                    elif any(cells[1:]):
                        issues.append(
                            (source_row,
                             "unparsed_row",
                             f"C{block_col} 區塊第 {source_row} 列有內容但不像條目："
                             f"{cells[0][:30]!r}")
                        )
                    continue

                current_path = flush(len(paths))

                feat, feat_issues = _build_feat(
                    cells, columns, extended, source_row, block_col, sheet,
                    block_class, current_path, region_category_source,
                )
                issues.extend(feat_issues)
                feats.append(feat)

            # 區塊結尾若還剩下未消化的標題，那是沒有任何條目的章節。
            flush(len(paths))

    return paths, feats, traits, notes, issues


def _flush_headers(headers, sheet, class_name, path_kind, block_col, order,
                   banners):
    """把累積的單欄列轉成一個流派，或轉成一段規則註記。

    名稱取「最後一個夠短的列」：短列是標題，長列是說明或前言。
    標題之後的列是這個流派的說明，之前的列則是上一段的殘留文字。

    banner_sections 裡宣告的標題（例如牧師領域表上並排兩次的「神祇領域」）
    是版面橫幅而不是真正的流派，改收進 rule_text。

    一組連續的單欄列裡可能同時有橫幅、真正的章節名與說明，例如牧師領域
    C10 區塊的開頭就是「神祇領域」（橫幅）、選擇規則（說明）、
    「生命領域 Life Domain」（真章節）、領域介紹（說明）四列。
    因此逐列判斷而不是整組只取一個名稱。

    回傳 (流派清單, 註記清單, issues)。
    """
    issues = []
    paths, notes = [], []
    pending_description = []
    current = None

    def close():
        if current is not None:
            current["description"] = "\n".join(pending_description) or None

    for source_row, text in headers:
        name, inline_note = split_section_name(text)
        if name:
            close()
            pending_description = [inline_note] if inline_note else []
            if name in banners:
                current = None
                notes.append(
                    {
                        "sheet": sheet,
                        "source_row": source_row,
                        "source_col": block_col,
                        "body": name,
                        "_collect": pending_description,
                    }
                )
                continue
            current = {
                "id": f"class_path:{class_name}:{name}",
                "class_name": class_name,
                "path_kind": path_kind,
                "name": name,
                "description": None,
                "sort_order": order + len(paths),
                "source_sheet": sheet,
                "source_row": source_row,
                "source_col": block_col,
            }
            paths.append(current)
        else:
            if current is None and notes:
                notes[-1]["body"] += "\n" + text
            elif current is None:
                # 章節開始之前的文字是整張表的前言，收進 rule_text。
                notes.append(
                    {
                        "sheet": sheet,
                        "source_row": source_row,
                        "source_col": block_col,
                        "body": text,
                    }
                )
            else:
                pending_description.append(text)
    close()

    for note in notes:
        note.pop("_collect", None)

    return paths, notes, issues


def _build_feat(cells, columns, extended, source_row, source_col, sheet,
                class_name, current_path, category_source):
    """把一列儲存格組成一個專長紀錄。"""
    issues = []

    # 加長版面只在多出來的那一欄真的有值時採用。
    layout_columns = columns
    if extended and len(cells) > len(columns) and cells[len(columns)]:
        layout_columns = extended

    values = dict(zip(layout_columns, cells))
    name_raw = values.get("name", "")
    name = clean_name(name_raw)
    categories = []

    # 分類註記寫在名稱第一行的尾端，但名稱底下可能還折了一行副標，
    # 例如「飛翔（感知）\n【咒火戰鬥機】」。先只看第一行取分類，
    # 再把剩下的行接回名稱。
    name_lines = [line for line in name_raw.split("\n") if line.strip()]
    head, tail = (name_lines[0], name_lines[1:]) if name_lines else ("", [])

    if category_source == "column":
        categories, unknown = parse_categories(values.get("category", ""))
        if unknown:
            issues.append(
                (source_row, "category_unrecognized", f"無法辨識的分類：{unknown}")
            )
        if not categories:
            issues.append((source_row, "missing_category", f"「{name}」分類欄為空"))
    elif category_source == "name":
        # 分類註記可能在第一行尾端（「飛翔（感知）」＋副標），
        # 也可能自己佔一行掛在名稱後面（「專攻武器（咒火）」＋「（感知）」）。
        # 兩種都試，剝掉帶分類的那一行、其餘接回名稱。
        stripped_head, categories = split_category_from_name(clean_name(head))
        if categories:
            name = clean_name(" ".join([stripped_head] + tail))
        elif tail:
            stripped_tail, categories = split_category_from_name(clean_name(tail[-1]))
            if categories:
                name = clean_name(
                    " ".join([head] + tail[:-1] + ([stripped_tail] if stripped_tail else []))
                )
        if not categories:
            issues.append(
                (source_row, "missing_category", f"「{name}」名稱沒有分類註記")
            )

    effect = values.get("effect", "").strip()
    if not effect:
        issues.append((source_row, "missing_effect", f"「{name}」效果欄為空"))

    difficulty_raw, inline_prereq = split_difficulty_cell(
        values.get("difficulty", "")
    )
    prereq_raw = values.get("prereq", "")
    if inline_prereq:
        prereq_raw = "\n".join(filter(None, [prereq_raw, inline_prereq]))
    path_name = current_path["name"] if current_path else "未分類"

    if current_path is None:
        issues.append(
            (source_row, "orphan_entry", f"「{name}」前面沒有章節標題，無法歸屬流派")
        )

    return (
        {
            "id": f"feat:class:{class_name}:{path_name}:{name}",
            "name": name,
            "feat_group": "class",
            "class_path_id": current_path["id"] if current_path else None,
            "difficulty": parse_difficulty(difficulty_raw),
            "difficulty_raw": difficulty_raw or None,
            "difficulty_scale": "cp",
            "parent_id": None,
            "effect": effect,
            "categories": categories,
            "tags": [],
            "prereq_raw": prereq_raw,
            "source_sheet": sheet,
            "source_row": source_row,
            "source_col": source_col,
        },
        issues,
    )


def parse_invocations(layout: dict, raw_dir=None):
    """解析 Warlock 的魔能祈喚清單。

    祈喚不是可以升級的技能，而是達到環數門檻後取得的固定效果，
    消耗的是「祈喚欄位」而不是 CP，因此不進 feat 表。
    """
    sheet = layout["sheet"]
    cols = layout["cols"]
    rows = read_sheet(sheet, raw_dir)
    row_start, row_end = layout["rows"]

    records, issues = [], []
    seen = {}

    for index in range(row_start - 1, min(row_end, len(rows))):
        source_row = index + 1
        name = clean_name(_at(rows, index, cols["name"]))
        if not name:
            continue

        cost_raw = _at(rows, index, cols["cost"]).strip()
        cost = parse_number(cost_raw)
        if cost_raw and cost is None:
            issues.append(
                (source_row, "cost_not_numeric", f"「{name}」的消耗欄為 {cost_raw!r}")
            )

        effect = _at(rows, index, cols["effect"]).strip()
        if not effect:
            issues.append((source_row, "missing_effect", f"祈喚「{name}」效果欄為空"))

        if name in seen:
            issues.append(
                (source_row, "duplicate_invocation",
                 f"祈喚「{name}」與第 {seen[name]} 列重複")
            )
            continue
        seen[name] = source_row

        records.append(
            {
                "id": f"invocation:{name}",
                "name": name,
                "cost": int(cost) if cost is not None else None,
                "cost_raw": cost_raw or None,
                "prereq_raw": _at(rows, index, cols["prereq"]).strip() or None,
                "effect": effect,
                "source_sheet": sheet,
                "source_row": source_row,
                "source_col": cols["name"],
            }
        )

    return records, issues


def _at(rows, row_index, col_1based):
    from rawio import cell as _cell
    return _cell(rows, row_index, col_1based - 1)
