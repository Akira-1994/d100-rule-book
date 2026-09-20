#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""D100 規則書 — 從 data/raw + data/errata 建置 SQLite 資料庫。

資料庫是建置產物，不進 git。真實來源是 data/raw（xlsx 的忠實映射）
與 data/errata（我們對原始資料的修正，連同理由）。整個流程可重現：
刪掉 dist/d100.db 再跑一次，結果完全一樣。

用法
----
    python tools/build_db.py
    python tools/build_db.py --out dist/d100.db
"""

from __future__ import annotations

import argparse
import json
import sqlite3
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import parsers  # noqa: E402
from rawio import ATTRIBUTES, CATEGORIES, RAW_DIR, REPO_ROOT  # noqa: E402

try:
    import yaml
except ImportError:  # pragma: no cover
    sys.stderr.write("缺少相依套件，請先執行：pip install -r tools/requirements.txt\n")
    raise SystemExit(2)

SCHEMA_PATH = REPO_ROOT / "db" / "schema.sql"
ERRATA_DIR = REPO_ROOT / "data" / "errata"
DEFAULT_OUT = REPO_ROOT / "dist" / "d100.db"

SCHEMA_VERSION = "2.0.0"

# 解析器自動偵測的問題代碼 -> 若人工勘誤已設定了這個欄位，該問題即視為已解決
AUTO_ISSUE_FIELDS = {
    "missing_category": "categories",
    "missing_difficulty": "difficulty",
    "missing_effect": "effect",
    "difficulty_not_numeric": "difficulty",
    "cp_not_numeric": "cp_cost",
    "missing_slot": "slots",
}


def load_errata() -> list:
    """讀入 data/errata/*.yaml，回傳扁平化的勘誤項目清單。"""
    if not ERRATA_DIR.is_dir():
        return []
    entries = []
    for path in sorted(ERRATA_DIR.glob("*.yaml")):
        document = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        sheet = document.get("sheet")
        if not sheet:
            raise SystemExit(f"{path.name} 缺少 sheet 欄位。")
        for entry in document.get("entries", []) or []:
            if "row" not in entry:
                raise SystemExit(f"{path.name} 有一筆勘誤缺少 row。")
            if not entry.get("reason"):
                raise SystemExit(
                    f"{path.name} 第 {entry['row']} 列的勘誤缺少 reason。"
                    "每一筆修正都必須寫明理由。"
                )
            entries.append({"sheet": sheet, "file": path.name, **entry})
    return entries


def index_records(data: dict) -> dict:
    """建立 (工作表, 列號) -> 紀錄 的索引，供勘誤定位。"""
    index = {}
    for bucket in ("feats", "races", "affixes"):
        for record in data[bucket]:
            index[(record["source_sheet"], record["source_row"])] = (bucket, record)
    return index


def apply_errata(data: dict, entries: list) -> list:
    """把勘誤套用到解析結果上，回傳要寫進 errata 表的列。"""
    index = index_records(data)
    feat_by_name = {f["name"]: f["id"] for f in data["feats"]}
    rows = []

    for entry in entries:
        key = (entry["sheet"], entry["row"])
        if key not in index:
            raise SystemExit(
                f"{entry['file']}：找不到 {entry['sheet']} 第 {entry['row']} 列，"
                "勘誤可能已經過期（原表列號變動了？）。"
            )
        _bucket, record = index[key]

        if "flag" in entry:
            rows.append(
                {
                    "sheet": entry["sheet"],
                    "source_row": entry["row"],
                    "field": None,
                    "action": "flag",
                    "raw_value": None,
                    "fixed_value": None,
                    "issue": entry["flag"],
                    "reason": entry["reason"],
                }
            )

        for field, value in (entry.get("set") or {}).items():
            if field == "parent":
                parent_id = feat_by_name.get(value)
                if parent_id is None:
                    raise SystemExit(
                        f"{entry['file']}：第 {entry['row']} 列指定的父項「{value}」不存在。"
                    )
                before, record["parent_id"] = record.get("parent_id"), parent_id
            else:
                if field not in record:
                    raise SystemExit(
                        f"{entry['file']}：第 {entry['row']} 列有未知欄位 {field!r}。"
                    )
                before, record[field] = record[field], value

            rows.append(
                {
                    "sheet": entry["sheet"],
                    "source_row": entry["row"],
                    "field": field,
                    "action": "set",
                    "raw_value": json.dumps(before, ensure_ascii=False),
                    "fixed_value": json.dumps(value, ensure_ascii=False),
                    "issue": None,
                    "reason": entry["reason"],
                }
            )

    return rows


def build(out_path: Path, raw_dir: Path) -> dict:
    data, issues = parsers.parse_all(raw_dir)
    errata_entries = load_errata()
    errata_rows = apply_errata(data, errata_entries)
    # 前置條件在勘誤之後才解析，這樣勘誤才改得動 prereq_raw。
    parsers.resolve_prereqs(data)

    # 解析過程自動偵測到的問題也一併記錄，與人工勘誤並列 ——
    # 但已經被人工勘誤處理掉的就不要再報一次，否則清單上會出現
    # 「缺少分類」這種我們明明已經補好的項目。
    flagged = {(e["sheet"], e["source_row"], e.get("issue")) for e in errata_rows}
    resolved = {
        (e["sheet"], e["source_row"], code)
        for e in errata_rows
        if e["action"] == "set"
        for code, field in AUTO_ISSUE_FIELDS.items()
        if e["field"] == field
    }
    # 人工勘誤可以用 covers: 明說「這幾個自動偵測的問題我已經在說明裡涵蓋了」，
    # 避免同一格在清單上被報兩次。
    resolved |= {
        (entry["sheet"], entry["row"], code)
        for entry in errata_entries
        for code in (entry.get("covers") or [])
    }
    for sheet, row, code, detail in issues:
        if (sheet, row, code) in flagged or (sheet, row, code) in resolved:
            continue
        errata_rows.append(
            {
                "sheet": sheet,
                "source_row": row,
                "field": None,
                "action": "flag",
                "raw_value": None,
                "fixed_value": None,
                "issue": code,
                "reason": f"（解析器自動偵測）{detail}",
            }
        )

    out_path.parent.mkdir(parents=True, exist_ok=True)
    if out_path.exists():
        out_path.unlink()

    connection = sqlite3.connect(out_path)
    connection.executescript(SCHEMA_PATH.read_text(encoding="utf-8"))

    connection.executemany(
        "INSERT INTO category (code, formula, sort_order) VALUES (?, ?, ?)",
        [(code, formula, i) for i, (code, formula) in enumerate(CATEGORIES.items())],
    )
    connection.executemany(
        "INSERT INTO attribute (code, name, sort_order) VALUES (?, ?, ?)",
        [(code, name, i) for i, (code, name) in enumerate(ATTRIBUTES.items())],
    )

    connection.executemany(
        """INSERT INTO feat (id, name, feat_group, difficulty, difficulty_raw,
                             parent_id, effect, source_sheet, source_row)
           VALUES (:id, :name, :feat_group, :difficulty, :difficulty_raw,
                   :parent_id, :effect, :source_sheet, :source_row)""",
        data["feats"],
    )
    connection.executemany(
        "INSERT INTO feat_category (feat_id, category) VALUES (?, ?)",
        [(f["id"], c) for f in data["feats"] for c in f["categories"]],
    )
    connection.executemany(
        """INSERT INTO feat_prereq (feat_id, seq, kind, ref_feat_id, ref_code,
                                    min_level, raw_text)
           VALUES (?, ?, ?, ?, ?, ?, ?)""",
        [
            (f["id"], seq, p["kind"], p["ref_feat_id"], p["ref_code"],
             p["min_level"], p["raw_text"])
            for f in data["feats"]
            for seq, p in enumerate(f["prereqs"], start=1)
        ],
    )

    connection.executemany(
        """INSERT INTO race (id, name, cp_cost, cp_raw, attr_text, racial_feat_text,
                             skill_mod_text, special_text, source_sheet, source_row)
           VALUES (:id, :name, :cp_cost, :cp_raw, :attr_text, :racial_feat_text,
                   :skill_mod_text, :special_text, :source_sheet, :source_row)""",
        data["races"],
    )
    connection.executemany(
        "INSERT INTO race_attr_modifier (race_id, attr, delta) VALUES (?, ?, ?)",
        [
            (r["id"], attr, delta)
            for r in data["races"]
            for attr, delta in r["attr_modifiers"].items()
        ],
    )

    connection.executemany(
        """INSERT INTO affix (id, name, affix_tier, plus_cost_raw, effect,
                              source_sheet, source_row)
           VALUES (:id, :name, :affix_tier, :plus_cost_raw, :effect,
                   :source_sheet, :source_row)""",
        data["affixes"],
    )
    connection.executemany(
        "INSERT INTO affix_slot (affix_id, slot) VALUES (?, ?)",
        [(a["id"], s) for a in data["affixes"] for s in a["slots"]],
    )
    connection.executemany(
        "INSERT INTO affix_rank (affix_id, rank, plus_cost, condition) VALUES (?, ?, ?, ?)",
        [
            (a["id"], rank, cost, condition or "")
            for a in data["affixes"]
            for rank, cost, condition in a["ranks"]
        ],
    )

    connection.executemany(
        """INSERT INTO errata (sheet, source_row, field, action, raw_value,
                               fixed_value, issue, reason)
           VALUES (:sheet, :source_row, :field, :action, :raw_value,
                   :fixed_value, :issue, :reason)""",
        errata_rows,
    )

    manifest = json.loads((raw_dir / "manifest.json").read_text(encoding="utf-8"))
    connection.executemany(
        "INSERT INTO build_info (key, value) VALUES (?, ?)",
        [
            ("schema_version", SCHEMA_VERSION),
            ("extract_tool_version", manifest["tool_version"]),
            ("source_file", manifest["source"]["file"]),
            ("source_sha256", manifest["source"]["sha256"]),
        ],
    )

    connection.commit()
    connection.execute("PRAGMA foreign_keys = ON")
    violations = connection.execute("PRAGMA foreign_key_check").fetchall()
    if violations:
        connection.close()
        raise SystemExit(f"外鍵違規 {len(violations)} 筆，建置中止：{violations[:5]}")
    connection.close()

    return {
        "feats": len(data["feats"]),
        "races": len(data["races"]),
        "affixes": len(data["affixes"]),
        "errata": len(errata_rows),
        "manual_errata": len(errata_entries),
    }


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description="建置 D100 規則書 SQLite 資料庫。")
    parser.add_argument("--out", default=str(DEFAULT_OUT), help="輸出路徑")
    parser.add_argument("--raw-dir", default=str(RAW_DIR), help="來源 data/raw 目錄")
    args = parser.parse_args(argv)

    out_path = Path(args.out)
    stats = build(out_path, Path(args.raw_dir))

    print(f"已建置：{out_path}")
    print(
        f"  專長 {stats['feats']}、種族 {stats['races']}、詞綴 {stats['affixes']}"
    )
    print(
        f"  勘誤紀錄 {stats['errata']} 筆（其中人工勘誤 {stats['manual_errata']} 筆）"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
