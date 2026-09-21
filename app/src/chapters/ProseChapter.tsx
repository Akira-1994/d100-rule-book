import { useEffect, useRef, useState } from "react";

import { ProseChapter as Chapter, getProseChapter } from "../api";
import RefTableView from "../components/RefTableView";
import SectionNav, { NavItem } from "../components/SectionNav";
import { revealEntry } from "../nav";

/**
 * 散文規則：創角流程、戰鬥流程、世界觀等。
 *
 * 兩層層級（戰鬥流程的大步驟底下再分一般／自由／即時動作）以標題與縮排
 * 表示。同一張工作表上的對照表接在內文之後 —— 「創角色須知」的等級對照表
 * 本來就是那篇的一部分。
 */
export default function ProseChapter({
  sheet,
  pending,
}: {
  sheet: string;
  pending?: string;
}) {
  const [chapter, setChapter] = useState<Chapter | null>(null);
  const [error, setError] = useState<string | null>(null);
  const scrollRoot = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setChapter(null);
    getProseChapter(sheet).then(setChapter).catch((e) => setError(String(e)));
  }, [sheet]);

  useEffect(() => {
    if (chapter && pending) revealEntry(pending);
  }, [chapter, pending]);

  if (error) return <p className="error">{error}</p>;
  if (!chapter) return <p className="loading">載入中⋯</p>;

  const items: NavItem[] = chapter.sections
    .map((s, i) => ({
      id: `prose:${i}`,
      label: s.title ?? "（前言）",
      count: s.blocks.length,
    }))
    .concat(
      chapter.tables.map((t) => ({
        id: t.id,
        label: t.name,
        count: t.rows.length,
      })),
    );

  return (
    <div className="chapter-layout">
      <div className="chapter-body prose" ref={scrollRoot}>
        {chapter.sections.map((section, i) => (
          <section key={i} className="path" data-section-id={`prose:${i}`}>
            {section.title && <h2 className="path-name">{section.title}</h2>}
            {section.blocks.map((block) => (
              <div key={block.id} className="prose-block" data-entry-id={String(block.id)}>
                {block.subsection && <h3 className="prose-sub">{block.subsection}</h3>}
                <p className="prose-body">{block.body}</p>
              </div>
            ))}
          </section>
        ))}

        {chapter.tables.map((t) => (
          <div key={t.id} data-section-id={t.id}>
            <RefTableView table={t} />
          </div>
        ))}
      </div>
      <SectionNav title="章節" items={items} scrollRoot={scrollRoot} />
    </div>
  );
}
