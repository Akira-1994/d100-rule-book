# 勘誤編輯功能設計 — 在應用裡改規則書

> 定案日期：2026-09-22
> 影響範圍：`app/`（新增編輯層）。`data/errata/*.yaml` 由程式追加內容，
> 但格式不變；`tools/` 與 `db/schema.sql` 完全不動。
> 前置狀態：規則書閱讀介面三個階段皆已完成

## 要解決的問題

規則書會改。房規是活的 —— 難度調整、效果改寫、分類補正，這些現在都得離開
應用、打開 YAML、手寫一筆勘誤、重跑建置。流程本身是對的（修正進 git、
可追溯、可回推給作者），只是沒有介面。

這次做的就是那個介面，**流程一步都不變**。

## 一、只在開發模式存在

編輯功能在打包版完全不出現 —— 不是隱藏，是根本不編譯進去。

理由是這個專案的核心前提：**SQLite 是建置產物，真實來源是 git 裡的 YAML**。
編輯必然要寫 repo 裡的檔案、必然要重跑 `tools/build_db.py`，這兩件事只有在
有 repo、有 Python 的機器上才成立。玩家機器上沒有這些東西。

曾經考慮過在使用者端做一層覆寫（overrides.yaml + 合成出 working.db），
可以讓玩家也改。那條路可行但代價高：要另外維護一份與 errata 平行的格式、
要處理匯入衝突、而且 `prereq_raw` 這類需要 Python 解析器的欄位永遠套不了。
決定不走。

判定方式用 `cfg!(debug_assertions)`：`tauri dev` 為真，`tauri build` 為假。

## 二、流程

```
App 的編輯表單
   ↓ 追加一筆到 data/errata/<工作表>.yaml
   ↓ 呼叫 python tools/build_db.py
   ↓ 重新開啟 dist/d100.db
畫面更新
```

三個步驟都可能失敗，失敗一律回報原文而不是「發生錯誤」：

- **寫檔失敗**（權限、磁碟）→ 不重建，狀態不變
- **重建失敗** → `build_db.py` 的 stderr 原樣顯示。最常見的是勘誤過期
  （`找不到 <工作表> 第 N 列，勘誤可能已經過期`），那通常表示上游試算表
  的列號變動了，需要人看
- **重新開啟失敗** → 提示重開應用

重建期間資料庫連線要先關閉：Windows 上檔案被開著時 `build_db.py` 寫不進去。

## 三、編輯永遠是追加一筆勘誤

**從不修改既有的 YAML 條目，只在檔尾追加。** 兩個理由：

1. 既有檔案裡有大量手寫註解與 `>-` 區塊字串（見 `data/errata/高級專長.yaml`
   的檔頭說明）。用 YAML 程式庫重寫整個檔案會把註解全部抹掉。
2. 追加天然形成歷史。要改回去就再追加一筆。

這在語意上成立，因為 `apply_errata` 是**依序套用、後蓋前**的：
`load_errata` 以 `sorted(glob)` 讀檔、entry 保持文件順序，`apply_errata`
逐筆執行 `record[field] = value`。同一列同一欄位被設定兩次時，後面那筆勝出。

檔案不存在時先寫入一段檔頭（`sheet:` 與 `entries:`），格式與現有檔案一致。

### 必須帶上 `col:` 的情況

Tier C 的職業表並排多個區塊，同一個 `source_row` 可能對應好幾個條目
（`apply_errata` 對這種情況要求以 `col:` 指明）。App 手上本來就有每個條目的
`source_col`，**寫入時一律帶上 `col:`**，不去判斷「這次需不需要」——
判斷錯的代價是勘誤套到隔壁欄的條目上，而多寫一個欄位沒有任何壞處。

## 四、可編輯的欄位

以現有 errata 已經用過的八個欄位為準，不自行擴充：

| 欄位 | 適用 | 說明 |
|---|---|---|
| `difficulty` / `difficulty_raw` | 專長 | 數值與原始字串並存，兩者要一起改 |
| `effect` | 專長、詞綴 | 效果敘述 |
| `categories` | 專長 | 多選，值域限定六大技能分類 |
| `tags` | 專長 | 多選，自由輸入 |
| `prereq_raw` | 專長 | 前置條件原文 |
| `parent` | 專長 | 父項名稱，`build_db.py` 會解析成 `parent_id` |
| `roll_min` / `roll_max` | 素材詞綴 | D100 區間，需搭配 `target: material_affix` |

`prereq_raw` 能支援正是走 Python 管線的好處 —— 前置解析器還在，改完重建就會
重新解析成 `feat_prereq` 列。使用者端覆寫方案做不到這件事。

`flag`（只標記不改值）一併支援，那是「我不確定正確答案」時該用的動作。

**理由（`reason`）為必填。** `load_errata` 本來就會在缺少時直接中止建置；
介面也不該讓人省略 —— 這些檔案存在的意義就是拿去跟規則書作者對帳。

## 五、變更歷史就是 git

不自己做歷史紀錄。`data/errata/` 在 git 裡，歷史已經有了。

附錄新增「修改紀錄」頁籤，顯示兩段：

- **尚未提交** —— `git diff -- data/errata` 的內容
- **已提交** —— `git log --oneline -- data/errata` 最近數十筆

**不自動 commit。** App 只改檔案，`git add/commit` 由人決定 ——
自動提交會讓一次誤按變成歷史的一部分。頁面上放一段可複製的指令即可。

## 六、程式結構

```
src-tauri/src/
  editing/mod.rs      可編輯欄位白名單、是否啟用（cfg!(debug_assertions)）
  editing/yaml.rs     追加一筆勘誤到 data/errata/<工作表>.yaml
  editing/rebuild.rs  呼叫 python tools/build_db.py 並回報 stderr
  editing/history.rs  git diff / git log 的讀取
  db/mod.rs           連線改為可關閉重開（重建期間必須放開檔案）
src/
  components/EditDialog.tsx   單一條目的編輯表單
  chapters/HistoryChapter.tsx 附錄的「修改紀錄」頁籤
```

新增 Rust 相依 `serde_yaml`（只用來產生單筆條目的 YAML 片段，不重寫檔案）。

### 連線的生命週期

現在是啟動時開一條連線常駐到結束。重建需要先關閉它，因此 `Db` 從
`Mutex<Connection>` 改為 `Mutex<Option<Connection>>`：重建前 `take()` 丟棄
連線，重建後重新開啟。所有查詢多一層「連線不在」的錯誤處理，那個狀態只在
重建的短暫期間出現。

### 前置條件檢查

Python 與 PyYAML 不在時，編輯按鈕停用並說明原因，而不是按下去才失敗。
檢查在啟動時做一次（`python -c "import yaml"`），結果隨 `build_info` 一起回傳。

## 七、測試

- `editing/yaml.rs`：追加到暫存目錄的檔案，驗證產出可被 `yaml` 解析、
  且 `col:` 與 `reason` 都在
- `editing/mod.rs`：白名單拒絕不在表上的欄位
- 重建與 git 讀取不寫單元測試 —— 它們是薄薄的子行程包裝，
  真正會錯的是外部指令本身，用假資料驗不出什麼

前端沿用既有做法：型別由 `tsc` 把關，行為靠實際操作驗證。

## 八、明確不做

- **不新增／刪除條目。** errata 格式表達不了「這條試算表裡沒有的專長」，
  硬要支援得新定義一種格式，而那會讓匯出的檔案不再是合法的 errata
- **不做規則驗證**，只做型別檢查（難度必須是數字）——與 Phase 4 的
  「軟性警告、可覆寫」一致
- **不自動 git commit**
- **不碰** `data/raw`、`db/schema.sql`、`tools/` 任何一個檔案
