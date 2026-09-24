import { useEffect, useMemo, useState } from "react";

import {
  ATTRIBUTES,
  Derived,
  FeatDifficulty,
  Race,
  Sheet,
  TRACK_LABELS,
  Track,
  getFeatDifficulties,
  getRaceChapter,
} from "../api";

/**
 * 角色卡的原始輸入。
 *
 * 只有「算不出來的東西」才在這裡：九大屬性、種族、已學專長、擲出來的獎勵
 * 點數、專長與詞綴提供的 HP/SP。衍生值一律不存、每次重算 —— 存了就會跟
 * 公式不同步。
 */
export default function SheetForm({
  sheet,
  derived,
  onChange,
}: {
  sheet: Sheet;
  derived: Derived | null;
  onChange: (sheet: Sheet) => void;
}) {
  const [races, setRaces] = useState<Race[]>([]);
  const [feats, setFeats] = useState<FeatDifficulty[]>([]);
  const [query, setQuery] = useState("");

  useEffect(() => {
    getRaceChapter().then((c) => setRaces(c.races)).catch(() => {});
    getFeatDifficulties().then(setFeats).catch(() => {});
  }, []);

  const featById = useMemo(
    () => new Map(feats.map((f) => [f.id, f])),
    [feats],
  );

  // 挑選器只在打字之後才列結果 —— 612 筆全列出來沒有人會捲。
  const matches = useMemo(() => {
    const q = query.trim();
    if (q === "") return [];
    const chosen = new Set(sheet.feats.map((f) => f.feat_id));
    return feats
      .filter((f) => f.name.includes(q) && !chosen.has(f.id))
      .slice(0, 10);
  }, [feats, query, sheet.feats]);

  const set = (patch: Partial<Sheet>) => onChange({ ...sheet, ...patch });

  return (
    <div className="sheet-form">
      <section className="sheet-block">
        <div className="expand-label">基本</div>
        <div className="sheet-fields">
          <Field label="角色名">
            <input value={sheet.name} onChange={(e) => set({ name: e.target.value })} />
          </Field>
          <Field label="起始 CP" hint="DM 給的，常見為 200/250/300">
            <input
              type="number"
              value={sheet.starting_cp}
              onChange={(e) => set({ starting_cp: Number(e.target.value) })}
            />
          </Field>
          <Field label="種族">
            <select
              value={sheet.race_id ?? ""}
              onChange={(e) => set({ race_id: e.target.value || null })}
            >
              <option value="">（未選）</option>
              {races.map((r) => (
                <option key={r.id} value={r.id}>
                  {r.name}（{r.cp_cost ?? r.cp_raw}）
                </option>
              ))}
            </select>
          </Field>
        </div>
      </section>

      <section className="sheet-block">
        <div className="expand-label">九大屬性</div>
        <div className="sheet-attrs">
          {ATTRIBUTES.map((attr) => (
            <label key={attr.code} className="sheet-attr">
              <span>
                {attr.code}
                <i>{attr.name}</i>
              </span>
              <input
                type="number"
                value={sheet.attributes[attr.code] ?? ""}
                onChange={(e) => {
                  const next = { ...sheet.attributes };
                  if (e.target.value === "") delete next[attr.code];
                  else next[attr.code] = Number(e.target.value);
                  set({ attributes: next });
                }}
              />
              <em>
                {derived?.modifiers[attr.code] !== undefined
                  ? formatSigned(derived.modifiers[attr.code])
                  : "—"}
              </em>
            </label>
          ))}
        </div>
        <p className="sheet-note">右邊是調整值：13 為 0，每增減 2 點則 ±1。</p>
      </section>

      <section className="sheet-block">
        <div className="expand-label">專長</div>
        <input
          className="sheet-search"
          type="search"
          placeholder="輸入專長名稱加入，例如「武器使用」"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        {matches.length > 0 && (
          <ul className="calc-matches">
            {matches.map((f) => (
              <li key={f.id}>
                <button
                  className="calc-match"
                  onClick={() => {
                    set({
                      feats: [
                        ...sheet.feats,
                        { feat_id: f.id, level: 1, track: "none" },
                      ],
                    });
                    setQuery("");
                  }}
                >
                  <b>{f.name}</b>
                  <span className="calc-match-diff">
                    難度 {f.difficulty ?? f.difficulty_raw ?? "—"}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}

        {sheet.feats.length === 0 ? (
          <p className="sheet-note">還沒有專長。</p>
        ) : (
          <table className="sheet-feats">
            <thead>
              <tr>
                <th>專長</th>
                <th>等級</th>
                <th>軌別</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {sheet.feats.map((entry, index) => (
                <tr key={entry.feat_id}>
                  <td>{featById.get(entry.feat_id)?.name ?? entry.feat_id}</td>
                  <td>
                    <input
                      type="number"
                      min={0}
                      max={5}
                      value={entry.level}
                      onChange={(e) =>
                        set({
                          feats: sheet.feats.map((f, i) =>
                            i === index ? { ...f, level: Number(e.target.value) } : f,
                          ),
                        })
                      }
                    />
                  </td>
                  <td>
                    <select
                      value={entry.track}
                      onChange={(e) =>
                        set({
                          feats: sheet.feats.map((f, i) =>
                            i === index ? { ...f, track: e.target.value as Track } : f,
                          ),
                        })
                      }
                    >
                      {(["melee", "spell", "none"] as Track[]).map((t) => (
                        <option key={t} value={t}>
                          {TRACK_LABELS[t]}
                        </option>
                      ))}
                    </select>
                  </td>
                  <td>
                    <button
                      className="sheet-remove"
                      onClick={() =>
                        set({ feats: sheet.feats.filter((_, i) => i !== index) })
                      }
                    >
                      移除
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        <p className="sheet-note">
          軌別決定獎勵 HP/SP 的門檻。規則書沒有定義哪些專長算「進戰相關」、
          哪些算「法術相關」，所以由你標記（見規則裁示紀錄第 12 項）。
        </p>
      </section>

      <section className="sheet-block">
        <div className="expand-label">HP / SP 的輸入</div>
        <div className="sheet-fields">
          <Field label="進戰獎勵 HP" hint="擲出來的點數">
            <input
              type="number"
              value={sheet.bonus.melee_hp}
              onChange={(e) =>
                set({ bonus: { ...sheet.bonus, melee_hp: Number(e.target.value) } })
              }
            />
          </Field>
          <Field label="進戰獎勵 SP">
            <input
              type="number"
              value={sheet.bonus.melee_sp}
              onChange={(e) =>
                set({ bonus: { ...sheet.bonus, melee_sp: Number(e.target.value) } })
              }
            />
          </Field>
          <Field label="法術獎勵 HP">
            <input
              type="number"
              value={sheet.bonus.spell_hp}
              onChange={(e) =>
                set({ bonus: { ...sheet.bonus, spell_hp: Number(e.target.value) } })
              }
            />
          </Field>
          <Field label="法術獎勵 SP">
            <input
              type="number"
              value={sheet.bonus.spell_sp}
              onChange={(e) =>
                set({ bonus: { ...sheet.bonus, spell_sp: Number(e.target.value) } })
              }
            />
          </Field>
          <Field label="專長／詞綴的 HP" hint="效果敘述裡的自由文字，程式讀不出來">
            <input
              type="number"
              value={sheet.manual_hp}
              onChange={(e) => set({ manual_hp: Number(e.target.value) })}
            />
          </Field>
          <Field label="專長／詞綴的 SP">
            <input
              type="number"
              value={sheet.manual_sp}
              onChange={(e) => set({ manual_sp: Number(e.target.value) })}
            />
          </Field>
          <Field label="加購後的 HP" hint="0 代表沒有加購">
            <input
              type="number"
              value={sheet.extra_hp}
              onChange={(e) => set({ extra_hp: Number(e.target.value) })}
            />
          </Field>
          <Field label="加購後的 SP">
            <input
              type="number"
              value={sheet.extra_sp}
              onChange={(e) => set({ extra_sp: Number(e.target.value) })}
            />
          </Field>
        </div>
      </section>

      <section className="sheet-block">
        <div className="expand-label">備註</div>
        <textarea
          rows={4}
          value={sheet.notes}
          onChange={(e) => set({ notes: e.target.value })}
        />
      </section>
    </div>
  );
}

function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <label className="sheet-field">
      <span className="edit-label">
        {label}
        {hint && <i>{hint}</i>}
      </span>
      {children}
    </label>
  );
}

function formatSigned(value: number): string {
  return value > 0 ? `+${value}` : String(value);
}
