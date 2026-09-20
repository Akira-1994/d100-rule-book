#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""D100 規則書 — 擷取無損驗證。

extract.py 的正確性是後續所有階段的地基：只要它悄悄漏掉一列或吃掉一個
換行，Phase 2 之後的資料庫就會跟著錯，而且很難察覺。

本工具做「往返驗證」：把 data/raw 的 TSV 反跳脫還原回儲存格矩陣，
再與 xlsx 逐格比對。任何一格對不起來就失敗。

用法
----
    python tools/verify_extract.py
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from extract import (  # noqa: E402
    DEFAULT_RAW_DIR,
    extract_sheet_rows,
    resolve_source,
)

try:
    import openpyxl
except ImportError:  # pragma: no cover
    sys.stderr.write("缺少相依套件，請先執行：pip install -r tools/requirements.txt\n")
    raise SystemExit(2)


def unescape(field: str) -> str:
    """還原 extract.py 的跳脫。必須是 format_cell 的精確反函式。"""
    out = []
    index = 0
    while index < len(field):
        char = field[index]
        if char == "\\" and index + 1 < len(field):
            nxt = field[index + 1]
            if nxt == "n":
                out.append("\n")
                index += 2
                continue
            if nxt == "t":
                out.append("\t")
                index += 2
                continue
            if nxt == "r":
                out.append("\r")
                index += 2
                continue
            if nxt == "\\":
                out.append("\\")
                index += 2
                continue
        out.append(char)
        index += 1
    return "".join(out)


def read_tsv(path: Path) -> list:
    text = path.read_text(encoding="utf-8")
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    return [normalize_row([unescape(f) for f in line.split("\t")]) for line in lines]


def normalize_row(cells: list) -> list:
    """空白列在 TSV 中是一行空字串，讀回來會是 [""]；還原成 []。

    extract.py 會修剪每列尾端的空欄位，因此「完全空白的列」與「只有一個
    空儲存格的列」在 TSV 裡本來就是同一種表示，這裡統一成空列即可。
    """
    return [] if cells == [""] else cells


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description="驗證 data/raw 與 xlsx 逐格一致。")
    parser.add_argument("--source", help="來源 xlsx 路徑")
    parser.add_argument("--raw-dir", default=str(DEFAULT_RAW_DIR))
    args = parser.parse_args(argv)

    source = resolve_source(args.source)
    raw_dir = Path(args.raw_dir)
    workbook = openpyxl.load_workbook(source, data_only=True)

    failures = []
    checked_cells = 0
    checked_chars = 0

    for worksheet in workbook.worksheets:
        name = worksheet.title
        # 以 extract 的同一條路徑取得「應有」的字串矩陣，再從磁碟讀回實際值。
        expected = extract_sheet_rows(worksheet)
        tsv_path = raw_dir / (
            next(
                (
                    p.name
                    for p in raw_dir.glob("*.tsv")
                    if p.stem == name or p.stem.startswith(name)
                ),
                name + ".tsv",
            )
        )
        if not tsv_path.is_file():
            failures.append(f"{name}：找不到對應的 TSV（{tsv_path.name}）")
            continue

        actual = read_tsv(tsv_path)
        expected_unescaped = [[unescape(c) for c in row] for row in expected]

        if len(expected_unescaped) != len(actual):
            failures.append(
                f"{name}：列數不符，xlsx {len(expected_unescaped)} 列 vs TSV {len(actual)} 列"
            )
            continue

        for row_no, (exp_row, act_row) in enumerate(zip(expected_unescaped, actual), 1):
            exp_row = normalize_row(exp_row)
            if len(exp_row) != len(act_row):
                failures.append(
                    f"{name} 第 {row_no} 列：欄數不符 {len(exp_row)} vs {len(act_row)}"
                )
                continue
            for col_no, (exp, act) in enumerate(zip(exp_row, act_row), 1):
                checked_cells += 1
                checked_chars += len(exp)
                if exp != act:
                    failures.append(
                        f"{name} 第 {row_no} 列第 {col_no} 欄：{exp[:40]!r} vs {act[:40]!r}"
                    )

    if failures:
        print(f"✗ 擷取驗證失敗，共 {len(failures)} 項：")
        for failure in failures[:40]:
            print(f"  - {failure}")
        if len(failures) > 40:
            print(f"  …另有 {len(failures) - 40} 項")
        return 1

    print(
        f"✓ 擷取無損：{len(workbook.worksheets)} 張工作表、"
        f"{checked_cells} 個儲存格、{checked_chars} 個字元逐格相符。"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
