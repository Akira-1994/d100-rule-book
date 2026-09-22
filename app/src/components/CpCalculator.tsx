import { useEffect, useMemo, useState } from "react";

import {
  CpPlan,
  FeatDifficulty,
  GROUP_LABELS,
  getCpPlan,
  getFeatDifficulties,
} from "../api";

/**
 * CP 試算工具。
 *
 * 公式是全書通則（2^等級 × 難度），不是每條專長各自的性質，所以不印在
 * 612 張卡上，集中成一個工具。難度可以直接輸入，也可以從專長挑一條。
 *
 * 兩種計價：一般難度走 CP，`傳N` 走傳奇技能點（一點需 10 點 CP 兌換）。
 */
export default function CpCalculator() {
  const [feats, setFeats] = useState<FeatDifficulty[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [difficulty, setDifficulty] = useState(1);
  const [scale, setScale] = useState("cp");
  const [picked, setPicked] = useState<FeatDifficulty | null>(null);
  const [query, setQuery] = useState("");
  const [plan, setPlan] = useState<CpPlan | null>(null);

  useEffect(() => {
    getFeatDifficulties().then(setFeats).catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    getCpPlan(difficulty, scale).then(setPlan).catch((e) => setError(String(e)));
  }, [difficulty, scale]);

  // 挑選器只在打字之後才列結果 —— 612 筆全列出來沒有人會捲。
  const matches = useMemo(() => {
    const q = query.trim();
    if (!feats || q === "") return [];
    return feats.filter((f) => f.name.includes(q)).slice(0, 12);
  }, [feats, query]);

  const pick = (feat: FeatDifficulty) => {
    setPicked(feat);
    setQuery("");
    setScale(feat.difficulty_scale);
    if (feat.difficulty !== null) setDifficulty(feat.difficulty);
  };

  if (error) return <p className="error">{error}</p>;

  // 難度沒有固定值的條目（〈知識〉〈語言〉的 1or2）由 DM 個案裁定，
  // 程式不替它挑一個數字，只提示使用者自己選。
  const needsRuling = picked !== null && picked.difficulty === null;

  return (
    <div className="chapter-layout">
      <div className="chapter-body prose">
        <section className="intro">
          <p>
            創角色須知：<b>CP 消耗公式為 2^（等級）× 技能難度</b>。等級 0
            是獨立選項，只為免除該技能判定時的 20% 減值，計價方式相同但不計入
            升級的累積成本。
          </p>
          <p>
            傳奇專長的難度寫成「傳N」時算的是<b>傳奇技能點</b>，依該表前言
            一點需以 10 點 CP 兌換。
          </p>
        </section>

        <div className="calc-controls">
          <label className="calc-field">
            <span>難度</span>
            <input
              type="number"
              min={0}
              step={0.5}
              value={difficulty}
              onChange={(e) => {
                setDifficulty(Number(e.target.value));
                setPicked(null);
              }}
            />
          </label>

          <label className="calc-field">
            <span>計價</span>
            <select value={scale} onChange={(e) => setScale(e.target.value)}>
              <option value="cp">CP</option>
              <option value="legend">傳奇技能點</option>
            </select>
          </label>

          <label className="calc-field calc-pick">
            <span>或從專長挑一條</span>
            <input
              type="search"
              placeholder="輸入專長名稱，例如「武器使用」"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </label>
        </div>

        {matches.length > 0 && (
          <ul className="calc-matches">
            {matches.map((f) => (
              <li key={f.id}>
                <button className="calc-match" onClick={() => pick(f)}>
                  <b>{f.name}</b>
                  <span className="chip tag">
                    {f.class_name ?? GROUP_LABELS[f.feat_group] ?? f.feat_group}
                  </span>
                  <span className="calc-match-diff">
                    難度 {f.difficulty ?? f.difficulty_raw ?? "—"}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}

        {picked && (
          <p className="calc-picked">
            目前試算：<b>{picked.name}</b>
            {needsRuling && (
              <span className="calc-warn">
                　這條原表寫「{picked.difficulty_raw}」，依 2026-09-20 的裁示由
                DM 個案裁定為 1 或 2。下方以你輸入的 {difficulty} 計算。
              </span>
            )}
          </p>
        )}

        {plan && (
          <figure className="ref-table">
            <figcaption>
              <b>
                難度 {plan.difficulty}
                {plan.scale === "legend" && "（傳奇技能點）"}
              </b>
            </figcaption>
            <div className="ref-scroll">
              <table>
                <thead>
                  <tr>
                    <th>等級</th>
                    <th>本級花費</th>
                    <th>累計</th>
                    {plan.scale === "legend" && <th>累計等值 CP</th>}
                  </tr>
                </thead>
                <tbody>
                  {plan.steps.map((s) => (
                    <tr key={s.level}>
                      <td>{s.level}</td>
                      <td>{format(s.step_cost)}</td>
                      <td>{s.level === 0 ? "—" : format(s.cumulative)}</td>
                      {plan.scale === "legend" && (
                        <td>{s.level === 0 ? "—" : format(s.cumulative_cp)}</td>
                      )}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <p className="calc-note">
              等級 0 的「累計」為 —— 因為它不是升級的第一階，而是另外付費的
              獨立選項。
            </p>
          </figure>
        )}
      </div>
    </div>
  );
}

/** 難度多半是整數，小數點後全是 0 時不要顯示。 */
function format(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(1);
}
