/**
 * 位址模型。
 *
 * 書裡的每個位置都是一個 `Address`：哪一章、哪一頁、（可選）哪一個條目。
 * 前置鏈跳轉、搜尋跳轉、右側目錄全部走這一套，元件之間不互相呼叫 ——
 * ClassChapter 不需要知道 FeatChapter 存在。
 */

import { useCallback, useEffect, useState } from "react";

import type { Toc } from "./api";

export interface Address {
  chapter: string;
  tab: string;
  /** 條目 id。跳轉後捲動到它並短暫高亮。 */
  anchor?: string;
}

export function sameTab(a: Address, b: Address): boolean {
  return a.chapter === b.chapter && a.tab === b.tab;
}

/** 目錄裡的第一章第一頁 —— 應用啟動時的落點。 */
export function firstAddress(toc: Toc): Address {
  const chapter = toc.chapters[0];
  return { chapter: chapter.key, tab: chapter.tabs[0].key };
}

/**
 * 切換章節時要落在該章的哪一頁。
 *
 * 記住每一章上次看到哪一頁 —— 從「職業／牧師」切去「專長」再切回來，
 * 應該回到牧師而不是跳回法師。翻書時手指會夾在剛才那一頁。
 */
export function useNavigation(toc: Toc | null) {
  const [address, setAddress] = useState<Address | null>(null);
  const [lastTab, setLastTab] = useState<Record<string, string>>({});

  useEffect(() => {
    if (toc && !address) setAddress(firstAddress(toc));
  }, [toc, address]);

  const goto = useCallback((next: Address) => {
    setLastTab((prev) => ({ ...prev, [next.chapter]: next.tab }));
    setAddress(next);
  }, []);

  const gotoChapter = useCallback(
    (chapterKey: string) => {
      if (!toc) return;
      const chapter = toc.chapters.find((c) => c.key === chapterKey);
      if (!chapter) return;
      const remembered = lastTab[chapterKey];
      const tab =
        remembered && chapter.tabs.some((t) => t.key === remembered)
          ? remembered
          : chapter.tabs[0].key;
      // 落點也要記起來，否則只有「手動點過頁籤」的章記得住位置。
      setLastTab((prev) => ({ ...prev, [chapterKey]: tab }));
      setAddress({ chapter: chapterKey, tab });
    },
    [toc, lastTab],
  );

  return { address, goto, gotoChapter };
}

/**
 * 捲動到某個條目並短暫高亮。
 *
 * 用 data 屬性而不是 DOM id：多分類的專長會同時出現在好幾個分節裡，
 * 用 id 會產生重複 id（HTML 不合法）。querySelector 取第一個命中的，
 * 跳到第一次出現的位置正是我們要的行為。
 */
export function revealEntry(anchor: string) {
  const el = document.querySelector<HTMLElement>(
    `[data-entry-id="${CSS.escape(anchor)}"]`,
  );
  if (!el) return;
  el.scrollIntoView({ behavior: "smooth", block: "center" });
  el.classList.add("flash");
  window.setTimeout(() => el.classList.remove("flash"), 1500);
}
