import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  BuildInfo,
  Facets,
  FeatDetail,
  FeatSummary,
  GROUP_LABELS,
  GROUP_ORDER,
  getBuildInfo,
  getFacets,
  getFeatDetail,
  searchFeats,
} from "./api";
import "./App.css";

/** 搜尋輸入到實際查詢之間的延遲。612 筆資料查得很快，但逐字查仍是浪費。 */
const SEARCH_DEBOUNCE_MS = 180;

export default function App() {
  const [info, setInfo] = useState<BuildInfo | null>(null);
  const [facets, setFacets] = useState<Facets | null>(null);
  const [startupError, setStartupError] = useState<string | null>(null);

  const [query, setQuery] = useState("");
  const [groups, setGroups] = useState<string[]>([]);
  const [categories, setCategories] = useState<string[]>([]);

  const [results, setResults] = useState<FeatSummary[]>([]);
  const [searching, setSearching] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<FeatDetail | null>(null);

  useEffect(() => {
    Promise.all([getBuildInfo(), getFacets()])
      .then(([i, f]) => {
        setInfo(i);
        setFacets(f);
      })
      .catch((e) => setStartupError(String(e)));
  }, []);

  // 搜尋條件變動時重查。requestId 用來丟棄比較舊的回應 ——
  // 快速打字時先送出的查詢可能比後送出的晚回來。
  const requestId = useRef(0);
  useEffect(() => {
    const id = ++requestId.current;
    setSearching(true);
    const timer = setTimeout(() => {
      searchFeats(query, groups, categories)
        .then((rows) => {
          if (requestId.current === id) setResults(rows);
        })
        .catch((e) => {
          if (requestId.current === id) setStartupError(String(e));
        })
        .finally(() => {
          if (requestId.current === id) setSearching(false);
        });
    }, SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [query, groups, categories]);

  useEffect(() => {
    if (!selectedId) {
      setDetail(null);
      return;
    }
    getFeatDetail(selectedId)
      .then(setDetail)
      .catch((e) => setStartupError(String(e)));
  }, [selectedId]);

  const toggle = useCallback(
    (list: string[], set: (v: string[]) => void, value: string) => {
      set(list.includes(value) ? list.filter((v) => v !== value) : [...list, value]);
    },
    [],
  );

  const groupFacets = useMemo(() => {
    if (!facets) return [];
    const byValue = new Map(facets.groups.map((g) => [g.value, g.count]));
    return GROUP_ORDER.filter((g) => byValue.has(g)).map((g) => ({
      value: g,
      count: byValue.get(g) ?? 0,
    }));
  }, [facets]);

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

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <h1>D100 規則書</h1>
          {info && (
            <span className="brand-meta">
              專長 {info.feats} · 流派 {info.class_paths} · 詞綴 {info.affixes} ·
              種族 {info.races}
            </span>
          )}
        </div>
        <input
          className="search"
          type="search"
          placeholder="搜尋專長名稱或效果內文，例如「健壯」「每輪回復」"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          autoFocus
        />
      </header>

      <div className="filters">
        <FilterRow
          label="分組"
          options={groupFacets.map((f) => ({
            value: f.value,
            label: GROUP_LABELS[f.value] ?? f.value,
            count: f.count,
          }))}
          selected={groups}
          onToggle={(v) => toggle(groups, setGroups, v)}
        />
        <FilterRow
          label="分類"
          options={(facets?.categories ?? []).map((f) => ({
            value: f.value,
            label: f.value,
            count: f.count,
          }))}
          selected={categories}
          onToggle={(v) => toggle(categories, setCategories, v)}
        />
      </div>

      <main className="panes">
        <section className="list" aria-label="搜尋結果">
          <div className="list-head">
            {searching ? "搜尋中…" : `${results.length} 筆`}
            {results.length >= 300 && "（僅顯示前 300 筆，請縮小範圍）"}
          </div>
          <ul>
            {results.map((feat) => (
              <li key={feat.id}>
                <button
                  className={feat.id === selectedId ? "row selected" : "row"}
                  onClick={() => setSelectedId(feat.id)}
                >
                  <div className="row-title">
                    <span className="row-name">{feat.name}</span>
                    <span className={`badge group-${feat.feat_group}`}>
                      {GROUP_LABELS[feat.feat_group] ?? feat.feat_group}
                    </span>
                    {feat.class_path && (
                      <span className="badge path">{feat.class_path}</span>
                    )}
                  </div>
                  <div className="row-meta">
                    {feat.categories.map((c) => (
                      <span key={c} className="chip">
                        {c}
                      </span>
                    ))}
                    <span className="difficulty">
                      難度 {formatDifficulty(feat)}
                    </span>
                  </div>
                  <p className="row-effect">{feat.effect_preview}</p>
                </button>
              </li>
            ))}
            {!searching && results.length === 0 && (
              <li className="empty">沒有符合的專長。試著放寬篩選或改用效果關鍵字。</li>
            )}
          </ul>
        </section>

        <section className="detail" aria-label="專長詳情">
          {detail ? (
            <FeatDetailView detail={detail} onNavigate={setSelectedId} />
          ) : (
            <div className="placeholder">
              <p>從左側選一個專長。</p>
              <p className="hint">
                搜尋同時比對名稱與效果內文 —— 記得住「每輪回復1d10hp」卻想不起
                技能叫什麼的時候特別好用。
              </p>
            </div>
          )}
        </section>
      </main>
    </div>
  );
}

function formatDifficulty(feat: {
  difficulty: number | null;
  difficulty_raw: string | null;
  difficulty_scale: string;
}) {
  if (feat.difficulty === null) return feat.difficulty_raw ?? "—";
  const value = Number.isInteger(feat.difficulty)
    ? String(feat.difficulty)
    : feat.difficulty.toFixed(1);
  return feat.difficulty_scale === "legend" ? `傳${value}` : value;
}

interface FilterOption {
  value: string;
  label: string;
  count: number;
}

function FilterRow({
  label,
  options,
  selected,
  onToggle,
}: {
  label: string;
  options: FilterOption[];
  selected: string[];
  onToggle: (value: string) => void;
}) {
  return (
    <div className="filter-row">
      <span className="filter-label">{label}</span>
      {options.map((o) => (
        <button
          key={o.value}
          className={selected.includes(o.value) ? "filter on" : "filter"}
          onClick={() => onToggle(o.value)}
        >
          {o.label}
          <span className="filter-count">{o.count}</span>
        </button>
      ))}
    </div>
  );
}

function FeatDetailView({
  detail,
  onNavigate,
}: {
  detail: FeatDetail;
  onNavigate: (id: string) => void;
}) {
  return (
    <article className="feat">
      <header>
        <h2>{detail.name}</h2>
        <div className="feat-badges">
          <span className={`badge group-${detail.feat_group}`}>
            {GROUP_LABELS[detail.feat_group] ?? detail.feat_group}
          </span>
          {detail.class_name && (
            <span className="badge path">
              {detail.class_name}
              {detail.class_path ? ` · ${detail.class_path}` : ""}
            </span>
          )}
          {detail.categories.map((c) => (
            <span key={c} className="chip">
              {c}
            </span>
          ))}
          {detail.tags.map((t) => (
            <span key={t} className="badge tag">
              {t}
            </span>
          ))}
        </div>
        {detail.parent_name && (
          <p className="parent">子項，母項為〈{detail.parent_name}〉</p>
        )}
      </header>

      <section>
        <h3>效果</h3>
        {detail.effect ? (
          <p className="effect">{detail.effect}</p>
        ) : (
          <p className="effect missing">
            原表沒有撰寫效果敘述。詳見下方的勘誤紀錄。
          </p>
        )}
      </section>

      {detail.cp_table.length > 0 && (
        <section>
          <h3>
            CP 成本
            <span className="section-note">
              公式 2<sup>等級</sup> × 難度{" "}
              {detail.difficulty_scale === "legend"
                ? "（此條計價單位是傳奇技能點，一點需 10 CP 兌換）"
                : ""}
            </span>
          </h3>
          <table className="cp">
            <thead>
              <tr>
                <th>等級</th>
                <th>本級花費</th>
                <th>累計</th>
              </tr>
            </thead>
            <tbody>
              {detail.cp_table.map((step) => (
                <tr key={step.level} className={step.level === 0 ? "level-zero" : ""}>
                  <td>{step.level}</td>
                  <td>{trim(step.step_cost)}</td>
                  <td>{step.level === 0 ? "—" : trim(step.cumulative)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="hint">
            等級 0 是獨立選項（只為免除該技能判定的 20% 減值），不計入升級的累計成本。
          </p>
        </section>
      )}

      {detail.prereqs.length > 0 && (
        <section>
          <h3>前置條件</h3>
          <ul className="links">
            {detail.prereqs.map((p, i) => (
              <li key={i}>
                {p.ref_feat_id ? (
                  <button className="link" onClick={() => onNavigate(p.ref_feat_id!)}>
                    {p.ref_feat_name}
                    {p.min_level ? ` ${p.min_level} 級` : ""}
                  </button>
                ) : (
                  <span className="raw">{p.raw_text}</span>
                )}
                <span className={`kind kind-${p.kind}`}>
                  {p.kind === "free" ? "需 DM 裁定" : p.kind}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}

      {detail.dependents.length > 0 && (
        <section>
          <h3>
            可開出
            <span className="section-note">把這個專長當前置的其他專長</span>
          </h3>
          <ul className="links">
            {detail.dependents.map((d) => (
              <li key={d.id}>
                <button className="link" onClick={() => onNavigate(d.id)}>
                  {d.name}
                </button>
                {d.min_level && <span className="kind">需 {d.min_level} 級</span>}
              </li>
            ))}
          </ul>
        </section>
      )}

      {detail.errata.length > 0 && (
        <section>
          <h3>
            勘誤
            <span className="section-note">我們對原始資料做過的事</span>
          </h3>
          <ul className="errata">
            {detail.errata.map((e, i) => (
              <li key={i}>
                <span className={`badge ${e.action === "set" ? "fixed" : "flagged"}`}>
                  {e.action === "set" ? `已修正 ${e.field ?? ""}` : "標記"}
                </span>
                {e.reason}
              </li>
            ))}
          </ul>
        </section>
      )}

      <footer className="source">
        來源：{detail.source_sheet} 第 {detail.source_row} 列
      </footer>
    </article>
  );
}

/** 去掉沒有意義的小數點（14.0 → 14）。 */
function trim(value: number) {
  return Number.isInteger(value) ? String(value) : value.toFixed(1);
}
