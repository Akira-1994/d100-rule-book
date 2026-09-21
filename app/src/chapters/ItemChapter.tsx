import { useEffect, useRef, useState } from "react";

import {
  Affix,
  AffixChapter,
  DistributionGroup,
  Material,
  MaterialChapter,
  RefTable,
  getAffixChapter,
  getAffixDistribution,
  getMaterialChapter,
  getRefSheet,
} from "../api";
import RefTableView from "../components/RefTableView";
import SectionNav, { NavItem } from "../components/SectionNav";
import { revealEntry } from "../nav";

/**
 * 物品章節：四種頁籤共用一個外殼，內容依 kind 分派。
 *
 * 四者的資料形狀差很多（詞綴是條目、素材是父子、價格表是一堆小表、
 * 分布是部位×加值的矩陣），但「載入 → 右側目錄 → 捲動」的骨架相同。
 */
export default function ItemChapter({
  kind,
  tabKey,
  pending,
}: {
  kind: string;
  tabKey: string;
  pending?: string;
}) {
  if (kind === "affix") return <AffixView tier={tabKey} pending={pending} />;
  if (kind === "material") return <MaterialView pending={pending} />;
  if (kind === "ref_sheet") return <RefSheetView sheet={tabKey} />;
  return <DistributionView />;
}

// 詞綴 --------------------------------------------------------------------

function AffixView({ tier, pending }: { tier: string; pending?: string }) {
  const [chapter, setChapter] = useState<AffixChapter | null>(null);
  const [error, setError] = useState<string | null>(null);
  const scrollRoot = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setChapter(null);
    getAffixChapter(tier).then(setChapter).catch((e) => setError(String(e)));
  }, [tier]);

  useEffect(() => {
    if (chapter && pending) revealEntry(pending);
  }, [chapter, pending]);

  if (error) return <p className="error">{error}</p>;
  if (!chapter) return <p className="loading">載入中⋯</p>;

  // 詞綴沒有分節，右側目錄改列部位，點了捲到該部位的第一條。
  const bySlot = new Map<string, string>();
  const counts = new Map<string, number>();
  for (const a of chapter.affixes) {
    for (const slot of a.slots) {
      if (!bySlot.has(slot)) bySlot.set(slot, a.id);
      counts.set(slot, (counts.get(slot) ?? 0) + 1);
    }
  }
  const items: NavItem[] = [...bySlot.keys()].map((slot) => ({
    id: `slot:${slot}`,
    label: slot,
    count: counts.get(slot) ?? 0,
  }));

  return (
    <div className="chapter-layout">
      <div className="chapter-body" ref={scrollRoot}>
        {chapter.intro.length > 0 && (
          <section className="intro">
            {chapter.intro.map((p, i) => (
              <p key={i}>{p}</p>
            ))}
          </section>
        )}
        <div className="card-stack">
          {chapter.affixes.map((affix) => (
            <AffixCard key={affix.id} affix={affix} slotAnchors={bySlot} />
          ))}
        </div>
      </div>
      <SectionNav title="可附的部位" items={items} scrollRoot={scrollRoot} />
    </div>
  );
}

function AffixCard({
  affix,
  slotAnchors,
}: {
  affix: Affix;
  slotAnchors: Map<string, string>;
}) {
  // 這張卡若是某個部位的第一條，就兼作那個部位的錨點。
  const anchorFor = [...slotAnchors.entries()].find(([, id]) => id === affix.id);

  return (
    <article
      className="feat-card"
      data-entry-id={affix.id}
      data-section-id={anchorFor ? `slot:${anchorFor[0]}` : undefined}
    >
      <div className="feat-head static">
        <b className="feat-name">{affix.name}</b>
        {affix.slots.map((s) => (
          <span key={s} className="chip">
            {s}
          </span>
        ))}
      </div>
      <p className="feat-effect">{affix.effect}</p>
      <div className="affix-ranks">
        {affix.ranks.map((r, i) => (
          <span key={i} className="badge-difficulty">
            {/* condition 非空代表這是同一階在不同裝備上的價碼，不是另一階。 */}
            {r.condition ? `${r.condition} ` : `第 ${r.rank} 階 `}＋{r.plus_cost}
          </span>
        ))}
        {affix.ranks.length === 0 && affix.plus_cost_raw && (
          <span className="chip tag">{affix.plus_cost_raw}</span>
        )}
      </div>
      <div className="source">
        {affix.source_sheet} R{affix.source_row}
      </div>
    </article>
  );
}

// 素材 --------------------------------------------------------------------

function MaterialView({ pending }: { pending?: string }) {
  const [chapter, setChapter] = useState<MaterialChapter | null>(null);
  const [error, setError] = useState<string | null>(null);
  const scrollRoot = useRef<HTMLDivElement>(null);

  useEffect(() => {
    getMaterialChapter().then(setChapter).catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    if (chapter && pending) revealEntry(pending);
  }, [chapter, pending]);

  if (error) return <p className="error">{error}</p>;
  if (!chapter) return <p className="loading">載入中⋯</p>;

  const items: NavItem[] = chapter.materials.map((m) => ({
    id: m.id,
    label: m.name,
    count: m.affixes.length,
  }));

  return (
    <div className="chapter-layout">
      <div className="chapter-body" ref={scrollRoot}>
        {chapter.intro.length > 0 && (
          <section className="intro">
            {chapter.intro.map((p, i) => (
              <p key={i}>{p}</p>
            ))}
          </section>
        )}
        {chapter.materials.map((m) => (
          <MaterialSection key={m.id} material={m} />
        ))}
      </div>
      <SectionNav title={`${chapter.materials.length} 種素材`} items={items} scrollRoot={scrollRoot} />
    </div>
  );
}

function MaterialSection({ material }: { material: Material }) {
  return (
    <section className="path" data-section-id={material.id} data-entry-id={material.id}>
      <div className="eyebrow">素材</div>
      <h2 className="path-name">{material.name}</h2>
      <p className="path-desc">
        {material.material_tier !== null && `${material.material_tier} 階`}
        {material.cost_multiplier !== null && ` · 加工費 ×${material.cost_multiplier}`}
        {material.slots.length > 0 && ` · 可附於 ${material.slots.join("、")}`}
      </p>
      <div className="card-stack">
        {material.affixes.map((a) => (
          <article key={a.id} className="feat-card" data-entry-id={a.id}>
            <div className="feat-head static">
              <b className="feat-name">{a.name}</b>
              {/* 附素材詞綴時擲一次 D100，落在哪個區間就得到哪一條。 */}
              <span className="badge-difficulty">
                D100 {a.roll_min}–{a.roll_max}
              </span>
              <span className="chip">第 {a.tier_rank} 階</span>
              {a.rarity_multiplier !== null && (
                <span className="chip tag">價格 ×{a.rarity_multiplier}</span>
              )}
            </div>
            <p className="feat-effect">{a.effect}</p>
          </article>
        ))}
      </div>
    </section>
  );
}

// 價格表 ------------------------------------------------------------------

function RefSheetView({ sheet }: { sheet: string }) {
  const [tables, setTables] = useState<RefTable[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const scrollRoot = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setTables(null);
    getRefSheet(sheet).then(setTables).catch((e) => setError(String(e)));
  }, [sheet]);

  if (error) return <p className="error">{error}</p>;
  if (!tables) return <p className="loading">載入中⋯</p>;

  const items: NavItem[] = tables.map((t) => ({
    id: t.id,
    label: t.name,
    count: t.rows.length,
  }));

  return (
    <div className="chapter-layout">
      <div className="chapter-body" ref={scrollRoot}>
        {tables.map((t) => (
          <div key={t.id} data-section-id={t.id}>
            <RefTableView table={t} />
          </div>
        ))}
      </div>
      <SectionNav title={`${tables.length} 張表`} items={items} scrollRoot={scrollRoot} />
    </div>
  );
}

// 詞綴分布 ----------------------------------------------------------------

function DistributionView() {
  const [groups, setGroups] = useState<DistributionGroup[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const scrollRoot = useRef<HTMLDivElement>(null);

  useEffect(() => {
    getAffixDistribution().then(setGroups).catch((e) => setError(String(e)));
  }, []);

  if (error) return <p className="error">{error}</p>;
  if (!groups) return <p className="loading">載入中⋯</p>;

  // 同一個部位的各加值等級收在一起。順序照後端給的原表順序，不重排。
  const slots: string[] = [];
  for (const g of groups) if (!slots.includes(g.slot)) slots.push(g.slot);

  const items: NavItem[] = slots.map((slot) => ({
    id: `dist:${slot}`,
    label: slot,
    count: groups.filter((g) => g.slot === slot).length,
  }));

  return (
    <div className="chapter-layout">
      <div className="chapter-body" ref={scrollRoot}>
        <section className="intro">
          <p>
            混沌石擲出詞綴時，可能出現哪些詞綴由「部位 × 魔法物品加值」決定。
            名稱一律以詞綴表為準 —— 分布表裡的四個筆誤已於建置時對正。
          </p>
        </section>
        {slots.map((slot) => (
          <section key={slot} className="path" data-section-id={`dist:${slot}`}>
            <div className="eyebrow">部位</div>
            <h2 className="path-name">{slot}</h2>
            <div className="card-stack">
              {groups
                .filter((g) => g.slot === slot)
                .map((g) => (
                  <article key={`${g.slot}:${g.plus_label}`} className="feat-card">
                    <div className="feat-head static">
                      <b className="feat-name">{g.plus_label}</b>
                      <span className="chip tag">{g.affixes.length} 條</span>
                    </div>
                    <p className="feat-effect">{g.affixes.join("、")}</p>
                  </article>
                ))}
            </div>
          </section>
        ))}
      </div>
      <SectionNav title="部位" items={items} scrollRoot={scrollRoot} />
    </div>
  );
}
