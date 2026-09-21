import { useEffect, useRef, useState } from "react";

import { ClassChapter as Chapter, getClassChapter } from "../api";
import FeatCard from "../components/FeatCard";
import SectionNav, { NavItem } from "../components/SectionNav";
import { Address, revealEntry } from "../nav";

/**
 * 職業章節：一篇連貫的章，流派依序往下捲。
 *
 * 整章一次查完，章內捲動不再查詢。右側小目錄可跳到任一流派。
 */
export default function ClassChapter({
  className,
  onNavigate,
  pending,
}: {
  className: string;
  onNavigate: (address: Address) => void;
  /** 跳轉進來時要捲到的條目 */
  pending?: string;
}) {
  const [chapter, setChapter] = useState<Chapter | null>(null);
  const [error, setError] = useState<string | null>(null);
  const scrollRoot = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setChapter(null);
    setError(null);
    getClassChapter(className).then(setChapter).catch((e) => setError(String(e)));
  }, [className]);

  // 捲動必須等資料到位後才做 —— 章節是非同步載入的，外層無法預知
  // 什麼時候 DOM 才畫得出來，猜一個延遲時間只會時好時壞。
  useEffect(() => {
    if (chapter && pending) revealEntry(pending);
  }, [chapter, pending]);

  if (error) return <p className="error">{error}</p>;
  if (!chapter) return <p className="loading">載入中⋯</p>;

  const items: NavItem[] = chapter.paths.map((p) => ({
    id: p.id,
    label: p.name,
    count: p.feats.length + p.traits.length,
  }));
  if (chapter.invocations.length > 0) {
    items.push({
      id: "invocations",
      label: "魔能祈喚",
      count: chapter.invocations.length,
    });
  }

  return (
    <div className="chapter-layout">
      <div className="chapter-body" ref={scrollRoot}>
        {chapter.paths.map((path) => (
          <section key={path.id} data-section-id={path.id} className="path">
            <div className="eyebrow">
              {chapter.class_name} ‧ {chapter.path_kind}
            </div>
            <h2 className="path-name">{path.name}</h2>
            {path.description && <p className="path-desc">{path.description}</p>}

            {path.feats.length > 0 && (
              <div className="card-stack">
                {path.feats.map((feat) => (
                  <FeatCard key={feat.id} feat={feat} onNavigate={onNavigate} />
                ))}
              </div>
            )}

            {path.traits.length > 0 && (
              <div className="traits">
                <div className="expand-label">被動特性</div>
                <p className="traits-note">
                  以下為被動特性，無法以 CP 購買或升級。
                </p>
                <div className="card-stack">
                  {path.traits.map((t) => (
                    <article key={t.id} className="feat-card" data-entry-id={t.id}>
                      <div className="feat-head static">
                        <b className="feat-name">{t.name}</b>
                        <span className="chip tag">被動</span>
                      </div>
                      <p className="feat-effect">{t.description}</p>
                    </article>
                  ))}
                </div>
              </div>
            )}
          </section>
        ))}

        {chapter.invocations.length > 0 && (
          <section data-section-id="invocations" className="path">
            <div className="eyebrow">{chapter.class_name}</div>
            <h2 className="path-name">魔能祈喚</h2>
            <p className="path-desc">
              祈喚不是可以升級的技能，消耗的是祈喚欄位而非 CP。
            </p>
            <div className="card-stack">
              {chapter.invocations.map((inv) => (
                <article key={inv.id} className="feat-card" data-entry-id={inv.id}>
                  <div className="feat-head static">
                    <b className="feat-name">{inv.name}</b>
                    {(inv.cost !== null || inv.cost_raw) && (
                      <span className="badge-difficulty">
                        欄位 {inv.cost ?? inv.cost_raw}
                      </span>
                    )}
                    {inv.prereq_raw && <span className="chip">{inv.prereq_raw}</span>}
                  </div>
                  <p className="feat-effect">{inv.effect}</p>
                </article>
              ))}
            </div>
          </section>
        )}
      </div>

      <SectionNav
        title={`${chapter.class_name} ‧ ${chapter.paths.length} ${chapter.path_kind}`}
        items={items}
        scrollRoot={scrollRoot}
      />
    </div>
  );
}
