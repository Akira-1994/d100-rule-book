# D100 規則書

自製龍與地下城房規「D100」的規則書資料庫與角色卡應用。

完整的四階段開發計劃見 [docs/PLAN.md](docs/PLAN.md)。

目前進度：**Phase 3 進行中** — 應用已改版為兩層章節式的閱讀介面（像一本書，
而不是一份可篩選的清單）。職業與專長兩章可讀，另有 Ctrl+K 全域跳轉與兩套主題；
種族、物品、創角規則、附錄四章施工中。設計與計畫見
[docs/superpowers/specs/](docs/superpowers/specs/) 與 [docs/superpowers/plans/](docs/superpowers/plans/)。

Phase 2 已完成：31 張工作表中的 30 張納入 SQLite，規則層級疑問於 2026-09-20 由作者
全數裁示（見 [docs/規則裁示紀錄.md](docs/規則裁示紀錄.md)）。

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
| `data/errata/` | 我們對原始資料的修正，連同理由（進 git） |
| `data/layout/` | 版面宣告：職業表的區塊位置、散文層級、對照表範圍 |
| `db/schema.sql` | SQLite 結構定義 |
| `dist/d100.db` | 建置產物，**不進 git**，隨時可重建 |
| `tools/` | 資料管線工具鏈 |
| `app/` | Tauri + React 規則書應用（見 [app/README.md](app/README.md)） |
| `docs/PLAN.md` | 開發計劃 |
| `docs/勘誤清單.md` | 自動產生，可直接拿去跟規則書作者對帳 |
| `docs/規則裁示紀錄.md` | 規則層級疑問與作者的裁示（手寫） |
| `docs/裝備欄位對應.md` | 詞綴部位 → 裝備欄位的對應與裁示依據 |

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
python tools/verify_extract.py        # 逐格驗證擷取無損
python tools/diff_report.py           # 工作目錄 vs HEAD
python tools/build_db.py              # 建置 dist/d100.db
python tools/validate.py              # 驗證資料庫與規則公式
python tools/errata_report.py --out docs/勘誤清單.md
```

完整重建：

```bash
python tools/extract.py && python tools/verify_extract.py && python tools/build_db.py && python tools/validate.py
```

## 執行應用

```bash
cd app
npm install
npm run sync-db
npm run tauri dev
```

首次建置要編譯 Rust 相依，約需一到兩分鐘。需要 Node.js 與 Rust 工具鏈，
Windows 上另需 WebView2 Runtime 與 MSVC C++ Build Tools。

## 修正規則書的錯誤

原始 xlsx 永遠不動。修正寫在 `data/errata/<工作表名>.yaml`：

```yaml
sheet: 高級專長
entries:
  - row: 19
    reason: 為什麼要改（必填，這段會出現在給作者看的勘誤清單上）
    set:
      difficulty: 6
  - row: 11
    flag: incomplete_entry      # 不確定正確答案時只標記，不臆測
    covers: [missing_effect]    # 宣告已涵蓋哪些自動偵測的問題
    reason: ...
  - row: 14
    target: material_affix      # 同一列對應多種紀錄時指明要改哪一個
    set:
      roll_max: 70
    reason: ...
```

重跑 `build_db.py` 即生效。所有勘誤連同理由都會寫進資料庫的 `errata` 表，
再由 `errata_report.py` 整理成清單。

## 設計原則

`data/raw` 是 xlsx 的**忠實機械映射**，不做任何清理、欄位對齊或語意判斷
（那些屬於 Phase 2 的 SQLite 建置階段）。這樣才能把「上游試算表真的改了」
和「我們的解析邏輯改了」區分開來 —— 前者反映在 `source.sha256`，
後者反映在 `tool_version`，變更報告會分別標示。

擷取具備可重現性：同一份 xlsx 重跑任意次數，產出的每一個位元組都相同，
且 `verify_extract.py` 會逐格比對確認沒有漏字。資料庫同樣可重現：
刪掉 `dist/d100.db` 重建，內容完全一樣。

解析不出來的東西一律留 NULL 並記一筆勘誤，**絕不臆測**。原始字串
（`difficulty_raw`、`plus_cost_raw`、`prereq.raw_text`）一律保留。
