import { useCallback, useEffect, useState } from "react";

import {
  ChapterFeat,
  EntryDetail,
  PREREQ_KIND_LABELS,
  Prereq,
  formatDifficulty,
  getEntryDetail,
} from "../api";
import EditButton from "./EditButton";
import type { Address } from "../nav";

/**
 * 一張專長卡。
 *
 * 預設就顯示效果全文 —— 這是規則書不是索引，讀者要的是規則本身而不是
 * 「點進去才看得到」的標題列。點擊卡片才展開前置鏈與來源，那些是查證
 * 用的資訊，讀規則時不該擋在中間。
 */
export default function FeatCard({
  feat,
  onNavigate,
}: {
  feat: ChapterFeat;
  onNavigate: (address: Address) => void;
}) {
  const [open, setOpen] = useState(false);
  const [detail, setDetail] = useState<EntryDetail | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open || detail) return;
    getEntryDetail("feat", feat.id)
      .then(setDetail)
      .catch((e) => setError(String(e)));
  }, [open, detail, feat.id]);

  const toggle = useCallback(() => setOpen((v) => !v), []);

  return (
    <article
      className={open ? "feat-card open" : "feat-card"}
      data-entry-id={feat.id}
    >
      <button className="feat-head" onClick={toggle} aria-expanded={open}>
        <b className="feat-name">{feat.name}</b>
        <span className="badge-difficulty">難度 {formatDifficulty(feat)}</span>
        {feat.categories.map((c) => (
          <span key={c} className="chip">
            {c}
          </span>
        ))}
        {feat.tags.map((t) => (
          <span key={t} className="chip tag">
            {t}
          </span>
        ))}
        <span className="caret">{open ? "▲" : "▼"}</span>
      </button>

      <p className="feat-effect">{feat.effect}</p>

      {open && (
        <div className="feat-expand">
          {error && <p className="error">{error}</p>}
          {detail && (
            <>
              <div className="expand-label">前置與後續</div>
              <p className="expand-body">
                前置：
                {detail.prereqs.length === 0 ? (
                  "無"
                ) : (
                  detail.prereqs.map((p, i) => (
                    <span key={i}>
                      {i > 0 && "、"}
                      <PrereqRef prereq={p} onNavigate={onNavigate} />
                    </span>
                  ))
                )}
              </p>
              {detail.dependents.length > 0 && (
                <p className="expand-body">
                  被這些當前置：
                  {detail.dependents.map((d, i) => (
                    <span key={d.id}>
                      {i > 0 && "、"}
                      <button
                        className="link"
                        onClick={() =>
                          onNavigate({ chapter: d.chapter, tab: d.tab, anchor: d.id })
                        }
                      >
                        {d.name}
                      </button>
                    </span>
                  ))}
                </p>
              )}
              <div className="expand-foot">
                <span className="source">
                  {detail.source_sheet} R{detail.source_row}
                </span>
                <EditButton
                  entryKind="feat"
                  entryName={feat.name}
                  sheet={detail.source_sheet}
                  row={detail.source_row}
                  col={detail.source_col}
                  current={{
                    difficulty: feat.difficulty,
                    difficulty_raw: feat.difficulty_raw,
                    effect: feat.effect,
                    categories: feat.categories,
                    tags: feat.tags,
                  }}
                />
              </div>
            </>
          )}
        </div>
      )}

    </article>
  );
}

function PrereqRef({
  prereq: p,
  onNavigate,
}: {
  prereq: Prereq;
  onNavigate: (address: Address) => void;
}) {
  const jumpable = p.ref_feat_id && p.ref_chapter && p.ref_tab;
  return (
    <>
      {jumpable ? (
        <button
          className="link"
          onClick={() =>
            onNavigate({
              chapter: p.ref_chapter!,
              tab: p.ref_tab!,
              anchor: p.ref_feat_id!,
            })
          }
        >
          {p.ref_feat_name ?? p.raw_text}
          {p.min_level !== null && ` ${p.min_level} 級`}
        </button>
      ) : (
        <span>{p.raw_text}</span>
      )}
      {/* kind 決定這一條能不能自動驗證。free 是 DM 裁定，標出來免得建卡時
          以為程式已經檢查過了。 */}
      <span className={p.kind === "free" ? "prereq-kind free" : "prereq-kind"}>
        {PREREQ_KIND_LABELS[p.kind] ?? p.kind}
      </span>
    </>
  );
}
