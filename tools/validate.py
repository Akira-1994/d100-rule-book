#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""D100 規則書 — 資料庫驗證器。

在 build_db.py 之後跑。分兩種結果：
  ERROR — 資料庫本身壞了（外鍵、重複主鍵、分類不在白名單），建置不該出貨
  WARN  — 原始資料有問題但我們刻意不擋（缺效果、難度非數值、部位沒見過）

另外以「法師範例」那張實際角色卡回歸驗證 rules.py 的公式，
確保我們對規則的理解沒有跑掉。

用法
----
    python tools/validate.py
    python tools/validate.py --strict     # 把 WARN 也當成失敗
"""

from __future__ import annotations

import argparse
import sqlite3
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import yaml  # noqa: E402

import rules  # noqa: E402
from rawio import ATTRIBUTES, CATEGORIES, cell, read_sheet  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DB = REPO_ROOT / "dist" / "d100.db"

# 目前資料中出現過的裝備部位。新增值會觸發警告，強迫我們看一眼是不是錯字。
KNOWN_SLOTS = {
    "武器", "魔導槍械", "項鍊", "法杖", "鎧甲", "戒指", "頭環", "手環",
    "披風", "盾牌", "手套", "鞋子", "腰帶", "主要裝備", "武器(弓)", "全",
}


def sheets_without_category_source() -> set:
    """讀版面宣告，找出整張都沒有分類來源的工作表。"""
    layout_dir = REPO_ROOT / "data" / "layout"
    if not layout_dir.is_dir():
        return set()
    sheets = set()
    for path in sorted(layout_dir.glob("*.yaml")):
        layout = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        if layout.get("category_source") == "none":
            sheets.add(layout["sheet"])
    return sheets


def resolved_rows(connection) -> set:
    """作者已經裁示過的列。

    這些資料本身仍然不是數值（「1or2」「劇情取得」），驗證器還是得指出來，
    但它們已經有結論了，不該和尚未處理的問題混在同一張清單上。
    """
    return {
        (sheet, row)
        for sheet, row in connection.execute(
            "SELECT sheet, source_row FROM errata"
            " WHERE action = 'flag' AND issue LIKE 'resolved_%'"
        )
    }


class Report:
    def __init__(self):
        self.errors = []
        self.warnings = []
        self.notes = []
        self.checks = 0
        self.resolved = set()

    def note(self, message: str):
        self.notes.append(message)

    def warn_unless_resolved(self, sheet, row, message: str):
        """已裁示的列降級為註記，其餘照常警告。"""
        if (sheet, row) in self.resolved:
            self.notes.append(message)
        else:
            self.warnings.append(message)

    def check(self, ok: bool, message: str, fatal: bool = True):
        self.checks += 1
        if ok:
            return
        (self.errors if fatal else self.warnings).append(message)

    def error(self, message: str):
        self.errors.append(message)

    def warn(self, message: str):
        self.warnings.append(message)


def validate_schema(connection, report: Report):
    violations = connection.execute("PRAGMA foreign_key_check").fetchall()
    report.check(not violations, f"外鍵違規 {len(violations)} 筆：{violations[:3]}")

    integrity = connection.execute("PRAGMA integrity_check").fetchone()[0]
    report.check(integrity == "ok", f"資料庫完整性檢查失敗：{integrity}")


def validate_feats(connection, report: Report):
    rows = connection.execute(
        "SELECT id, name, feat_group, difficulty, difficulty_raw, source_sheet, source_row"
        " FROM feat"
    ).fetchall()
    report.check(bool(rows), "feat 表是空的")

    seen = {}
    for feat_id, name, group, difficulty, difficulty_raw, sheet, row in rows:
        key = (group, name)
        if key in seen:
            report.error(
                f"專長重複：{group} 的「{name}」同時出現在 {sheet} 第 {seen[key]} 列與第 {row} 列"
            )
        seen[key] = row

        if difficulty is not None and difficulty <= 0:
            report.error(f"{sheet} 第 {row} 列「{name}」難度為 {difficulty}，應為正數")
        if difficulty is None and difficulty_raw:
            report.warn_unless_resolved(
                sheet, row,
                f"{sheet} 第 {row} 列「{name}」難度 {difficulty_raw!r} 非數值，"
                "CP 成本需由 DM 裁定",
            )

    # 每個專長都應該至少有一個分類，否則角色卡無從歸類。
    # 但有些工作表整張都沒有分類欄（術士），那是規則書本身的缺口而不是
    # 逐列的錯誤，逐條列出只會把真正的問題淹掉，改成一條彙總警告。
    no_category_sheets = sheets_without_category_source()
    orphans = connection.execute(
        "SELECT f.source_sheet, f.source_row, f.name FROM feat f"
        " WHERE NOT EXISTS (SELECT 1 FROM feat_category c WHERE c.feat_id = f.id)"
        " ORDER BY f.source_sheet, f.source_row"
    ).fetchall()

    bulk = {}
    for sheet, row, name in orphans:
        if sheet in no_category_sheets:
            bulk[sheet] = bulk.get(sheet, 0) + 1
        else:
            report.warn(f"{sheet} 第 {row} 列「{name}」沒有任何分類")
    for sheet, count in sorted(bulk.items()):
        report.warn(
            f"{sheet} 整張表沒有分類欄，{count} 個條目無法歸類"
            "（見 docs/規則裁示紀錄.md）"
        )

    unknown = connection.execute(
        "SELECT DISTINCT category FROM feat_category"
        " WHERE category NOT IN (SELECT code FROM category)"
    ).fetchall()
    report.check(not unknown, f"出現不在白名單的分類：{[u[0] for u in unknown]}")


def validate_prereq_graph(connection, report: Report):
    """前置條件不能成環，否則角色永遠學不到。"""
    edges = {}
    names = {}
    for feat_id, name in connection.execute("SELECT id, name FROM feat"):
        edges[feat_id] = []
        names[feat_id] = name
    for feat_id, ref in connection.execute(
        "SELECT feat_id, ref_feat_id FROM feat_prereq WHERE ref_feat_id IS NOT NULL"
    ):
        edges[feat_id].append(ref)
    for feat_id, parent in connection.execute(
        "SELECT id, parent_id FROM feat WHERE parent_id IS NOT NULL"
    ):
        edges[feat_id].append(parent)

    WHITE, GREY, BLACK = 0, 1, 2
    color = dict.fromkeys(edges, WHITE)

    def visit(node, trail):
        if color[node] == GREY:
            cycle = trail[trail.index(node):] + [node]
            report.error("前置條件成環：" + " → ".join(names[n] for n in cycle))
            return
        if color[node] == BLACK:
            return
        color[node] = GREY
        for neighbour in edges[node]:
            visit(neighbour, trail + [node])
        color[node] = BLACK

    for node in edges:
        if color[node] == WHITE:
            visit(node, [])

    report.checks += 1


def validate_affixes(connection, report: Report):
    unknown = connection.execute(
        "SELECT DISTINCT slot FROM affix_slot"
    ).fetchall()
    for (slot,) in unknown:
        if slot not in KNOWN_SLOTS:
            report.warn(f"出現未見過的裝備部位 {slot!r}，請確認不是錯字或新制")

    for affix_id, name, sheet, row in connection.execute(
        "SELECT a.id, a.name, a.source_sheet, a.source_row FROM affix a"
        " WHERE NOT EXISTS (SELECT 1 FROM affix_slot s WHERE s.affix_id = a.id)"
    ):
        report.warn(f"{sheet} 第 {row} 列詞綴「{name}」沒有任何部位")

    bad = connection.execute(
        "SELECT affix_id, rank, plus_cost FROM affix_rank WHERE plus_cost <= 0"
    ).fetchall()
    report.check(not bad, f"詞綴兌換加值應為正數，異常 {len(bad)} 筆：{bad[:3]}")

    # 階級越高、所需加值應該越貴
    previous = {}
    for affix_id, rank, cost, condition in connection.execute(
        "SELECT affix_id, rank, plus_cost, condition FROM affix_rank"
        " ORDER BY affix_id, condition, rank"
    ):
        key = (affix_id, condition)
        if key in previous and cost <= previous[key]:
            name = connection.execute(
                "SELECT name FROM affix WHERE id = ?", (affix_id,)
            ).fetchone()[0]
            report.warn(
                f"詞綴「{name}」第 {rank} 階所需加值 {cost} 未高於前一階 {previous[key]}"
            )
        previous[key] = cost
    report.checks += 1


def validate_materials(connection, report: Report):
    """素材詞綴的機率區間必須不重不漏地蓋滿 1~100。

    附素材詞綴時是擲一次 D100 查表，所以區間若有重疊就會出現「擲到 71
    同時符合兩條」的歧義，有斷層則會擲出查不到東西的結果。
    """
    by_material = {}
    for material_id, name, rank, low, high, row in connection.execute(
        "SELECT a.material_id, m.name, a.tier_rank, a.roll_min, a.roll_max,"
        "       a.source_row"
        " FROM material_affix a JOIN material m ON m.id = a.material_id"
        " ORDER BY a.material_id, a.roll_min, a.tier_rank"
    ):
        by_material.setdefault((material_id, name), []).append((low, high, rank, row))

    for (_material_id, name), spans in by_material.items():
        # 同一階的多條詞綴共用區間（武器用／防具用），比對時只取一次。
        unique = sorted({(low, high) for low, high, _rank, _row in spans})

        if unique[0][0] != 1:
            report.warn(f"素材「{name}」的機率區間從 {unique[0][0]} 開始，未涵蓋 1")
        if unique[-1][1] != 100:
            report.warn(f"素材「{name}」的機率區間到 {unique[-1][1]} 為止，未涵蓋 100")

        for (low_a, high_a), (low_b, high_b) in zip(unique, unique[1:]):
            if low_b <= high_a:
                report.error(
                    f"素材「{name}」的機率區間重疊：{low_a}~{high_a} 與 {low_b}~{high_b}"
                    f"（擲出 {low_b}~{high_a} 時會同時符合兩條詞綴）"
                )
            elif low_b != high_a + 1:
                report.warn(
                    f"素材「{name}」的機率區間有斷層：{high_a} 與 {low_b} 之間"
                    f"（擲出 {high_a + 1}~{low_b - 1} 時查不到詞綴）"
                )

    bad = connection.execute(
        "SELECT id FROM material_affix WHERE roll_min < 1 OR roll_max > 100"
        " OR roll_min > roll_max"
    ).fetchall()
    report.check(not bad, f"機率區間超出 1~100 或起迄顛倒：{len(bad)} 筆")

    for name, sheet, row in connection.execute(
        "SELECT m.name, m.source_sheet, m.source_row FROM material m"
        " WHERE NOT EXISTS (SELECT 1 FROM material_affix a WHERE a.material_id = m.id)"
    ):
        report.warn(f"{sheet} 第 {row} 列素材「{name}」沒有任何詞綴")

    for (slot,) in connection.execute("SELECT DISTINCT slot FROM material_slot"):
        if slot not in KNOWN_SLOTS:
            report.warn(f"素材部位出現未見過的 {slot!r}，請確認不是錯字或新制")

    report.checks += 1


def validate_races(connection, report: Report):
    unknown = connection.execute(
        "SELECT DISTINCT attr FROM race_attr_modifier"
        " WHERE attr NOT IN (SELECT code FROM attribute)"
    ).fetchall()
    report.check(not unknown, f"種族屬性調整出現未知屬性：{[u[0] for u in unknown]}")

    for name, cp_raw, sheet, row in connection.execute(
        "SELECT name, cp_raw, source_sheet, source_row FROM race WHERE cp_cost IS NULL"
    ):
        report.warn_unless_resolved(
            sheet, row,
            f"{sheet} 第 {row} 列種族「{name}」的 CP 調整 {cp_raw!r} 非數值，"
            "需由 DM 建卡時手填",
        )

    positive = connection.execute(
        "SELECT name, cp_cost FROM race WHERE cp_cost > 0"
    ).fetchall()
    for name, cost in positive:
        report.warn(f"種族「{name}」的 CP 調整為正數 {cost}，原表慣例是扣除（負數）")
    report.checks += 1


def validate_reference_data(connection, report: Report):
    """對照表、散文段落與祈喚的一致性。"""
    import json

    # 每一列的欄位數必須與表頭一致，否則應用端渲染會錯位。
    for table_id, name, columns_json in connection.execute(
        "SELECT id, name, columns_json FROM ref_table"
    ):
        columns = json.loads(columns_json)
        rows = connection.execute(
            "SELECT row_index, cells_json, source_row FROM ref_row"
            " WHERE table_id = ? ORDER BY row_index",
            (table_id,),
        ).fetchall()
        if not rows:
            report.warn(f"對照表「{name}」沒有任何資料列")
            continue
        for _index, cells_json, source_row in rows:
            cells = json.loads(cells_json)
            if len(cells) != len(columns):
                report.error(
                    f"對照表「{name}」第 {source_row} 列有 {len(cells)} 格，"
                    f"表頭卻是 {len(columns)} 欄"
                )
                break
    report.checks += 1

    empty = connection.execute(
        "SELECT count(*) FROM rule_text WHERE trim(body) = ''"
    ).fetchone()[0]
    report.check(empty == 0, f"rule_text 有 {empty} 段是空白內容")

    bad_cost = connection.execute(
        "SELECT name, cost_raw FROM invocation WHERE cost IS NULL OR cost <= 0"
    ).fetchall()
    for name, cost_raw in bad_cost:
        report.warn(f"祈喚「{name}」的消耗欄 {cost_raw!r} 不是正整數")

    duplicates = connection.execute(
        "SELECT name, count(*) FROM invocation GROUP BY name HAVING count(*) > 1"
    ).fetchall()
    report.check(
        not duplicates, f"祈喚名稱重複：{[d[0] for d in duplicates]}"
    )

    no_prereq = connection.execute(
        "SELECT count(*) FROM invocation WHERE prereq_raw IS NULL"
    ).fetchone()[0]
    if no_prereq:
        report.warn(f"有 {no_prereq} 條祈喚沒有填寫前置條件")

    # 混沌石分布表引用的詞綴應該都能在詞綴表裡找到。對不起來多半是錯字，
    # 也可能是分布表寫的是一整類效果（「CHA 增加」）而非單一詞綴名。
    # 比對時忽略空白：分布表寫「RES增加」、詞綴表寫「RES 增加」，
    # 那是排版差異而不是兩條不同的詞綴。
    known = {
        name.replace(" ", "")
        for (name,) in connection.execute("SELECT name FROM affix")
    }
    unknown_affixes = {}
    for name, count in connection.execute(
        "SELECT affix_name, count(*) FROM affix_distribution"
        " GROUP BY affix_name ORDER BY 2 DESC"
    ):
        if name.replace(" ", "") not in known:
            unknown_affixes[name] = count

    if unknown_affixes:
        total = sum(unknown_affixes.values())
        listed = "、".join(
            f"{name}×{count}" for name, count in list(unknown_affixes.items())[:8]
        )
        report.warn(
            f"詞綴分布表引用了 {len(unknown_affixes)} 個詞綴表裡查不到的名稱"
            f"（共 {total} 處）：{listed}"
        )
    report.checks += 1


def validate_rule_formulas(report: Report):
    """拿「法師範例」那張實際角色卡回歸驗證 rules.py。

    這是唯一能確認我們對規則的理解沒跑掉的辦法 —— 規則書沒有測試，
    但它附了一張算好的角色卡。
    """
    rows = read_sheet("法師範例")

    scores = {}
    for index in range(2, 11):  # C3:C11 是九大屬性
        code = cell(rows, index, 0).strip()
        raw = cell(rows, index, 2).strip()
        if code in ATTRIBUTES and raw:
            scores[code] = int(float(raw))

    report.check(
        len(scores) == 9, f"法師範例只讀到 {len(scores)} 個屬性，應為 9 個"
    )
    if len(scores) != 9:
        return

    # 表上的調整值（D 欄）應該和我們算的一致
    for index in range(2, 11):
        code = cell(rows, index, 0).strip()
        expected = cell(rows, index, 3).strip()
        if code in ATTRIBUTES and expected:
            actual = rules.attribute_modifier(scores[code])
            report.check(
                actual == int(float(expected)),
                f"法師範例 {code}={scores[code]} 的調整值：表上 {expected}，算出 {actual}",
            )

    # 六大技能（E/H 欄）與抗性（I/L 欄）、特殊（E/H 欄下半）
    sheet_skills = {}
    for index in range(1, 12):
        label = cell(rows, index, 4).strip()
        value = cell(rows, index, 7).strip()
        if label and value:
            sheet_skills[label] = value
    for index in range(1, 7):
        label = cell(rows, index, 8).strip()
        value = cell(rows, index, 11).strip()
        if label and value:
            sheet_skills[label] = value

    for label, expected_raw in sheet_skills.items():
        try:
            expected = int(float(expected_raw))
        except ValueError:
            continue
        if label in rules.SKILL_FORMULAS or label == "知識":
            actual = rules.skill_value(scores, label)
        elif label in rules.RESIST_FORMULAS:
            actual = rules.resist_value(scores, label)
        elif label in rules.SPECIAL_FORMULAS:
            actual = rules.special_value(scores, label)
        else:
            continue
        report.check(
            actual == expected,
            f"法師範例「{label}」：表上 {expected}，rules.py 算出 {actual}",
            fatal=False,
        )

    # 創角色須知的 CP 範例：難度 1 的技能學到等級 3 應為 14 點
    report.check(
        rules.cp_cumulative(3, 1) == 14,
        f"CP 公式回歸失敗：難度1學到等級3應為 14，算出 {rules.cp_cumulative(3, 1)}",
    )
    report.check(
        rules.cp_cumulative(3, 1) * 1 == 14 and rules.cp_for_level(5, 1) == 32,
        "CP 公式回歸失敗：難度1的等級5單級應為 32",
    )


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description="驗證 D100 規則書資料庫。")
    parser.add_argument("--db", default=str(DEFAULT_DB))
    parser.add_argument("--strict", action="store_true", help="把警告也視為失敗")
    args = parser.parse_args(argv)

    db_path = Path(args.db)
    if not db_path.is_file():
        raise SystemExit(f"找不到資料庫：{db_path}，請先執行 python tools/build_db.py")

    connection = sqlite3.connect(db_path)
    connection.execute("PRAGMA foreign_keys = ON")
    report = Report()
    report.resolved = resolved_rows(connection)

    validate_schema(connection, report)
    validate_feats(connection, report)
    validate_prereq_graph(connection, report)
    validate_affixes(connection, report)
    validate_materials(connection, report)
    validate_races(connection, report)
    validate_reference_data(connection, report)
    validate_rule_formulas(report)
    connection.close()

    if report.errors:
        print(f"✗ 錯誤 {len(report.errors)} 項：")
        for message in report.errors:
            print(f"  - {message}")
    if report.warnings:
        print(f"⚠ 警告 {len(report.warnings)} 項（原始資料問題，不擋建置）：")
        for message in report.warnings:
            print(f"  - {message}")
    if report.notes:
        print(f"ℹ 已裁示 {len(report.notes)} 項（作者已給出結論，見 docs/勘誤清單.md）：")
        for message in report.notes:
            print(f"  - {message}")
    if not report.errors and not report.warnings:
        print(f"✓ 無錯誤與未決問題（共 {report.checks} 項檢查）。")
    elif not report.errors:
        print(f"✓ 無錯誤（共 {report.checks} 項檢查）。")

    if report.errors:
        return 1
    return 1 if (args.strict and report.warnings) else 0


if __name__ == "__main__":
    raise SystemExit(main())
