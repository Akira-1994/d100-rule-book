#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""D100 規則書 — 勘誤清單產生器。

把資料庫裡的 errata 表整理成一份給人看的報告，用途是直接拿去跟規則書
作者對帳：哪些是我們已經替他修掉的、哪些是我們不敢動要他確認的。

用法
----
    python tools/errata_report.py
    python tools/errata_report.py --out docs/勘誤清單.md
"""

from __future__ import annotations

import argparse
import json
import re
import sqlite3
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DB = REPO_ROOT / "dist" / "d100.db"

# issue 代碼以 resolved_ 開頭 = 作者已經給出結論，不再是待確認事項。
RESOLVED_PREFIX = "resolved_"

ISSUE_LABELS = {
    "resolved_dm_ruling": "由 DM 裁定",
    "resolved_pending_content": "保留待補內容",
    "resolved_effect_pending": "入職門檻，效果待補",
    "resolved_duplicate": "重複列，已去重",
    "incomplete_entry": "條目不完整",
    "unknown_prerequisite": "前置條件未定",
    "missing_effect": "缺少效果敘述",
    "missing_category": "缺少分類",
    "missing_difficulty": "缺少難度",
    "difficulty_not_numeric": "難度非數值",
    "category_unrecognized": "分類無法辨識",
    "cp_not_numeric": "CP 調整非數值",
    "name_normalized": "名稱已正規化",
    "missing_slot": "缺少裝備部位",
    "plus_cost_unparsed": "加值兌換無法解析",
}


_CJK = (
    "　-〿㐀-䶿一-鿿豈-﫿＀-￯"
)
_CJK_GAP = re.compile(f"(?<=[{_CJK}])[ \t]+(?=[{_CJK}])")


def tidy(text: str) -> str:
    """整理勘誤理由：收斂空白，並去掉 YAML 折行在中文之間留下的空格。"""
    if not text:
        return ""
    collapsed = " ".join(text.split())
    return _CJK_GAP.sub("", collapsed)


def md_cell(text: str) -> str:
    """把值塞進 Markdown 表格：跳脫豎線、換行改成 <br>。"""
    return str(text).replace("|", "\\|").replace("\n", "<br>")


def unjson(value):
    if value is None:
        return "（無）"
    try:
        parsed = json.loads(value)
    except (TypeError, ValueError):
        return str(value)
    if parsed is None:
        return "（空）"
    if isinstance(parsed, list):
        return "、".join(str(v) for v in parsed) if parsed else "（空）"
    return str(parsed)


def build_report(connection) -> str:
    info = dict(connection.execute("SELECT key, value FROM build_info"))
    out = ["# D100 規則書勘誤清單", ""]
    out.append(f"- 來源檔案：`{info.get('source_file', '？')}`")
    out.append(f"- 來源雜湊：`{info.get('source_sha256', '？')[:16]}`")
    out.append("")
    out.append(
        "這份清單由 `tools/errata_report.py` 自動產生。"
        "**「已修正」**是我們在資料庫中替換掉的值，原始試算表並未變動；"
        "**「已裁示」**是規則書作者給出結論、資料庫已照辦的項目；"
        "**「待作者確認」**是我們無法判斷正確答案、只做標記的項目。"
    )
    out.append("")

    fixes = connection.execute(
        "SELECT sheet, source_row, field, raw_value, fixed_value, reason"
        " FROM errata WHERE action = 'set' ORDER BY sheet, source_row, field"
    ).fetchall()

    # 同一列同一個理由常常一次改好幾個欄位（例如補上父項連同繼承的分類與
    # 難度），逐欄重複整段理由會讓清單很難讀，這裡合併成一個區塊。
    grouped = {}
    order = []
    for sheet, row, field, raw, fixed, reason in fixes:
        key = (sheet, row, tidy(reason))
        if key not in grouped:
            grouped[key] = []
            order.append(key)
        grouped[key].append((field, raw, fixed))

    out.append(f"## 已修正（{len(order)} 項，共 {len(fixes)} 個欄位）")
    out.append("")
    if not order:
        out.append("（無）")
    for key in order:
        sheet, row, reason = key
        out.append(f"### {sheet} 第 {row} 列")
        out.append("")
        out.append(f"{reason}")
        out.append("")
        out.append("| 欄位 | 原值 | 改為 |")
        out.append("|---|---|---|")
        for field, raw, fixed in grouped[key]:
            out.append(
                f"| `{field}` | {md_cell(unjson(raw))} | {md_cell(unjson(fixed))} |"
            )
        out.append("")

    flags = connection.execute(
        "SELECT sheet, source_row, issue, reason FROM errata"
        " WHERE action = 'flag' ORDER BY sheet, source_row, issue"
    ).fetchall()

    # 分三類：作者已裁示的、還需要作者回覆的、以及純粹記錄我們做了正規化的。
    resolved = [f for f in flags if (f[2] or "").startswith(RESOLVED_PREFIX)]
    informational = [f for f in flags if f[2] == "name_normalized"]
    needs_author = [
        f for f in flags
        if f not in resolved and f not in informational
    ]

    out.append(f"## 已裁示（{len(resolved)} 項）")
    out.append("")
    out.append("作者已給出結論，資料庫已照辦，列在這裡供日後回溯。")
    out.append("")
    if not resolved:
        out.append("（無）")
    else:
        out.append("| 工作表 | 列 | 結論 | 說明 |")
        out.append("|---|---:|---|---|")
        for sheet, row, issue, reason in resolved:
            label = ISSUE_LABELS.get(issue, issue)
            text = tidy(reason).replace("|", "\\|")
            out.append(f"| {sheet} | {row if row is not None else '整表'} | {label} | {text} |")
    out.append("")

    out.append(f"## 待作者確認（{len(needs_author)} 項）")
    out.append("")
    if not needs_author:
        out.append("（無，全部項目均已裁示）")
    else:
        out.append("| 工作表 | 列 | 問題 | 說明 |")
        out.append("|---|---:|---|---|")
        for sheet, row, issue, reason in needs_author:
            label = ISSUE_LABELS.get(issue, issue)
            text = tidy(reason).replace("|", "\\|")
            out.append(f"| {sheet} | {row} | {label} | {text} |")
    out.append("")

    out.append(f"## 僅供記錄（{len(informational)} 項）")
    out.append("")
    out.append("我們自動做掉、不需要作者處理的正規化。")
    out.append("")
    if informational:
        out.append("| 工作表 | 列 | 處理 |")
        out.append("|---|---:|---|")
        for sheet, row, issue, reason in informational:
            text = tidy(reason).replace("|", "\\|")
            out.append(f"| {sheet} | {row} | {text} |")
    else:
        out.append("（無）")

    return "\n".join(out).rstrip() + "\n"


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description="產生 D100 規則書勘誤清單。")
    parser.add_argument("--db", default=str(DEFAULT_DB))
    parser.add_argument("--out", help="輸出檔案（預設印到標準輸出）")
    args = parser.parse_args(argv)

    db_path = Path(args.db)
    if not db_path.is_file():
        raise SystemExit(f"找不到資料庫：{db_path}，請先執行 python tools/build_db.py")

    connection = sqlite3.connect(db_path)
    report = build_report(connection)
    connection.close()

    if args.out:
        out_path = Path(args.out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        with out_path.open("w", encoding="utf-8", newline="\n") as handle:
            handle.write(report)
        print(f"勘誤清單已寫入：{out_path}")
    else:
        sys.stdout.write(report)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
