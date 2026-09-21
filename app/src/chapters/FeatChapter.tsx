import { useEffect, useRef, useState } from "react";

import { FeatChapter as Chapter, getFeatChapter } from "../api";
import FeatCard from "../components/FeatCard";
import SectionNav, { NavItem } from "../components/SectionNav";
import { Address, revealEntry } from "../nav";

/**
 * 專長章節：非職業的六組專長，依分類分節。
 *
 * 一個專長可屬多個分類，因此同一條會出現在多個分節 —— 這是刻意的，
 * 找「戰鬥/運動」的專長時兩節都該找得到。
 */
export default function FeatChapter({
  group,
  onNavigate,
  pending,
}: {
  group: string;
  onNavigate: (address: Address) => void;
  pending?: string;
}) {
  const [chapter, setChapter] = useState<Chapter | null>(null);
  const [error, setError] = useState<string | null>(null);
  const scrollRoot = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setChapter(null);
    setError(null);
    getFeatChapter(group).then(setChapter).catch((e) => setError(String(e)));
  }, [group]);

  // 捲動必須等資料到位後才做 —— 章節是非同步載入的，外層無法預知
  // 什麼時候 DOM 才畫得出來，猜一個延遲時間只會時好時壞。
  useEffect(() => {
    if (chapter && pending) revealEntry(pending);
  }, [chapter, pending]);

  if (error) return <p className="error">{error}</p>;
  if (!chapter) return <p className="loading">載入中⋯</p>;

  const items: NavItem[] = chapter.sections.map((s) => ({
    id: `cat:${s.category}`,
    label: s.category,
    count: s.feats.length,
  }));
  if (chapter.uncategorized.length > 0) {
    items.push({
      id: "uncategorized",
      label: "未分類",
      count: chapter.uncategorized.length,
    });
  }

  return (
    <div className="chapter-layout">
      <div className="chapter-body" ref={scrollRoot}>
        {chapter.intro.length > 0 && (
          <section className="intro">
            {chapter.intro.map((paragraph, i) => (
              <p key={i}>{paragraph}</p>
            ))}
          </section>
        )}

        {chapter.sections.map((section) => (
          <section
            key={section.category}
            data-section-id={`cat:${section.category}`}
            className="path"
          >
            <div className="eyebrow">分類</div>
            <h2 className="path-name">{section.category}</h2>
            <div className="card-stack">
              {section.feats.map((feat) => (
                <FeatCard key={feat.id} feat={feat} onNavigate={onNavigate} />
              ))}
            </div>
          </section>
        ))}

        {chapter.uncategorized.length > 0 && (
          <section data-section-id="uncategorized" className="path">
            <div className="eyebrow">分類</div>
            <h2 className="path-name">未分類</h2>
            <p className="path-desc">
              原表的分類欄留空。不臆測歸屬，照原樣列在這裡。
            </p>
            <div className="card-stack">
              {chapter.uncategorized.map((feat) => (
                <FeatCard key={feat.id} feat={feat} onNavigate={onNavigate} />
              ))}
            </div>
          </section>
        )}
      </div>

      <SectionNav title="分類" items={items} scrollRoot={scrollRoot} />
    </div>
  );
}
