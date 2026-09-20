#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""D100 規則書 — Phase 1 變更報告產生器。

比對兩份 data/raw 快照，輸出人類可讀的中文報告，回答「這次試算表更新
到底改了什麼」。報告會細到「哪一張工作表、第幾列、第幾欄、從什麼變成什麼」。

因為 extract.py 保證了輸出的可重現性，任何差異都必定來自下列兩者之一，
而報告會明確區分：
  - 上游試算表真的改了（source.sha256 變更）
  - 我們的擷取邏輯改了（tool_version 變更）

用法
----
    python tools/diff_report.py                        # 拿工作目錄與 HEAD 比
    python tools/diff_report.py --old git:v1.2         # 與某個 tag/commit 比
    python tools/diff_report.py --old old_raw/ --new data/raw
    python tools/diff_report.py --out 變更報告.md
"""

from __future__ import annotations

import argparse
import difflib
import json
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_RAW_DIR = REPO_ROOT / "data" / "raw"
RAW_REL_POSIX = "data/raw"

MANIFEST_NAME = "manifest.json"
SIDECAR_FILES = ("_merges.tsv", "_formulas.tsv")

VALUE_MAX = 60
HEADER_LABEL_MAX = 12


class Snapshot:
    """一份 data/raw 快照，可能來自磁碟目錄或某個 git 版本。"""

    def __init__(self, label: str, files: dict):
        self.label = label
        self.files = files  # {檔名: 內容字串}

    @property
    def manifest(self) -> dict:
        raw = self.files.get(MANIFEST_NAME)
        return json.loads(raw) if raw else {}

    def lines(self, filename: str) -> list:
        content = self.files.get(filename)
        if content is None:
            return []
        return content.split("\n")[:-1] if content.endswith("\n") else content.split("\n")

    @property
    def is_empty(self) -> bool:
        return not self.files


def load_from_dir(path: Path, label: str) -> Snapshot:
    if not path.is_dir():
        raise SystemExit(f"找不到目錄：{path}")
    files = {}
    for item in sorted(path.iterdir()):
        if item.is_file():
            files[item.name] = item.read_text(encoding="utf-8")
    return Snapshot(label, files)


def load_from_git(rev: str, label: str) -> Snapshot:
    listing = subprocess.run(
        ["git", "-c", "core.quotepath=false", "ls-tree", "-r", "--name-only",
         rev, "--", RAW_REL_POSIX],
        cwd=REPO_ROOT, capture_output=True,
    )
    if listing.returncode != 0:
        return Snapshot(label, {})

    files = {}
    for path in listing.stdout.decode("utf-8").splitlines():
        if not path.strip():
            continue
        blob = subprocess.run(
            ["git", "show", f"{rev}:{path}"], cwd=REPO_ROOT, capture_output=True
        )
        if blob.returncode == 0:
            files[path.split("/")[-1]] = blob.stdout.decode("utf-8")
    return Snapshot(label, files)


def load_snapshot(spec: str, label: str) -> Snapshot:
    if spec.startswith("git:"):
        return load_from_git(spec[4:], label)
    return load_from_dir(Path(spec), label)


def clip(text: str) -> str:
    text = text if text != "" else "（空）"
    return text if len(text) <= VALUE_MAX else text[:VALUE_MAX] + "…"


def render_change(old_cell: str, new_cell: str) -> str:
    """呈現單一欄位的前後差異。

    規則書大量使用數百字的效果敘述，若兩邊都直接截斷，畫面上會出現
    「一長串文字 → 一模一樣的一長串文字」這種毫無資訊量的輸出。
    因此長文字改為剝掉共同前後綴，只呈現真正改動的片段與其位置。
    """
    if len(old_cell) <= VALUE_MAX and len(new_cell) <= VALUE_MAX:
        return f"{clip(old_cell)} → {clip(new_cell)}"

    prefix = 0
    limit = min(len(old_cell), len(new_cell))
    while prefix < limit and old_cell[prefix] == new_cell[prefix]:
        prefix += 1

    suffix = 0
    while (
        suffix < len(old_cell) - prefix
        and suffix < len(new_cell) - prefix
        and old_cell[len(old_cell) - 1 - suffix] == new_cell[len(new_cell) - 1 - suffix]
    ):
        suffix += 1

    old_mid = old_cell[prefix : len(old_cell) - suffix]
    new_mid = new_cell[prefix : len(new_cell) - suffix]
    same = prefix + suffix

    text = f"第 {prefix + 1} 字起 {clip(old_mid)} → {clip(new_mid)}"
    if same:
        text += f"（前後共 {same} 字未變）"
    return text


def column_label(header: list, index: int) -> str:
    """第 N 欄，若表頭列有短標題則附上欄位名稱。"""
    if index < len(header):
        name = header[index].strip()
        if name and len(name) <= HEADER_LABEL_MAX:
            return f"第 {index + 1} 欄「{name}」"
    return f"第 {index + 1} 欄"


def diff_row_pair(old_row: str, new_row: str, header: list) -> list:
    """同一列的欄位級差異。"""
    old_cells = old_row.split("\t")
    new_cells = new_row.split("\t")
    notes = []
    for i in range(max(len(old_cells), len(new_cells))):
        old_cell = old_cells[i] if i < len(old_cells) else ""
        new_cell = new_cells[i] if i < len(new_cells) else ""
        if old_cell != new_cell:
            notes.append(f"{column_label(header, i)}：{render_change(old_cell, new_cell)}")
    if len(old_cells) != len(new_cells):
        notes.append(f"欄位數 {len(old_cells)} → {len(new_cells)}")
    return notes


def first_cell(row: str) -> str:
    cells = row.split("\t")
    return clip(cells[0]) if cells else "（空）"


def diff_sheet(old_lines: list, new_lines: list, max_rows: int) -> list:
    """回傳一張工作表的變更敘述清單。"""
    header = new_lines[0].split("\t") if new_lines else []
    entries = []
    matcher = difflib.SequenceMatcher(None, old_lines, new_lines, autojunk=False)

    for tag, i1, i2, j1, j2 in matcher.get_opcodes():
        if tag == "equal":
            continue
        if tag == "replace" and (i2 - i1) == (j2 - j1):
            for offset in range(i2 - i1):
                old_row = old_lines[i1 + offset]
                new_row = new_lines[j1 + offset]
                row_no = j1 + offset + 1
                old_no = i1 + offset + 1
                location = f"第 {row_no} 列" if row_no == old_no else f"第 {row_no} 列（原第 {old_no} 列）"
                for note in diff_row_pair(old_row, new_row, header):
                    entries.append(f"- 修改 {location}〔{first_cell(new_row)}〕{note}")
            continue
        if tag in ("replace", "delete"):
            for offset in range(i2 - i1):
                entries.append(
                    f"- 刪除 原第 {i1 + offset + 1} 列：{clip(old_lines[i1 + offset])}"
                )
        if tag in ("replace", "insert"):
            for offset in range(j2 - j1):
                entries.append(
                    f"- 新增 第 {j1 + offset + 1} 列：{clip(new_lines[j1 + offset])}"
                )

    if len(entries) > max_rows:
        hidden = len(entries) - max_rows
        entries = entries[:max_rows] + [f"- …另有 {hidden} 筆變更未列出（可用 --max-rows 調整）"]
    return entries


def diff_sidecar(old_lines: list, new_lines: list, max_rows: int) -> list:
    """_merges.tsv / _formulas.tsv 的差異：列號無意義，只列增刪。"""
    entries = []
    matcher = difflib.SequenceMatcher(None, old_lines, new_lines, autojunk=False)
    for tag, i1, i2, j1, j2 in matcher.get_opcodes():
        if tag == "equal":
            continue
        for offset in range(i2 - i1):
            entries.append(f"- 移除：{clip(old_lines[i1 + offset])}")
        for offset in range(j2 - j1):
            entries.append(f"- 新增：{clip(new_lines[j1 + offset])}")
    if len(entries) > max_rows:
        hidden = len(entries) - max_rows
        entries = entries[:max_rows] + [f"- …另有 {hidden} 筆變更未列出（可用 --max-rows 調整）"]
    return entries


def sheet_index(manifest: dict) -> dict:
    return {s["name"]: s for s in manifest.get("sheets", [])}


def cached_value_warning(old_mf: dict, new_mf: dict) -> list:
    """偵測公式快取值整批消失 —— 一種安靜但足以毀掉整張表的變化。

    Excel 會把公式的計算結果一併寫進檔案，我們讀的就是這份快取。
    openpyxl、部分線上編輯器與 headless LibreOffice 存檔時不會重算公式，
    快取便整批變成空值，在報告裡看起來像是「幾百個數字被刪光了」。
    """

    def total(manifest: dict) -> int:
        return sum(
            s.get("formulas_without_cached_value", 0) for s in manifest.get("sheets", [])
        )

    old_bad, new_bad = total(old_mf), total(new_mf)
    if new_bad <= old_bad:
        return []

    lines = [
        f"> 🚨 **公式快取值遺失**：無快取值的公式由 {old_bad} 條增加到 {new_bad} 條。",
        ">",
        "> 這通常不是規則改動，而是這份 xlsx 被「不會重算公式的工具」存過"
        "（openpyxl、headless LibreOffice、部分線上編輯器）。",
        "> 受影響的工作表會出現大量「數值 → （空）」，看起來像資料被刪光。",
        ">",
        "> 處置：用真正的 Excel 重新開啟該檔並存檔一次，再重跑 "
        "`python tools/extract.py`。",
        ">",
    ]
    old_by_name = {
        s["name"]: s.get("formulas_without_cached_value", 0)
        for s in old_mf.get("sheets", [])
    }
    for sheet in new_mf.get("sheets", []):
        bad = sheet.get("formulas_without_cached_value", 0)
        if bad > old_by_name.get(sheet["name"], 0):
            lines.append(f"> - {sheet['name']}：{bad}/{sheet['formulas']} 條公式失去快取值")
    lines.append("")
    return lines


def build_report(old: Snapshot, new: Snapshot, max_rows: int) -> str:
    out = ["# D100 規則書資料變更報告", ""]
    out.append(f"- 比較基準：`{old.label}`")
    out.append(f"- 比較對象：`{new.label}`")
    out.append("")

    if old.is_empty:
        out.append("基準快照不存在（首次建立），因此無法比對。")
        out.append("")
        total = new.manifest.get("totals", {})
        out.append(
            f"對象快照共 {total.get('sheets', 0)} 張工作表、{total.get('rows', 0)} 列。"
        )
        return "\n".join(out) + "\n"

    old_mf, new_mf = old.manifest, new.manifest
    old_src = old_mf.get("source", {})
    new_src = new_mf.get("source", {})

    out.append("## 摘要")
    out.append("")
    out.append("| 項目 | 基準 | 對象 | |")
    out.append("|---|---|---|---|")

    def row(label, a, b):
        mark = "" if a == b else "⚠️ 已變更"
        out.append(f"| {label} | {a} | {b} | {mark} |")

    row("來源檔名", old_src.get("file", "—"), new_src.get("file", "—"))
    row("來源雜湊", (old_src.get("sha256") or "—")[:12], (new_src.get("sha256") or "—")[:12])
    row("擷取工具版本", old_mf.get("tool_version", "—"), new_mf.get("tool_version", "—"))
    for key, label in (("sheets", "工作表數"), ("rows", "總列數"),
                       ("merged_ranges", "合併範圍數"), ("formulas", "公式數")):
        row(label, old_mf.get("totals", {}).get(key, "—"), new_mf.get("totals", {}).get(key, "—"))
    out.append("")

    if old_mf.get("tool_version") != new_mf.get("tool_version"):
        out.append(
            "> ⚠️ 擷取工具版本不同，以下差異可能部分來自擷取邏輯調整，而非試算表內容變更。"
        )
        out.append("")

    out.extend(cached_value_warning(old_mf, new_mf))

    old_sheets, new_sheets = sheet_index(old_mf), sheet_index(new_mf)
    added = [n for n in new_sheets if n not in old_sheets]
    removed = [n for n in old_sheets if n not in new_sheets]
    common = [n for n in new_sheets if n in old_sheets]
    changed = [n for n in common if old_sheets[n]["sha256"] != new_sheets[n]["sha256"]]
    reordered = [
        n for n in common if old_sheets[n]["index"] != new_sheets[n]["index"]
    ]

    out.append("## 工作表異動")
    out.append("")
    out.append(f"- 新增 {len(added)} 張、刪除 {len(removed)} 張、內容變更 {len(changed)} 張、位置調動 {len(reordered)} 張")
    for name in added:
        info = new_sheets[name]
        out.append(f"- ➕ 新增工作表「{name}」（{info['rows']} 列 × {info['cols']} 欄）")
    for name in removed:
        info = old_sheets[name]
        out.append(f"- ➖ 刪除工作表「{name}」（原 {info['rows']} 列）")
    for name in reordered:
        out.append(
            f"- ↔️ 「{name}」位置由第 {old_sheets[name]['index'] + 1} 張移到第 {new_sheets[name]['index'] + 1} 張"
        )
    out.append("")

    sidecar_changed = [
        f for f in SIDECAR_FILES if old.files.get(f) != new.files.get(f)
    ]

    if not changed and not added and not removed and not sidecar_changed:
        out.append("## 變更明細")
        out.append("")
        out.append("內容完全相同，無任何變更。")
        return "\n".join(out) + "\n"

    out.append("## 變更明細")
    out.append("")

    for name in changed:
        info_old, info_new = old_sheets[name], new_sheets[name]
        out.append(f"### {name}")
        out.append("")
        if (info_old["rows"], info_old["cols"]) != (info_new["rows"], info_new["cols"]):
            out.append(
                f"規模：{info_old['rows']} 列 × {info_old['cols']} 欄 → "
                f"{info_new['rows']} 列 × {info_new['cols']} 欄"
            )
            out.append("")
        entries = diff_sheet(
            old.lines(info_old["file"]), new.lines(info_new["file"]), max_rows
        )
        out.extend(entries if entries else ["- （內容雜湊不同但逐列比對無差異）"])
        out.append("")

    for filename in sidecar_changed:
        title = "合併儲存格（版面）" if filename == "_merges.tsv" else "公式"
        out.append(f"### {title} — {filename}")
        out.append("")
        entries = diff_sidecar(old.lines(filename), new.lines(filename), max_rows)
        out.extend(entries if entries else ["- （無差異）"])
        out.append("")

    return "\n".join(out).rstrip() + "\n"


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(
        description="比對兩份 data/raw 快照並產生中文變更報告。"
    )
    parser.add_argument(
        "--old", default="git:HEAD",
        help="比較基準：目錄路徑，或 git:<rev>（預設 git:HEAD）",
    )
    parser.add_argument(
        "--new", default=str(DEFAULT_RAW_DIR), help="比較對象目錄（預設 data/raw）"
    )
    parser.add_argument("--out", help="輸出檔案路徑（預設印到標準輸出）")
    parser.add_argument(
        "--max-rows", type=int, default=40, help="每張表最多列出的變更筆數（預設 40）"
    )
    args = parser.parse_args(argv)

    old = load_snapshot(args.old, args.old)
    new = load_snapshot(args.new, args.new)
    report = build_report(old, new, args.max_rows)

    if args.out:
        out_path = Path(args.out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        with out_path.open("w", encoding="utf-8", newline="\n") as handle:
            handle.write(report)
        print(f"報告已寫入：{out_path}")
    else:
        sys.stdout.write(report)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
