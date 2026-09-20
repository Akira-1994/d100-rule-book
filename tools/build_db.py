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


def load_slot_mapping() -> dict:
    """讀入裝備欄位清單與「詞綴部位 → 裝備欄位」的對應。"""
    path = ERRATA_DIR / "_裝備欄位對應.yaml"
    if not path.is_file():
        return {"slots": [], "mappings": []}
    document = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
    return {
        "slots": document.get("equipment_slots", []) or [],
        "mappings": document.get("mappings", []) or [],
    }


def load_affix_aliases() -> dict:
    """讀入 data/errata/*.yaml 裡的詞綴名稱別名對照。

    回傳 {分布表寫法: 正式名稱: 理由}。同一個寫錯的名稱在分布表裡會出現
    很多次，逐列開勘誤不切實際，因此改用一條別名涵蓋全部。
    """
    if not ERRATA_DIR.is_dir():
        return {}
    aliases = {}
    for path in sorted(ERRATA_DIR.glob("*.yaml")):
        if path.name.startswith("_"):
            continue
        document = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        for item in document.get("affix_name_aliases", []) or []:
            if not item.get("reason"):
                raise SystemExit(f"{path.name}：詞綴別名 {item.get('from')!r} 缺少 reason。")
            aliases[item["from"]] = item
    return aliases


def load_errata() -> list:
    """讀入 data/errata/*.yaml，回傳扁平化的勘誤項目清單。"""
    if not ERRATA_DIR.is_dir():
        return []
    entries = []
    # 底線開頭的是設定檔（例如 _裝備欄位對應.yaml）而不是逐列勘誤，跳過。
    for path in sorted(ERRATA_DIR.glob("*.yaml")):
        if path.name.startswith("_"):
            continue
        document = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        sheet = document.get("sheet")
        if not sheet:
            raise SystemExit(f"{path.name} 缺少 sheet 欄位。")
        for entry in document.get("entries", []) or []:
            # row: all 代表整張工作表適用（例如作者裁示「術士全表分類為交涉」），
            # 逐列寫 56 筆同樣的理由只會把清單淹掉。
            if entry.get("row") == "all":
                entry["row"] = None
            elif "row" not in entry:
                raise SystemExit(f"{path.name} 有一筆勘誤缺少 row。")
            if not entry.get("reason"):
                raise SystemExit(
                    f"{path.name} 第 {entry['row']} 列的勘誤缺少 reason。"
                    "每一筆修正都必須寫明理由。"
                )
            entries.append({"sheet": sheet, "file": path.name, **entry})
    return entries


RECORD_BUCKETS = ("feats", "races", "affixes", "materials",
                  "material_affixes", "class_paths", "class_traits",
                  "invocations")

# 勘誤 YAML 裡寫單數的 target（讀起來比較自然），對應到內部的資料桶名稱。
TARGET_BUCKETS = {
    "feat": "feats",
    "race": "races",
    "affix": "affixes",
    "material": "materials",
    "material_affix": "material_affixes",
    "class_path": "class_paths",
    "class_trait": "class_traits",
    "invocation": "invocations",
}


def index_records(data: dict) -> dict:
    """建立 (工作表, 列號) -> [(bucket, 紀錄)] 的索引，供勘誤定位。

    值是清單而不是單一紀錄：素材詞綴表的同一列同時承載了素材本身與它的
    第一條詞綴，兩者列號相同。這種情況下勘誤必須用 target: 指明要改哪一個。
    """
    index = {}
    for bucket in RECORD_BUCKETS:
        for record in data[bucket]:
            key = (record["source_sheet"], record["source_row"])
            index.setdefault(key, []).append((bucket, record))
    return index


def _flag_row(entry: dict) -> dict:
    return {
        "sheet": entry["sheet"],
        "source_row": entry["row"],
        "field": None,
        "action": "flag",
        "raw_value": None,
        "fixed_value": None,
        "issue": entry["flag"],
        "reason": entry["reason"],
    }


def _apply_sheet_wide(data: dict, entry: dict) -> list:
    """把一筆勘誤套用到某張工作表的所有紀錄，只記一列到 errata 表。"""
    touched = 0
    for bucket in RECORD_BUCKETS:
        for record in data[bucket]:
            if record.get("source_sheet") != entry["sheet"]:
                continue
            for field, value in (entry.get("set") or {}).items():
                if field in record:
                    record[field] = list(value) if isinstance(value, list) else value
                    touched += 1
    if not touched:
        raise SystemExit(
            f"{entry['file']}：整表勘誤沒有套用到任何紀錄，"
            f"請確認工作表名稱 {entry['sheet']!r} 與欄位名稱是否正確。"
        )
    return [
        {
            "sheet": entry["sheet"],
            "source_row": None,
            "field": "、".join((entry.get("set") or {}).keys()) or None,
            "action": "set",
            "raw_value": None,
            "fixed_value": json.dumps(entry.get("set"), ensure_ascii=False),
            "issue": None,
            "reason": f"（整表適用，共 {touched} 處）{entry['reason']}",
        }
    ]


def apply_errata(data: dict, entries: list) -> list:
    """把勘誤套用到解析結果上，回傳要寫進 errata 表的列。"""
    index = index_records(data)
    feat_by_name = {f["name"]: f["id"] for f in data["feats"]}
    rows = []

    for entry in entries:
        if entry["row"] is None:
            rows.extend(_apply_sheet_wide(data, entry))
            continue

        key = (entry["sheet"], entry["row"])
        flag_only = "flag" in entry and not entry.get("set")

        if key not in index:
            # 純標記的勘誤談的是「原表這一列有問題」，不一定有對應的紀錄 ——
            # 被判定為重複而丟棄的列就是這種情況。只有要修改欄位時，
            # 才非得先找到那筆紀錄不可。
            if flag_only:
                rows.append(_flag_row(entry))
                continue
            raise SystemExit(
                f"{entry['file']}：找不到 {entry['sheet']} 第 {entry['row']} 列，"
                "勘誤可能已經過期（原表列號變動了？）。"
            )

        candidates = index[key]
        # Tier C 的職業表並排多個區塊，同一列可能有好幾個條目，
        # 此時勘誤要再以 col: 指明是哪一欄的那一個。
        if "col" in entry:
            narrowed = [
                (bucket, record) for bucket, record in candidates
                if record.get("source_col", 1) == entry["col"]
            ]
            if not narrowed:
                if flag_only:
                    rows.append(_flag_row(entry))
                    continue
                columns = sorted({r.get("source_col", 1) for _b, r in candidates})
                raise SystemExit(
                    f"{entry['file']}：第 {entry['row']} 列沒有 col={entry['col']} 的紀錄"
                    f"（該列有內容的欄位：{columns}）。"
                )
            candidates = narrowed

        available = "、".join(
            sorted({name for name, b in TARGET_BUCKETS.items()
                    if b in {bucket for bucket, _ in candidates}})
        )
        target = entry.get("target")
        if target:
            bucket_name = TARGET_BUCKETS.get(target, target)
            matched = [r for bucket, r in candidates if bucket == bucket_name]
            if not matched:
                raise SystemExit(
                    f"{entry['file']}：第 {entry['row']} 列沒有 target={target!r} 的紀錄"
                    f"（可用的有：{available}）。"
                )
            record = matched[0]
        elif len(candidates) > 1:
            raise SystemExit(
                f"{entry['file']}：第 {entry['row']} 列同時對應多種紀錄（{available}），"
                "請以 target: 指明要修正哪一個。"
            )
        else:
            record = candidates[0][1]

        if "flag" in entry:
            rows.append(_flag_row(entry))

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


def normalize_records(data: dict):
    """補齊各解析器之間不一致的選用欄位。

    Tier A/B 的專長沒有職業流派、Tier C 的專長沒有標籤，兩邊都只填自己
    關心的欄位。與其要求每個解析器都記得填滿，不如在寫入前統一補預設值。
    """
    for record in data["feats"]:
        record.setdefault("class_path_id", None)
        record.setdefault("tags", [])
        record.setdefault("source_col", 1)
    for record in data["rule_texts"]:
        record.setdefault("section", None)
        record.setdefault("subsection", None)
        record.setdefault("sort_order", 0)


def build(out_path: Path, raw_dir: Path) -> dict:
    affix_aliases = load_affix_aliases()
    data, issues = parsers.parse_all(raw_dir, affix_aliases)
    normalize_records(data)
    errata_entries = load_errata()
    errata_rows = apply_errata(data, errata_entries)
    for item in affix_aliases.values():
        errata_rows.append(
            {
                "sheet": "詞墜(依物品分類)",
                "source_row": None,
                "field": "affix_name",
                "action": "set",
                "raw_value": json.dumps(item["from"], ensure_ascii=False),
                "fixed_value": json.dumps(item["to"], ensure_ascii=False),
                "issue": None,
                "reason": item["reason"],
            }
        )
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
        """INSERT INTO class_path (id, class_name, path_kind, name, description,
                                   sort_order, source_sheet, source_row, source_col)
           VALUES (:id, :class_name, :path_kind, :name, :description,
                   :sort_order, :source_sheet, :source_row, :source_col)""",
        data["class_paths"],
    )
    connection.executemany(
        """INSERT INTO class_trait (id, class_path_id, name, description,
                                    source_sheet, source_row, source_col)
           VALUES (:id, :class_path_id, :name, :description,
                   :source_sheet, :source_row, :source_col)""",
        data["class_traits"],
    )
    connection.executemany(
        """INSERT INTO feat (id, name, feat_group, class_path_id, difficulty,
                             difficulty_raw, difficulty_scale, parent_id, effect,
                             source_sheet, source_row, source_col)
           VALUES (:id, :name, :feat_group, :class_path_id, :difficulty,
                   :difficulty_raw, :difficulty_scale, :parent_id, :effect,
                   :source_sheet, :source_row, :source_col)""",
        data["feats"],
    )
    connection.executemany(
        "INSERT INTO feat_category (feat_id, category) VALUES (?, ?)",
        [(f["id"], c) for f in data["feats"] for c in f["categories"]],
    )
    connection.executemany(
        "INSERT INTO feat_tag (feat_id, tag) VALUES (?, ?)",
        [(f["id"], t) for f in data["feats"] for t in f["tags"]],
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
        """INSERT INTO material (id, name, material_tier, cost_multiplier,
                                 source_sheet, source_row)
           VALUES (:id, :name, :material_tier, :cost_multiplier,
                   :source_sheet, :source_row)""",
        data["materials"],
    )
    connection.executemany(
        "INSERT INTO material_slot (material_id, slot) VALUES (?, ?)",
        [(m["id"], s) for m in data["materials"] for s in m["slots"]],
    )
    connection.executemany(
        """INSERT INTO material_affix (id, material_id, name, tier_rank,
                                       rarity_multiplier, roll_min, roll_max,
                                       effect, source_sheet, source_row)
           VALUES (:id, :material_id, :name, :tier_rank,
                   :rarity_multiplier, :roll_min, :roll_max,
                   :effect, :source_sheet, :source_row)""",
        data["material_affixes"],
    )
    connection.executemany(
        """INSERT INTO rule_text (sheet, section, subsection, body, sort_order,
                                  source_row, source_col)
           VALUES (:sheet, :section, :subsection, :body, :sort_order,
                   :source_row, :source_col)""",
        data["rule_texts"],
    )
    connection.executemany(
        """INSERT INTO ref_table (id, sheet, name, note, columns_json,
                                  sort_order, source_row, source_col)
           VALUES (:id, :sheet, :name, :note, :columns_json,
                   :sort_order, :source_row, :source_col)""",
        data["ref_tables"],
    )
    connection.executemany(
        """INSERT INTO ref_row (table_id, row_index, cells_json, source_row)
           VALUES (:table_id, :row_index, :cells_json, :source_row)""",
        data["ref_rows"],
    )
    slot_mapping = load_slot_mapping()
    connection.executemany(
        "INSERT INTO equipment_slot (code, sort_order, note) VALUES (?, ?, ?)",
        [
            (item["code"], order, item.get("note"))
            for order, item in enumerate(slot_mapping["slots"])
        ],
    )
    connection.executemany(
        "INSERT INTO affix_slot_mapping (affix_slot, equipment_slot) VALUES (?, ?)",
        [
            (item["affix_slot"], slot)
            for item in slot_mapping["mappings"]
            for slot in item["slots"]
        ],
    )

    connection.executemany(
        """INSERT INTO affix_distribution (slot, plus_label, affix_name,
                                           source_sheet, source_row, source_col)
           VALUES (:slot, :plus_label, :affix_name,
                   :source_sheet, :source_row, :source_col)""",
        data["affix_distribution"],
    )
    connection.executemany(
        """INSERT INTO invocation (id, name, cost, cost_raw, prereq_raw, effect,
                                   source_sheet, source_row, source_col)
           VALUES (:id, :name, :cost, :cost_raw, :prereq_raw, :effect,
                   :source_sheet, :source_row, :source_col)""",
        data["invocations"],
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
        "class_paths": len(data["class_paths"]),
        "class_traits": len(data["class_traits"]),
        "materials": len(data["materials"]),
        "material_affixes": len(data["material_affixes"]),
        "rule_texts": len(data["rule_texts"]),
        "ref_tables": len(data["ref_tables"]),
        "ref_rows": len(data["ref_rows"]),
        "invocations": len(data["invocations"]),
        "affix_distribution": len(data["affix_distribution"]),
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
        f"  專長 {stats['feats']}（職業流派 {stats['class_paths']}、"
        f"被動特性 {stats['class_traits']}）、"
        f"種族 {stats['races']}、詞綴 {stats['affixes']}、"
        f"素材 {stats['materials']}（素材詞綴 {stats['material_affixes']}）、"
        f"規則段落 {stats['rule_texts']}、對照表 {stats['ref_tables']}"
        f"（{stats['ref_rows']} 列）、祈喚 {stats['invocations']}、"
        f"詞綴分布 {stats['affix_distribution']}"
    )
    print(
        f"  勘誤紀錄 {stats['errata']} 筆（其中人工勘誤 {stats['manual_errata']} 筆）"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
