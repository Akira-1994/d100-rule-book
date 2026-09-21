import { useEffect, useRef, useState } from "react";

import { Race, RaceChapter as Chapter, getRaceChapter } from "../api";
import SectionNav, { NavItem } from "../components/SectionNav";
import { revealEntry } from "../nav";

export default function RaceChapter({ pending }: { pending?: string }) {
  const [chapter, setChapter] = useState<Chapter | null>(null);
  const [error, setError] = useState<string | null>(null);
  const scrollRoot = useRef<HTMLDivElement>(null);

  useEffect(() => {
    getRaceChapter().then(setChapter).catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    if (chapter && pending) revealEntry(pending);
  }, [chapter, pending]);

  if (error) return <p className="error">{error}</p>;
  if (!chapter) return <p className="loading">載入中⋯</p>;

  const items: NavItem[] = chapter.races.map((r) => ({
    id: `race:${r.id}`,
    label: r.name,
    count: r.modifiers.length,
  }));

  return (
    <div className="chapter-layout">
      <div className="chapter-body" ref={scrollRoot}>
        <section className="path">
          <div className="eyebrow">種族</div>
          <h2 className="path-name">種族與其調整</h2>
          <p className="path-desc">
            CP 為負代表建卡時扣除。〈原初-星之幼體〉沒有固定價碼，依規則書作者
            2026-09-20 的裁示由 DM 於建卡時裁定。
          </p>
          <div className="card-stack">
            {chapter.races.map((race) => (
              <RaceCard key={race.id} race={race} />
            ))}
          </div>
        </section>
      </div>
      <SectionNav title="13 個種族" items={items} scrollRoot={scrollRoot} />
    </div>
  );
}

function RaceCard({ race }: { race: Race }) {
  return (
    <article
      className="feat-card"
      data-entry-id={race.id}
      data-section-id={`race:${race.id}`}
    >
      <div className="feat-head static">
        <b className="feat-name">{race.name}</b>
        {/* 非數值的 CP 顯示原文並標明要 DM 裁定，不假裝它是個數字。 */}
        {race.cp_cost === null ? (
          <span className="chip tag">{race.cp_raw} · 需 DM 裁定</span>
        ) : (
          <span className="badge-difficulty">CP {race.cp_cost}</span>
        )}
        {race.modifiers.map((m) => (
          <span key={m.attr} className="chip">
            {m.attr} {m.delta > 0 ? `+${m.delta}` : m.delta}
          </span>
        ))}
      </div>

      <dl className="race-fields">
        <Field label="屬性調整" value={race.attr_text} />
        <Field label="種族專長" value={race.racial_feat_text} />
        <Field label="技能調整" value={race.skill_mod_text} />
        <Field label="特殊" value={race.special_text} />
      </dl>

      <div className="source">
        {race.source_sheet} R{race.source_row}
      </div>
    </article>
  );
}

function Field({ label, value }: { label: string; value: string | null }) {
  if (!value) return null;
  return (
    <>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </>
  );
}
