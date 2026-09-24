import { useCallback, useEffect, useRef, useState } from "react";

import {
  BuildInfo,
  EditingStatus,
  Toc,
  getBuildInfo,
  getEditingStatus,
  getToc,
} from "./api";
import AppendixChapter from "./chapters/AppendixChapter";
import ClassChapter from "./chapters/ClassChapter";
import FeatChapter from "./chapters/FeatChapter";
import ItemChapter from "./chapters/ItemChapter";
import HistoryChapter from "./chapters/HistoryChapter";
import ProseChapter from "./chapters/ProseChapter";
import RaceChapter from "./chapters/RaceChapter";
import CommandPalette from "./components/CommandPalette";
import ThemeToggle from "./components/ThemeToggle";
import { EditingProvider } from "./editing";
import { Address, revealEntry, sameTab, useNavigation } from "./nav";
import "./theme.css";
import "./layout.css";

/**
 * 外殼：章節列、頁籤列、主題切換、Ctrl+K，以及依位址決定渲染哪一章。
 *
 * 各章節元件彼此不引用 —— 跨章跳轉一律走 nav.ts 的位址模型。
 */
export default function App() {
  const [info, setInfo] = useState<BuildInfo | null>(null);
  const [toc, setToc] = useState<Toc | null>(null);
  const [startupError, setStartupError] = useState<string | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [editing, setEditing] = useState<EditingStatus | null>(null);
  // 重建之後用來強迫章節元件重新抓資料。位址不變，所以捲動位置與所在頁籤
  // 都保持原樣 —— 改完一條專長跳回第一章會很煩。
  const [dataVersion, setDataVersion] = useState(0);
  // 重建期間資料庫連線是關著的，任何查詢都會失敗。與其讓人看到一片紅字，
  // 不如先把會觸發查詢的入口擋住並說明正在做什麼。
  const [rebuilding, setRebuilding] = useState(false);

  const { address, goto, gotoChapter } = useNavigation(toc);

  useEffect(() => {
    Promise.all([getBuildInfo(), getToc(), getEditingStatus()])
      .then(([i, t, e]) => {
        setInfo(i);
        setToc(t);
        setEditing(e);
      })
      .catch((e) => setStartupError(String(e)));
  }, []);

  // 快速鍵的監聽只掛一次，用 ref 讀最新的旗標而不是把它放進相依陣列 ——
  // 否則每次重建開始與結束都要重掛一次監聽器。
  const rebuildingRef = useRef(false);
  rebuildingRef.current = rebuilding;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        // 搜尋會查資料庫，重建期間開了只會拿到錯誤。
        if (!rebuildingRef.current) setPaletteOpen(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // 同一頁內的條目直接捲過去。跨頁的話章節要重查，捲動時機只有章節元件
  // 自己知道（資料到位才有 DOM），所以把 anchor 當 pending 交給它。
  const navigate = useCallback(
    (next: Address) => {
      if (address && sameTab(address, next) && next.anchor) {
        revealEntry(next.anchor);
        return;
      }
      goto(next);
    },
    [address, goto],
  );

  if (startupError) {
    return (
      <div className="startup-error">
        <h1>無法載入規則書</h1>
        <pre>{startupError}</pre>
        <p>
          資料庫是建置產物，不在 git 裡。請在 repo 根目錄執行
          <code>python tools/build_db.py</code>，再重新開啟應用。
        </p>
      </div>
    );
  }

  if (!toc || !address || !editing) {
    return <div className="loading">載入規則書⋯</div>;
  }

  // 重建會改變條目數，目錄要跟著更新。
  const applied = () => {
    getBuildInfo().then(setInfo).catch(() => {});
    getToc().then(setToc).catch(() => {});
    setDataVersion((v) => v + 1);
  };

  const chapter = toc.chapters.find((c) => c.key === address.chapter);
  const tab = chapter?.tabs.find((t) => t.key === address.tab);

  return (
    <EditingProvider
      value={{ status: editing, onApplied: applied, rebuilding, setRebuilding }}
    >
    <div className="app">
      <header className="chapter-bar">
        {toc.chapters.map((c) => (
          <button
            key={c.key}
            className={c.key === address.chapter ? "chapter-tab on" : "chapter-tab"}
            disabled={rebuilding}
            onClick={() => gotoChapter(c.key)}
          >
            {c.title}
          </button>
        ))}
        <div className="bar-tail">
          <button
            className="search-button"
            disabled={rebuilding}
            onClick={() => setPaletteOpen(true)}
          >
            搜尋 <kbd>Ctrl</kbd>
            <kbd>K</kbd>
          </button>
          <ThemeToggle />
        </div>
      </header>

      <nav className="tab-bar">
        {chapter?.tabs.map((t) => (
          <button
            key={t.key}
            className={t.key === address.tab ? "tab on" : "tab"}
            disabled={rebuilding}
            onClick={() => goto({ chapter: address.chapter, tab: t.key })}
          >
            {t.title}
            {t.count > 0 && <i>{t.count}</i>}
          </button>
        ))}
      </nav>

      <main className="page">
        {tab && renderChapter(tab, address, navigate, dataVersion)}
      </main>

      {rebuilding && (
        <div className="rebuild-banner">
          正在重建資料庫⋯ 這段期間查詢會暫停，約一兩秒。
        </div>
      )}

      <footer className="statusbar">
        {info && (
          <>
            {info.source_file} · {info.source_sha256.slice(0, 12)} · 專長 {info.feats}
            、流派 {info.class_paths}、詞綴 {info.affixes}、種族 {info.races}
          </>
        )}
      </footer>

      <CommandPalette
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
        onPick={navigate}
      />
    </div>
    </EditingProvider>
  );
}

/**
 * 依頁籤的 kind 決定渲染哪個章節元件。
 *
 * key 用頁籤代碼，切頁時整個重建而不是沿用舊狀態 —— 否則捲動位置與展開
 * 狀態會跟著跑到新的一頁去。
 */
function renderChapter(
  tab: { key: string; kind: string },
  address: Address,
  navigate: (a: Address) => void,
  /** 資料重建的版本號。併進 key 就能在重建後強迫章節重新抓資料。 */
  version: number,
) {
  const key = `${tab.key}:${version}`;
  switch (tab.kind) {
    case "class":
      return (
        <ClassChapter
          key={key}
          className={tab.key}
          onNavigate={navigate}
          pending={address.anchor}
        />
      );
    case "feats":
      return (
        <FeatChapter
          key={key}
          group={tab.key}
          onNavigate={navigate}
          pending={address.anchor}
        />
      );
    case "race":
      return <RaceChapter key={key} pending={address.anchor} />;
    case "affix":
    case "material":
    case "ref_sheet":
    case "affix_distribution":
      return (
        <ItemChapter
          key={key}
          kind={tab.kind}
          tabKey={tab.key}
          pending={address.anchor}
        />
      );
    case "prose":
      return <ProseChapter key={key} sheet={tab.key} pending={address.anchor} />;
    case "history":
      return <HistoryChapter key={key} />;
    default:
      return <AppendixChapter key={key} kind={tab.kind} />;
  }
}
