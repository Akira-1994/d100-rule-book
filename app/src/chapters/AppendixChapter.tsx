import { useEffect, useRef, useState } from "react";

import { ErrataEntry, RefTableSummary, getErrataList, getRefIndex } from "../api";
import SectionNav, { NavItem } from "../components/SectionNav";

/**
 * 附錄的兩個非散文頁籤：對照表總覽與勘誤清單。
 * （世界觀、大陸簡史那幾個走 ProseChapter；CP 試算是階段 3。）
 */
export default function AppendixChapter({ kind }: { kind: string }) {
  if (kind === "ref_index") return <RefIndexView />;
  if (kind === "errata") return <ErrataView />;
  return (
    <p className="loading">
      CP 試算工具於階段 3 補上。CP 消耗公式為 2^等級 × 難度，等級 0 另計。
    </p>
  );
}

function RefIndexView() {
  const [index, setIndex] = useState<RefTableSummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const scrollRoot = useRef<HTMLDivElement>(null);

  useEffect(() => {
    getRefIndex().then(setIndex).catch((e) => setError(String(e)));
  }, []);

  if (error) return <p className="error">{error}</p>;
  if (!index) return <p className="loading">載入中⋯</p>;

  const sheets: string[] = [];
  for (const t of index) if (!sheets.includes(t.sheet)) sheets.push(t.sheet);

  const items: NavItem[] = sheets.map((s) => ({
    id: `sheet:${s}`,
    label: s,
    count: index.filter((t) => t.sheet === s).length,
  }));

  return (
    <div className="chapter-layout">
      <div className="chapter-body" ref={scrollRoot}>
        <section className="intro">
          <p>
            全書 {index.length} 張對照表的索引。這些表散落在各章之中 ——
            價格表在物品章、地形額外法術在德魯伊、等級對照在創角流程 ——
            這裡只是把它們列在一起方便查找。
          </p>
        </section>
        {sheets.map((sheet) => (
          <section key={sheet} className="path" data-section-id={`sheet:${sheet}`}>
            <div className="eyebrow">工作表</div>
            <h2 className="path-name">{sheet}</h2>
            <div className="card-stack">
              {index
                .filter((t) => t.sheet === sheet)
                .map((t) => (
                  <article key={t.id} className="feat-card">
                    <div className="feat-head static">
                      <b className="feat-name">{t.name}</b>
                      <span className="chip tag">{t.row_count} 列</span>
                    </div>
                  </article>
                ))}
            </div>
          </section>
        ))}
      </div>
      <SectionNav title="工作表" items={items} scrollRoot={scrollRoot} />
    </div>
  );
}

function ErrataView() {
  const [list, setList] = useState<ErrataEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const scrollRoot = useRef<HTMLDivElement>(null);

  useEffect(() => {
    getErrataList().then(setList).catch((e) => setError(String(e)));
  }, []);

  if (error) return <p className="error">{error}</p>;
  if (!list) return <p className="loading">載入中⋯</p>;

  const sheets: string[] = [];
  for (const e of list) if (!sheets.includes(e.sheet)) sheets.push(e.sheet);

  const items: NavItem[] = sheets.map((s) => ({
    id: `errata:${s}`,
    label: s,
    count: list.filter((e) => e.sheet === s).length,
  }));

  return (
    <div className="chapter-layout">
      <div className="chapter-body" ref={scrollRoot}>
        <section className="intro">
          <p>
            我們對原始試算表所做的每一處修正與標記，連同理由。原始 xlsx 永遠
            不動，修正寫在 git 內的 YAML，因此這份清單可以直接拿去跟規則書
            作者對帳。
          </p>
          <p>
            <b>set</b> 是修正、<b>flag</b> 是只標記不臆測答案。共 {list.length} 筆。
          </p>
        </section>
        {sheets.map((sheet) => (
          <section key={sheet} className="path" data-section-id={`errata:${sheet}`}>
            <div className="eyebrow">工作表</div>
            <h2 className="path-name">{sheet}</h2>
            <div className="card-stack">
              {list
                .filter((e) => e.sheet === sheet)
                .map((e, i) => (
                  <article key={i} className="feat-card">
                    <div className="feat-head static">
                      <b className="feat-name">
                        {e.source_row !== null ? `R${e.source_row}` : "整張表"}
                      </b>
                      <span className="badge-difficulty">{e.action}</span>
                      {e.field && <span className="chip">{e.field}</span>}
                      {e.issue && <span className="chip tag">{e.issue}</span>}
                    </div>
                    {e.action === "set" && (
                      <p className="errata-change">
                        <code>{e.raw_value ?? "（空）"}</code> →{" "}
                        <code>{e.fixed_value ?? "（空）"}</code>
                      </p>
                    )}
                    <p className="feat-effect">{e.reason}</p>
                  </article>
                ))}
            </div>
          </section>
        ))}
      </div>
      <SectionNav title="工作表" items={items} scrollRoot={scrollRoot} />
    </div>
  );
}
