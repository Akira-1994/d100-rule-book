# D100 規則書

自製龍與地下城房規「D100」的規則書資料庫與角色卡應用。

完整的四階段開發計劃見 [docs/PLAN.md](docs/PLAN.md)。

目前進度：**Phase 1 — 資料固化與變更追蹤**

## 環境準備

```bash
pip install -r tools/requirements.txt
git config core.hooksPath .githooks
```

## 目錄結構

| 路徑 | 說明 |
|---|---|
| `data/source/` | 原始試算表（唯一的上游來源，二進位檔） |
| `data/raw/` | 由 `extract.py` 機械式產生的文字快照，**不要手動編輯** |
| `data/raw/manifest.json` | 每張工作表的列數、欄數、內容雜湊與順序 |
| `data/raw/_merges.tsv` | 所有合併儲存格範圍，用來偵測純版面調整 |
| `data/raw/_formulas.tsv` | 所有公式（目前集中在「狩獵任務」） |
| `tools/` | Phase 1 工具鏈 |
| `docs/PLAN.md` | 開發計劃 |

## 試算表更新流程

當規則書作者給了新版 xlsx：

```bash
cp 新版.xlsx data/source/D100專長表.xlsx
python tools/extract.py
python tools/diff_report.py
```

`diff_report.py` 會告訴你哪一張工作表、第幾列、第幾欄、從什麼變成什麼。
確認無誤後再 `git add data/raw && git commit`。

### 常用指令

```bash
python tools/extract.py               # 重新擷取
python tools/extract.py --check       # 檢查 data/raw 是否與 xlsx 同步
python tools/diff_report.py           # 工作目錄 vs HEAD
python tools/diff_report.py --old git:v1.2 --out 報告.md
```

## 設計原則

`data/raw` 是 xlsx 的**忠實機械映射**，不做任何清理、欄位對齊或語意判斷
（那些屬於 Phase 2 的 SQLite 建置階段）。這樣才能把「上游試算表真的改了」
和「我們的解析邏輯改了」區分開來 —— 前者反映在 `source.sha256`，
後者反映在 `tool_version`，變更報告會分別標示。

擷取具備可重現性：同一份 xlsx 重跑任意次數，產出的每一個位元組都相同。
