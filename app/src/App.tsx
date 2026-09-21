import { useCallback, useEffect, useState } from "react";

import { BuildInfo, Toc, getBuildInfo, getToc } from "./api";
import ClassChapter from "./chapters/ClassChapter";
import FeatChapter from "./chapters/FeatChapter";
import CommandPalette from "./components/CommandPalette";
import ThemeToggle from "./components/ThemeToggle";
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

  const { address, goto, gotoChapter } = useNavigation(toc);

  useEffect(() => {
    Promise.all([getBuildInfo(), getToc()])
      .then(([i, t]) => {
        setInfo(i);
        setToc(t);
      })
      .catch((e) => setStartupError(String(e)));
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen(true);
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

  if (!toc || !address) return <div className="loading">載入規則書⋯</div>;

  const chapter = toc.chapters.find((c) => c.key === address.chapter);
  const tab = chapter?.tabs.find((t) => t.key === address.tab);

  return (
    <div className="app">
      <header className="chapter-bar">
        {toc.chapters.map((c) => (
          <button
            key={c.key}
            className={c.key === address.chapter ? "chapter-tab on" : "chapter-tab"}
            onClick={() => gotoChapter(c.key)}
          >
            {c.title}
          </button>
        ))}
        <div className="bar-tail">
          <button className="search-button" onClick={() => setPaletteOpen(true)}>
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
            onClick={() => goto({ chapter: address.chapter, tab: t.key })}
          >
            {t.title}
            {t.count > 0 && <i>{t.count}</i>}
          </button>
        ))}
      </nav>

      <main className="page">
        {tab?.kind === "class" && (
          <ClassChapter
            key={tab.key}
            className={tab.key}
            onNavigate={navigate}
            pending={address.anchor}
          />
        )}
        {tab?.kind === "feats" && (
          <FeatChapter
            key={tab.key}
            group={tab.key}
            onNavigate={navigate}
            pending={address.anchor}
          />
        )}
        {tab && tab.kind !== "class" && tab.kind !== "feats" && (
          <p className="loading">這一章於階段 2 補上。</p>
        )}
      </main>

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
  );
}
