import { Adjustment, Derived, DerivedValue } from "../api";

/**
 * 衍生值與修正。
 *
 * 每一項都顯示 **公式值 ＋ 修正 ＝ 顯示值**，三個數字都看得到。實際跑團
 * 一定會有公式沒涵蓋的加值（裝備、buff、DM 裁定），但那些是「差額」而不是
 * 「換一個數字」—— 直接覆寫會遺失「本來應該是多少」，屬性改了之後也不
 * 知道那個覆寫還算不算數。
 */
export default function DerivedPanel({
  derived,
  adjustments,
  onAdjust,
}: {
  derived: Derived;
  adjustments: Record<string, Adjustment>;
  onAdjust: (label: string, adjustment: Adjustment) => void;
}) {
  return (
    <div className="derived">
      {derived.warnings.length > 0 && (
        <section className="sheet-warnings">
          <div className="expand-label">提醒</div>
          {/* 一律只提示、不擋存檔 —— 與 PLAN.md 的「違規可覆寫並標記」一致。 */}
          <ul>
            {derived.warnings.map((w, i) => (
              <li key={i}>{w}</li>
            ))}
          </ul>
        </section>
      )}

      <CpPanel derived={derived} />
      <PoolPanel derived={derived} />

      <Group
        title="六大技能"
        values={derived.skills}
        adjustments={adjustments}
        onAdjust={onAdjust}
      />
      <Group
        title="五大抗性"
        values={derived.resists}
        adjustments={adjustments}
        onAdjust={onAdjust}
      />
      <Group
        title="三特殊"
        values={derived.specials}
        adjustments={adjustments}
        onAdjust={onAdjust}
        note="不計屬性調整值"
      />
    </div>
  );
}

function CpPanel({ derived }: { derived: Derived }) {
  const cp = derived.cp;
  return (
    <section className="sheet-block">
      <div className="expand-label">CP 收支</div>
      <dl className="sheet-ledger">
        <Row label="起始" value={cp.starting} />
        <Row label="屬性調整值" value={cp.attribute_adjustment} signed />
        <Row label="可用" value={cp.available} strong />
        <Row label="種族" value={cp.race} signed />
        <Row label="專長" value={-cp.feats} signed />
        {cp.legend_as_cp > 0 && (
          <Row label="傳奇技能點" value={-cp.legend_as_cp} signed />
        )}
        {cp.extra_hp > 0 && <Row label="額外 HP" value={-cp.extra_hp} signed />}
        {cp.extra_sp > 0 && <Row label="額外 SP" value={-cp.extra_sp} signed />}
        <Row label="餘額" value={cp.remaining} strong danger={cp.remaining < 0} />
      </dl>
      <p className="sheet-note">
        兩軌投資：進戰 {cp.melee_invested} CP（{derived.melee_dice_steps} 階）、
        法術 {cp.spell_invested} CP（{derived.spell_dice_steps} 階）
      </p>
    </section>
  );
}

function PoolPanel({ derived }: { derived: Derived }) {
  return (
    <section className="sheet-block">
      <div className="expand-label">HP / SP</div>
      <table className="sheet-pool">
        <thead>
          <tr>
            <th></th>
            <th>主屬性</th>
            <th>獎勵</th>
            <th>手填</th>
            <th>基礎</th>
            <th>加購</th>
            <th>合計</th>
          </tr>
        </thead>
        <tbody>
          <PoolRow label="HP" pool={derived.hp} />
          <PoolRow label="SP" pool={derived.sp} />
        </tbody>
      </table>
      <p className="sheet-note">
        獎勵是擲出來的（{derived.melee_dice_steps}d8 進戰、
        {derived.spell_dice_steps}d8 法術⋯），程式算得出該擲幾顆、算不出點數，
        結果請填在左側。
      </p>
    </section>
  );
}

function PoolRow({ label, pool }: { label: string; pool: Derived["hp"] }) {
  return (
    <tr>
      <th>{label}</th>
      <td>{pool.attribute_part}</td>
      <td>{pool.rolled}</td>
      <td>{pool.manual}</td>
      <td>
        <b>{pool.base}</b>
      </td>
      <td>{pool.extra_target > 0 ? `${pool.extra_target}（${pool.extra_cost} CP）` : "—"}</td>
      <td>
        <b>{pool.total}</b>
      </td>
    </tr>
  );
}

function Group({
  title,
  values,
  adjustments,
  onAdjust,
  note,
}: {
  title: string;
  values: DerivedValue[];
  adjustments: Record<string, Adjustment>;
  onAdjust: (label: string, adjustment: Adjustment) => void;
  note?: string;
}) {
  return (
    <section className="sheet-block">
      <div className="expand-label">{title}</div>
      {note && <p className="sheet-note">{note}</p>}
      <table className="sheet-values">
        <tbody>
          {values.map((v) => {
            const current = adjustments[v.label] ?? { delta: 0, note: "" };
            return (
              <tr key={v.label}>
                <th>{v.label}</th>
                <td className="sheet-formula">{v.formula}</td>
                <td className="sheet-plus">＋</td>
                <td>
                  <input
                    type="number"
                    className="sheet-delta"
                    value={current.delta}
                    onChange={(e) =>
                      onAdjust(v.label, { ...current, delta: Number(e.target.value) })
                    }
                  />
                </td>
                <td className="sheet-plus">＝</td>
                <td className="sheet-total">{v.total}</td>
                <td>
                  <input
                    type="text"
                    className="sheet-adj-note"
                    placeholder="修正的來源，例如「靈巧之靴」"
                    value={current.note}
                    onChange={(e) =>
                      onAdjust(v.label, { ...current, note: e.target.value })
                    }
                  />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </section>
  );
}

function Row({
  label,
  value,
  signed,
  strong,
  danger,
}: {
  label: string;
  value: number;
  signed?: boolean;
  strong?: boolean;
  danger?: boolean;
}) {
  const text = signed && value > 0 ? `+${value}` : String(value);
  return (
    <>
      <dt>{label}</dt>
      <dd className={danger ? "danger" : strong ? "strong" : undefined}>{text}</dd>
    </>
  );
}
