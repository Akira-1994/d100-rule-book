# 規則書應用改版 — 實作計畫

> 對應設計：[2026-09-21-rulebook-ui-redesign-design.md](../specs/2026-09-21-rulebook-ui-redesign-design.md)
> 分支：`feat/rulebook-ui-redesign`

## 起點與原則

**起點**：`db.rs` 621 行、`App.tsx` 408 行、`App.css` 477 行，
四個指令（`build_info` / `facets` / `search_feats` / `feat_detail`），
六個跑在真實 `dist/d100.db` 上的測試。

**每個階段結束時應用必須可跑。** 不容許「中間這幾個 commit 是壞的，
最後才會好」。這決定了任務順序：先讓新骨架能跑起來並承接舊功能，
再一章一章補上，最後才移除舊介面。

**每個任務結束時跑**：

```bash
cd app && npm run build && cd src-tauri && cargo test --lib
```

前端型別與 Rust 測試都過才算完成。`cargo test` 直接跑在真正的資料庫上 ——
造假資料在這裡沒有意義，要驗的正是「查詢能不能對付這份資料的形狀」。

---

## 階段 1 — 骨架與核心兩章

目標：新介面能取代舊介面。做完這一階段，舊的搜尋／篩選頁就整個刪掉。

### 1.1 後端：拆檔，先不改行為

把 `db.rs` 拆成模組，**純搬移，不改任何查詢邏輯**，
現有六個測試必須原封不動全過。

```
src/db/mod.rs      Db、locate、open、build_info、共用 helper（preview、numbered、categories_of）
src/db/feat.rs     FeatSummary、FeatDetail、Prereq、Dependent、feat_detail、cp_table
src/db/search.rs   search_feats、facets（此階段稍後汰換）
```

**驗收**：`cargo test --lib` 六個測試全過，`npm run tauri dev` 行為與改版前一致。

> 先拆檔再改行為，是為了讓後面每個 commit 的 diff 只包含真正的邏輯變動。
> 搬移與改寫混在同一個 commit 裡，review 時分不出哪些是手滑。

### 1.2 後端：`toc` 指令

回傳整棵章節／頁籤樹與各頁條目數，啟動時呼叫一次。

- 新檔 `src/db/toc.rs`
- 章節與頁籤的定義**寫死在 Rust**（不進資料庫）—— 這是編輯決定，
  不是規則資料；哪一章放哪些工作表是我們的判斷，不是試算表的內容
- 條目數由查詢算出，不寫死

**驗收**：新增測試 `目錄涵蓋六個章節且條目數非零`，逐章檢查條目數 > 0。

### 1.3 後端：`class_chapter(class_name)`

一次拿到某職業整章的資料：流派（含描述）、每個流派底下的專長
（依 `source_row` 排序）、被動特性、Warlock 的祈喚。

- 新檔 `src/db/class.rs`
- 回傳巢狀結構 `ClassChapter { class_name, path_kind, paths: [ClassPath { name, description, feats, traits }], invocations }`
- 一次查完整章，不做 N+1：用三個查詢（流派、專長、特性）在 Rust 端組裝

**驗收**：
- `class_chapter("Warlock")` 拿到 12 個契約、41 條專長、58 條祈喚
- `class_chapter("法師")` 拿到 9 個學派，「防護」底下 5 條專長且首條為〈奧術防禦〉
- `class_chapter("神掌門")` 的 9 條被動特性全部出現在 `traits` 而非 `feats`

### 1.4 後端：`feat_chapter(group)`

非職業的六組專長，依分類分節。一個專長可屬多個分類
（`feat_category` 是多對多），因此**同一條可能出現在多個分節** ——
這是刻意的，「戰鬥/運動」的專長在兩節都該找得到。

**驗收**：`feat_chapter("general")` 的分節聯集涵蓋全部 103 條；
多分類的條目確實出現在每一個所屬分節。

### 1.5 後端：`entry_detail(kind, id)`

展開區內容：前置鏈（含 `kind` 標籤）、被誰當前置、來源工作表與列號。

- 從現有 `feat_detail` 改寫：**移除 `cp_table` 與 `errata` 兩個欄位**
- `kind` 參數預留給 `class_trait` / `invocation` / `affix`，此階段只實作 `feat`

**驗收**：改寫既有測試 `前置鏈雙向都查得到`（保留斷言，移除 CP 與勘誤的部分）。

### 1.6 後端：`search` 取代 `search_feats` / `facets`

跨型別搜尋，每筆結果帶 `{chapter, tab, anchor}` 位址。

- 沿用現有的排序規則：名稱命中排在效果命中前面
- 此階段涵蓋 `feat` / `class_trait` / `invocation`；
  `affix` / `race` / `rule_text` 留到階段 2，該章節存在之後才有地方跳
- 刪除 `search_feats` 與 `facets`，連同測試
  `關鍵字同時比對名稱與效果`、`無關鍵字時篩選仍然正確`

**驗收**：新測試 `搜尋結果帶得出可跳轉的位址` ——
搜「奧術防禦」回傳 `{chapter:"職業", tab:"法師", anchor:<feat id>}`；
新測試 `搜尋涵蓋非專長型別` —— 搜一條祈喚名稱找得到且 tab 為 Warlock。

### 1.7 前端：主題與位址模型

- `src/theme.css` —— 設計文件第四節的兩套變數，`:root` 為羊皮紙暖白，
  `[data-theme="dark"]` 為燭光皮革，另以 `prefers-color-scheme` 設預設
- `src/nav.ts` —— 位址型別 `Address = {chapter, tab, anchor?}`、
  序列化（供未來做深連結）、以及 `useNavigation()` 狀態
- `src/components/ThemeToggle.tsx` —— 切換並寫入 `localStorage`

**驗收**：切換主題時只有顏色變化，版面不位移；重開應用記得上次選擇。

### 1.8 前端：外殼

`App.tsx` 縮減為外殼：章節列、頁籤列、主題切換、Ctrl+K 掛載、
以及依 `Address` 決定渲染哪個 chapter 元件。**目標行數 120 行以內。**

**驗收**：切章切頁不重新查 `toc`；頁籤列在 1280px 下不溢出（職業 14 個頁籤需可換行）。

### 1.9 前端：`ClassChapter` 與共用元件

- `src/components/FeatCard.tsx` —— 專長卡、就地展開、前置鏈連結
- `src/components/SectionNav.tsx` —— 右側小目錄，捲動高亮用
  `IntersectionObserver`（非 scroll 事件，避免逐幀重算）
- `src/chapters/ClassChapter.tsx` —— 流派依序往下捲，兩欄卡片網格

**驗收**：
- 法師頁 9 個學派全部渲染，右側目錄點擊可跳轉且高亮跟隨
- 點〈奧術防禦〉展開後，「被這些當前置」的連結可跳到〈進階防護〉
- 兩欄在窄視窗（< 900px）自動降為單欄

### 1.10 前端：`FeatChapter` 與 `CommandPalette`

- `src/chapters/FeatChapter.tsx` —— 六個頁籤，依分類分節
- `src/components/CommandPalette.tsx` —— Ctrl+K 覆蓋層，
  上下鍵選擇、Enter 跳轉、Esc 關閉，選中後高亮目標 1.5 秒

**驗收**：Ctrl+K 搜「每輪回復」找得到效果內文命中的條目；
選中後正確切到該條目所在章節並捲到位置。

### 1.11 移除舊介面

刪掉 `App.css` 裡舊清單／篩選器的樣式與 `api.ts` 中已無用的型別。

**驗收**：全域搜尋 `search_feats`、`facets`、`FeatSummary`、`Facets`
在 `app/` 下零結果；`npm run build` 無未使用變數警告。

---

## 階段 2 — 其餘四章

### 2.1 種族章節
`race` + `race_attr_modifier`，13 個種族。
〈原初-星之幼體〉的 `cp_cost` 為 NULL、`cp_raw` 是「劇情取得」——
顯示原始字串並註明「由 DM 建卡時裁定」（依 2026-09-20 裁示）。

**驗收**：13 個種族全數渲染；非數值 CP 不顯示為空白或 0。

### 2.2 物品章節
四個頁籤：詞綴（三個階級分節）、素材、價格表、詞綴分布。

- 詞綴卡顯示部位（`affix_slot`）與加值兌換階級（`affix_rank`）；
  `condition` 非空時標明是依裝備類型而異的價碼，**不是另一個階級**
- 素材頁為父子版面：素材 → 其詞綴與 D100 區間
- `src/components/RefTableView.tsx` 渲染 `ref_table`／`ref_row`
  （欄名與儲存格都是 JSON 陣列）
- 詞綴分布 685 筆依「部位 × 加值」分組

**驗收**：〈幽冥〉的兩筆 `condition` 顯示為同一階的兩種價碼；
12 張魔法物品價格表全部渲染。

### 2.3 創角規則章節與 `ProseChapter`
`rule_text` 的 `section` / `subsection` 兩層標題。
戰鬥流程是兩層巢狀（大步驟底下再分一般／自由／即時動作）。

**驗收**：戰鬥流程 19 段的層級正確，子項不被當成大步驟。

### 2.4 附錄章節
世界觀、大陸簡史、Patch note、狩獵任務、對照表總覽、勘誤清單。

勘誤清單在這裡完整呈現（38 筆，含理由），這是它唯一該出現的地方。

**驗收**：`errata` 38 筆全數列出並可依工作表分組。

### 2.5 搜尋擴充
把 `affix` / `material_affix` / `race` / `rule_text` 納入 `search`。

**驗收**：搜一個詞綴名稱可跳到物品章節對應位置。

---

## 階段 3 — CP 試算工具

### 3.1 `src-tauri/src/rules.rs`
把 `db/feat.rs` 裡的 `cp_table` 抽出來獨立成模組，對齊 `tools/rules.py`：

- `cp_for_level(level, difficulty)` = `2^level × difficulty`
- `cp_cumulative(level, difficulty)` —— 等級 0 不計入累積
- `difficulty_scale = "legend"` 時換算為 CP（1 傳奇點 = 10 CP）

**驗收**：保留並擴充既有測試 `cp表符合規則書範例`
（〈武器使用〉難度 1 學到等級 3 為 14 點），
新增傳奇計價的測試，並在檔頭註明與 `tools/rules.py` 的對應關係。

### 3.2 `cp_table` 指令與試算頁
`src/chapters/AppendixChapter.tsx` 底下的試算頁：
輸入難度或從清單挑專長 → 列出各級成本與累計。
`1or2` 這類條目讓使用者自選；傳奇專長顯示兩種計價。

**驗收**：挑〈武器使用〉學到 3 級顯示 14 點；
挑一條 `difficulty_scale=legend` 的傳奇專長顯示傳奇點與等值 CP。

---

## 風險與對策

1. **一次查整章的資料量**。法師整章含 41 條專長的效果全文，
   單次回傳約數十 KB —— 可接受，且切頁時才查。
   若牧師（60 條）明顯延遲，再改為流派層級的延遲載入。
2. **兩欄網格的高度落差**。相鄰卡片效果長度差距大時右欄會留白。
   先用 `align-items: start` 接受落差；若實際觀感不佳再考慮 CSS columns
   （代價是閱讀順序變成直向，需另外驗證）。
3. **拆檔造成的 merge 衝突**。此分支獨佔 `app/`，
   拆檔任務（1.1）盡早完成並單獨 commit。

## 明確不做

- 不動 `data/`、`db/schema.sql`、`tools/` 任何一個檔案
- 不做編輯功能、版本比對頁、規則驗證
- 不加第三方 UI 框架
