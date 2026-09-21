import { useEffect, useRef, useState } from "react";

import { SearchHit, searchAll } from "../api";
import type { Address } from "../nav";

const DEBOUNCE_MS = 160;

/**
 * Ctrl+K 全域跳轉。
 *
 * 搜尋是導覽工具而不是另一種閱讀模式，所以選中結果的行為是「切到它所在的
 * 章節並捲到它」，而不是把結果本身當成內容顯示。
 */
export default function CommandPalette({
  open,
  onClose,
  onPick,
}: {
  open: boolean;
  onClose: () => void;
  onPick: (address: Address) => void;
}) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [cursor, setCursor] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const requestId = useRef(0);

  useEffect(() => {
    if (open) {
      setQuery("");
      setHits([]);
      setCursor(0);
      // 對話框掛上之後才 focus，否則拿不到元素。
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const id = ++requestId.current;
    const timer = setTimeout(() => {
      searchAll(query)
        .then((rows) => {
          // 快速打字時先送出的查詢可能比後送出的晚回來。
          if (requestId.current === id) {
            setHits(rows);
            setCursor(0);
          }
        })
        .catch(() => {
          if (requestId.current === id) setHits([]);
        });
    }, DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [query, open]);

  if (!open) return null;

  const pick = (hit: SearchHit) => {
    onPick({ chapter: hit.chapter, tab: hit.tab, anchor: hit.id });
    onClose();
  };

  return (
    <div className="palette-backdrop" onClick={onClose}>
      <div
        className="palette"
        role="dialog"
        aria-label="搜尋規則書"
        onClick={(e) => e.stopPropagation()}
      >
        <input
          ref={inputRef}
          className="palette-input"
          type="search"
          placeholder="搜尋名稱或效果內文，例如「健壯」「每輪回復」"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") onClose();
            else if (e.key === "ArrowDown") {
              e.preventDefault();
              setCursor((c) => Math.min(c + 1, hits.length - 1));
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setCursor((c) => Math.max(c - 1, 0));
            } else if (e.key === "Enter" && hits[cursor]) {
              e.preventDefault();
              pick(hits[cursor]);
            }
          }}
        />

        <ul className="palette-list">
          {hits.map((hit, i) => (
            <li key={`${hit.kind}:${hit.id}`}>
              <button
                className={i === cursor ? "palette-row on" : "palette-row"}
                onMouseEnter={() => setCursor(i)}
                onClick={() => pick(hit)}
              >
                <span className="palette-name">{hit.name}</span>
                {hit.context && <span className="palette-where">{hit.context}</span>}
                <span className="palette-preview">{hit.preview}</span>
              </button>
            </li>
          ))}
          {query.trim() !== "" && hits.length === 0 && (
            <li className="palette-empty">沒有符合的條目。</li>
          )}
          {query.trim() === "" && (
            <li className="palette-empty">
              同時比對名稱與效果內文 —— 記得住「每輪回復1d10hp」卻想不起技能
              叫什麼的時候特別好用。
            </li>
          )}
        </ul>
      </div>
    </div>
  );
}
