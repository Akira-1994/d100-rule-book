# D100 規則書應用

Tauri 2 + React 19 + TypeScript。後端以 Rust 讀 `dist/d100.db`，前端只負責呈現。

## 開發

```bash
cd app
npm install
npm run sync-db      # 把 dist/d100.db 複製進 src-tauri/resources
npm run tauri dev
```

`sync-db` 是必要步驟：Tauri 在建置時會檢查 `bundle.resources` 列出的檔案存在，
而資料庫是 `tools/build_db.py` 的產物、不進 git。規則書內容更新後要重跑：

```bash
python ../tools/build_db.py && npm run sync-db
```

## 打包

```bash
npm run sync-db && npm run tauri build
```

資料庫會一起打包進安裝檔，使用者不需要另外準備。

## 結構

| 路徑 | 說明 |
|---|---|
| `src/api.ts` | Rust 指令的型別化包裝。型別必須與 `src-tauri/src/db.rs` 同步 |
| `src/App.tsx` | 介面：搜尋、篩選、清單、詳情 |
| `src-tauri/src/db.rs` | 資料讀取層與資料契約 |
| `src-tauri/src/lib.rs` | Tauri 指令註冊與啟動流程 |
| `scripts/sync-db.mjs` | 從 `dist/` 同步資料庫 |

## 設計決定

**資料庫唯讀。** Phase 3 之後要加的編輯功能是改 `data/errata` 底下的 YAML
再重建資料庫，而不是直接寫 `.db` —— 直接改會讓修改失去可追溯性，也違背
「SQLite 是建置產物」這個前提。

**搜尋同時比對名稱與效果內文。** 規則書的效果敘述往往才是玩家記得的那一句
（「每輪回復1d10hp」），只比對名稱等於少掉一半的用處。名稱命中會排在
效果命中前面。

**前置鏈是雙向的。** 詳情頁除了列出這個專長需要什麼，也列出「把它當前置的
其他專長」——「我點了這個之後能開出什麼」是建卡時最常問的問題。

**資料庫找不到時讓啟動直接失敗。** 並印出找過哪些路徑，比開起一個每次查詢
都報錯的空視窗容易診斷。
