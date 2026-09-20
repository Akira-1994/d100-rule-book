// 把 tools/build_db.py 產出的資料庫複製進 Tauri 的 resources 目錄。
//
// Tauri 在建置時要求 bundle.resources 列出的檔案必須存在，而資料庫是建置
// 產物、不進 git，所以每次建置前都要先同步一次。開發模式其實會直接讀
// repo 的 dist/d100.db，但 cargo 的 build script 仍會檢查這個路徑。

import { copyFileSync, existsSync, mkdirSync, statSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const source = resolve(here, "..", "..", "dist", "d100.db");
const target = resolve(here, "..", "src-tauri", "resources", "d100.db");

if (!existsSync(source)) {
  console.error(
    `找不到規則書資料庫：${source}\n` +
      `請先在 repo 根目錄執行：python tools/build_db.py`,
  );
  process.exit(1);
}

mkdirSync(dirname(target), { recursive: true });
copyFileSync(source, target);

const kb = Math.round(statSync(target).size / 1024);
console.log(`已同步資料庫（${kb} KB）→ src-tauri/resources/d100.db`);
