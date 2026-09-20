# D100 規則書 / 角色卡應用 — 開發計劃

> 來源：`D100專長表.xlsx`（31 個工作表，約 465 KB 有效文字）
> 定案日期：2026-09-20

## 技術決策

| 項目 | 決定 |
|---|---|
| 應用技術棧 | Tauri + React + TypeScript |
| 資料真實來源 | git 內的文字檔（YAML / TSV）；SQLite 為**建置產物**，隨時可重建 |
| 規則驗證強度 | 軟性警告，違規可覆寫並標記，不阻擋存檔 |
| 協作需求 | 角色卡 JSON 匯入匯出（不做多人同畫面、不做 xlsx/PDF 匯出） |

## 原始試算表盤點

| 類別 | 工作表 | 解析難度 |
|---|---|---|
| 規則說明 | 創角色須知、施法者創角須知、戰鬥流程、世界觀、大陸簡史、FATE特規、Patch note | C（散文，原樣保存） |
| 專長 | 基本專長(33)、一般專長(103)、製作專長(52)、超魔專長(17)、高級專長(11)、傳奇專長 | A–B |
| 職業／流派 | 法師學派專精、術士、牧師領域、德魯伊結社、吟遊詩人特殊專長、特殊職業1/2/武僧/warlock | C（卡片式版面） |
| 物品系統 | 魔法物品價格表、詞墜(依物品分類)、一般詞綴(40)、高階詞綴(18)、素材詞綴、永恆聖器詞綴 | A–C |
| 其他 | 種族與其調整(11)、狩獵任務(含 250 條公式的計算器)、法師範例(角色卡樣板) | A / 需移植為程式碼 |

**難度分級**
- **Tier A**：接近乾淨表格 — 種族、基本／一般／高級專長、一般／高階／永恆聖器詞綴
- **Tier B**：前言區塊 + 表格 + 續行列 — 超魔／製作／傳奇專長、素材詞綴
- **Tier C**：自由版面，1300+ 合併儲存格 — 職業／流派 9 張、物品價格表、詞墜分類、狩獵任務、各散文表

## 規則骨架（Phase 4 數學基礎）

- 九大屬性：STR / DEX / SKI / CON / RES / INT / WIS / CHA / SPI，4D6 擲骰
- 屬性調整值：13 為 0，每 ±2 一階（7→-3 … 19→+3）；調整值總和超過 10 每點扣 10 CP，低於 10 每點加 10 CP
- 六大技能判定
  - 戰鬥 = DEX + SKI + STR
  - 運動 = DEX + SKI + CON
  - 操作 = INT + SKI + WIS
  - 感知 = INT + RES + SPI
  - 知識 = (INT + WIS) × 1.5
  - 交涉 = CHA + WIS + SPI
- 五抗性：抗毒素 = RES+CON、抗控制 = RES+WIS、抗轉化 = RES×2、抗噴吐 = RES+DEX、抗魔法 = RES+INT
- 三特殊（不計調整值）：強韌 = CON×5、精神 = RES×5、靈魂 = SPI×5
- CP 消耗：`2^等級 × 難度`；等級 0 亦需付費（免除該技能判定 20% 減值）
- 獎勵 HP/SP 門檻：10 / 30 / 70 / 150 / 300 / 620 / 1020 CP，進戰軌與法術軌可累算
- 額外 HP/SP 轉換：基礎值每多一倍，換匯率 +1（1x–2x 為 1:1，2x–3x 為 2:1，依此類推）
- 施法者等級 = 本身環數 × 3 + 變動值
- 法術位 = 環數 × 2 + 主屬性調整值 + 魔法物品變動值 + 技能變動值
- 施法檢定 = 主屬性 + 10×(總施法者等級/3) − 連續施法懲罰 − 本次環數×10
- 擲骰結果小數 ≥ 0.5 一律進位（Patch 1.1）

## 已知資料問題（Phase 2 處理）

1. 「一般詞綴」〈光亮〉：部位文字換行溢到下一列，造成後續欄位整列左移一格
2. 「一般專長」分類欄混用 `感知/知識` 與 `知識/感知`、`戰鬥/運動` 等自由文字
3. 高級／製作等表的效果說明溢成僅有 A 欄的續行列（〈沼澤人儀式〉佔 13 列），無任何標記可區分新條目與續行
4. 職業表（術士 36 欄、牧師領域 27 欄）為並排區塊排版，無可靠表頭
5. 前置條件為自由文字，混用「施法環數達四環以上」「綁定血脈:神魔裔」「跑步二級」三種格式
6. 全表零公式（狩獵任務除外），所有衍生數值靠人工計算

---

## Phase 1 — 資料固化與變更追蹤

**目標**：xlsx 更新時，`git diff` 能直接指出哪一條的哪個欄位改了。

- [ ] `data/source/D100專長表.xlsx` 納管，記錄 SHA-256
- [ ] `tools/extract.py`：純機械式轉換（零判斷），每張表 → `data/raw/<NN>_<表名>.tsv`
      規則：None → 空字串、換行 → `\n`、去除尾端空欄、去除尾端空列
      驗收：同一份 xlsx 重跑必須逐 byte 相同
- [ ] `data/raw/manifest.json`：每張表的列數／欄數／內容雜湊
- [ ] `tools/diff_report.py`：比對兩版 manifest + TSV，輸出中文變更報告
- [ ] pre-commit hook：xlsx 變更時強制重跑 extract，避免 raw 與 xlsx 不同步

**原則**：Phase 1 不做任何清理。raw 層只負責忠實映射，清理留到 Phase 2，
才能區分「上游真的改了」與「我們的解析邏輯改了」。

## Phase 2 — SQLite 規則資料庫與勘誤

**目標**：從 raw 產出正規化、可查詢、可驗證的資料庫；錯誤記錄下來而非偷偷修掉。

排程依 Tier 進行：Tier A → Tier B → Tier C（Tier C 採半自動：程式切區塊，人工在 YAML 覆蓋檔校正）。

**Schema 草案**

```
feat(id, name, source_sheet, category, difficulty, effect_md, max_level, notes)
feat_prereq(feat_id, kind, ref_feat_id, min_level, raw_text)   -- kind: feat|caster_ring|bloodline|attr|free
feat_category(feat_id, category_code)                          -- 拆解「戰鬥/運動」多分類
race(id, name, cp_cost, special_md)
race_modifier(race_id, target_type, target, delta)
class_path(id, class, kind, name, desc_md)                     -- 學派/血脈/領域/結社 統一模型
affix(id, name, tier, slots, plus_cost, effect_md)
item_slot / item_price / material_affix / hunt_table
rule_text(id, sheet, section, order, body_md)                  -- 散文規則原樣保存
errata(id, table, row_ref, field, raw_value, fixed_value, reason, decided_by, decided_at)
schema_version / import_run(xlsx_hash, at, tool_version)
```

- [ ] `tools/build_db.py`：raw + `data/errata/*.yaml` → `dist/d100.db`
- [ ] `tools/validate.py`：前置專長存在性、難度為正數、分類白名單、詞綴部位白名單、CP 公式抽查；失敗即中斷建置
- [ ] 前置條件解析器：解析不出的標 `kind=free`，保留 `raw_text` 僅顯示不驗證

**勘誤原則**：DB 為建置產物，所有修正寫在 git 內的 YAML，永遠可重現、可追溯、可回推給規則書作者。

## Phase 3 — 規則書瀏覽／編輯應用（Tauri）

- [ ] 全文搜尋（專長名／效果內文）＋ 分類、難度、來源多重篩選
- [ ] 專長詳情頁：前置鏈往上追、被誰當前置往下追、等級 0–5 CP 成本表
- [ ] 新增／修改／刪除：編輯一律寫成 errata / override YAML 並進 git，附變更歷史
- [ ] 版本比對頁：接 Phase 1 的 diff 報告，顯示版本間差異
- [ ] 規則散文頁（創角須知、戰鬥流程等）以 Markdown 呈現

## Phase 4 — 角色卡建立與跑團模式

**建卡精靈**
- [ ] 起始 CP → 背景／種族（自動扣 CP）→ 擲 9 組 4D6 或手動輸入
- [ ] 自動計算六大技能、五抗性、三特殊
- [ ] 專長分配：即時扣 CP、前置檢查、餘額顯示
- [ ] 獎勵 HP/SP 門檻自動觸發（進戰軌 + 法術軌）
- [ ] 額外 HP/SP 累進換匯
- [ ] 魔法物品與詞綴配置，檢核加值兌換上限

**施法者分支**
- [ ] 環數、法術位、記憶法術位、施法檢定值、連續施法懲罰追蹤

**跑團模式**
- [ ] HP / SP / 法術位即時增減
- [ ] 狀態與 buff 管理、輪次計時
- [ ] 擲骰器（含 0.5 進位規則、CP 重骰）
- [ ] 行動順序（DEX）／宣告順序（INT）表，含同值判定

**存檔**
- [ ] 角色卡 JSON 匯入匯出

**驗證行為**：所有規則衝突以警告呈現，可覆寫，覆寫項目在角色卡上標記。

---

## 風險

1. **Tier C 職業表是排版而非資料**。硬寫解析器脆弱且痛苦。建議一次性人工轉成 YAML（9 張表），之後由 App 維護不再回頭解析。代價：Phase 1 的 diff 對這幾張表只能提示「這區有變動」，細節仍需人眼確認。
2. **前置條件文字化**是 Phase 4 自動驗證的瓶頸。採漸進策略：`raw_text` 與結構化欄位並存，解析率逐步提升。
3. **Phase 1 的 diff 品質決定後三個 Phase 的成本**。不應為趕進度跳過 manifest 與可重現性設計。
