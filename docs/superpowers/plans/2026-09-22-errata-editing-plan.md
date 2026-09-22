# 勘誤編輯功能 — 實作計畫

> 對應設計：[2026-09-22-errata-editing-design.md](../specs/2026-09-22-errata-editing-design.md)
> 分支：`feat/errata-editing`

## 起點與原則

**起點**：閱讀介面三個階段已完成，16 個指令、33 個 Rust 測試、零警告。
`Db` 是啟動時開一條連線常駐到結束。

**每個階段結束時應用必須可跑**，與前次改版相同。這次多一條：

**不得在測試中寫入 `data/errata/`。** 那是 repo 裡的真實資料，測試污染它
會讓 `git status` 變髒、也可能讓下一次建置失敗。寫檔相關的測試一律在暫存
目錄進行。

**每個任務結束時跑**：

```bash
cd app && npm run build && cd src-tauri && cargo test --lib
```

---

## 階段 1 — 後端

做完這一階段沒有任何畫面變化，但指令都能用、測試都過。

### 1.1 連線改為可關閉重開

`build_db.py` 要覆寫 `dist/d100.db`，Windows 上檔案被開著就寫不進去，
因此重建前必須放開連線。

- `Db(Mutex<Connection>)` → `Db(Mutex<Option<Connection>>)`
- `with_conn` 在連線不存在時回報「資料庫正在重建，請稍候」
- 新增 `Db::close()` 與 `Db::reopen(path)`

**純重構，不改任何查詢邏輯。** 現有 33 個測試必須原封不動全過。

> 與前次改版的 1.1 同一個道理：先把會擴散到既有程式碼的改動單獨做完、
> 單獨 commit，後面每個 commit 的 diff 才只含新功能。

### 1.2 `editing/mod.rs`：白名單與啟用判定

- `pub fn editing_enabled() -> bool { cfg!(debug_assertions) }`
- 可編輯欄位白名單，依條目種類分組（設計文件第四節那張表）
- `pub fn validate(kind, field, value) -> Result<(), String>`：
  型別檢查而已 —— 難度必須是數字、分類必須在六大技能分類內

**驗收**：測試 `白名單拒絕表外的欄位`、`難度必須是數字`。

### 1.3 `editing/yaml.rs`：追加一筆勘誤

寫入 `data/errata/<工作表>.yaml`：檔案存在就追加到末尾，不存在就先寫檔頭。

- **一律帶 `col:`**（設計文件第三節）
- `reason` 必填，空白直接拒絕而不是寫一個空字串進去
- 多行字串用 `>-` 區塊，與現有檔案的風格一致
- 產出前先確認目標檔案能被 `yaml.safe_load` 解析；追加後再驗一次，
  失敗則還原檔案 —— 寫壞 errata 會讓整個建置停擺

**驗收**：
- 測試在暫存目錄追加一筆，讀回來確認 `row`、`col`、`reason`、`set` 都在
- 測試對不存在的檔案追加，確認產出含 `sheet:` 檔頭
- 測試空白 `reason` 被拒絕

### 1.4 `editing/rebuild.rs`：重跑建置

```
關閉連線 → python tools/build_db.py → 重新開啟 → 回報結果
```

- 帶 `PYTHONIOENCODING=utf-8` 執行。Windows 的預設編碼會讓中文 stderr
  變成亂碼，而 `build_db.py` 的錯誤訊息全是中文 —— 亂碼的錯誤等於沒有錯誤訊息
- 失敗時回傳 stderr 原文。最常見的是「找不到 <工作表> 第 N 列，勘誤可能
  已經過期」，那要讓人看到原句
- **無論成功失敗都要把連線重新開起來**，否則一次失敗會讓應用再也查不了

**驗收**：不寫單元測試（薄薄的子行程包裝，真正會錯的是外部指令本身）。
以階段 2 的手動驗收涵蓋。

### 1.5 `editing/history.rs`：讀 git

- `git diff -- data/errata`（尚未提交）
- `git log --oneline -20 -- data/errata`（已提交）
- git 不在或不是 repo 時回空並註明原因，不讓整頁失敗

**驗收**：不寫單元測試，理由同上。

### 1.6 指令與前置條件

新增指令：`append_errata`、`rebuild_db`、`errata_history`。

`build_info` 增加三個欄位：`editing_enabled`、`python_ok`、`python_hint`。
啟動時跑一次 `python -c "import yaml"` 判定 —— 讓按鈕在按下去之前就知道
自己能不能用。

**驗收**：`build_info` 在開發模式回 `editing_enabled: true`；
指令數由 16 增為 19，`api.ts` 與 `lib.rs` 兩邊名稱對應。

---

## 階段 2 — 前端

### 2.1 `EditDialog.tsx`

單一條目的編輯表單：列出該條目可編輯的欄位、現值、理由輸入框。

- 只送出**有改動**的欄位。沒動的欄位不該出現在勘誤裡，否則清單上會是
  一堆「把 X 改成 X」的雜訊
- 理由沒填就不能送出
- 送出後顯示重建進度，失敗時原樣顯示 stderr

### 2.2 卡片的編輯入口

展開區多一個「編輯」按鈕，`editing_enabled` 為假時整個不渲染。
`python_ok` 為假時渲染為停用並顯示 `python_hint`。

### 2.3 重建後的畫面更新

重建成功後重新抓 `toc` 與當前章節。位址（章節／頁籤／捲動位置）要保持不變 ——
改完一條專長跳回第一章會很煩。

### 2.4 `HistoryChapter.tsx` 與新頁籤

附錄新增「修改紀錄」頁籤（`kind: "history"`），**只在開發模式出現**：
`db/toc.rs` 依 `editing_enabled()` 決定要不要推這個頁籤。

顯示未提交的 diff 與最近的 commit，附一段可複製的提交指令。

**階段 2 手動驗收**（端到端，這是這個功能唯一真正的驗收）：

1. 開啟應用 → 職業 → 法師 → 防護 → 〈奧術防禦〉→ 編輯 → 難度改 2、填理由
2. 送出 → 重建成功 → 卡片顯示難度 2，且仍停在同一個位置
3. 「修改紀錄」頁籤看得到那一筆 diff
4. 終端機 `git diff data/errata/法師學派專精.yaml` 看得到同一筆
5. `git checkout data/errata` 還原 → 重跑 `python tools/build_db.py` →
   應用重開後難度回到 1

---

## 風險與對策

1. **測試污染 `data/errata/`**。寫檔測試一律用暫存目錄；
   端到端驗收改動的是真實檔案，驗完要 `git checkout` 還原。
2. **重建期間的查詢**。連線被關掉時所有查詢會失敗，前端要把那個狀態
   顯示成「重建中」而不是錯誤紅字。重建約需一兩秒。
3. **勘誤過期**。上游試算表列號變動時，舊勘誤會讓建置直接中止 ——
   這是既有機制刻意的行為（寧可停下來也不要默默套錯），介面只負責把
   原句顯示清楚。

## 明確不做

- 不新增／刪除條目、不做規則驗證、不自動 git commit
- 不碰 `data/raw`、`db/schema.sql`、`tools/`
