import { useCallback, useEffect, useRef, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";

import {
  Adjustment,
  Derived,
  Sheet,
  SheetSummary,
  deleteSheet,
  deriveSheet,
  exportSheet,
  importSheet,
  listSheets,
  loadSheet,
  newSheet,
  saveSheet,
} from "../api";
import DerivedPanel from "../components/DerivedPanel";
import SheetForm from "../components/SheetForm";

/** 停手多久才存檔。每敲一個字就寫檔沒有必要，也會讓磁碟一直在動。 */
const SAVE_DEBOUNCE_MS = 600;

/**
 * 角色卡章節：左側清單、右側一張卡。
 *
 * 表單改動 → 重算衍生值（純計算，立即）→ 稍後存檔（有延遲）。
 * 兩件事分開是因為重算要跟得上打字，存檔不必。
 */
export default function SheetChapter() {
  const [sheets, setSheets] = useState<SheetSummary[] | null>(null);
  const [current, setCurrent] = useState<Sheet | null>(null);
  const [derived, setDerived] = useState<Derived | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(true);

  const refresh = useCallback(
    () => listSheets().then(setSheets).catch((e) => setError(String(e))),
    [],
  );

  useEffect(() => {
    refresh();
  }, [refresh]);

  // 衍生值跟著打字重算。requestId 丟掉比較舊的回應 —— 快速輸入時先送出的
  // 查詢可能比後送出的晚回來。
  const deriveId = useRef(0);
  useEffect(() => {
    if (!current) {
      setDerived(null);
      return;
    }
    const id = ++deriveId.current;
    deriveSheet(current)
      .then((d) => {
        if (deriveId.current === id) setDerived(d);
      })
      .catch((e) => setError(String(e)));
  }, [current]);

  // 存檔延遲。卸載時要把未存的改動補寫，否則切走就掉了。
  const pending = useRef<Sheet | null>(null);
  useEffect(() => {
    if (!current || saved) return;
    pending.current = current;
    const timer = setTimeout(() => {
      saveSheet(current)
        .then(() => {
          setSaved(true);
          pending.current = null;
          refresh();
        })
        .catch((e) => setError(String(e)));
    }, SAVE_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [current, saved, refresh]);

  useEffect(
    () => () => {
      if (pending.current) saveSheet(pending.current).catch(() => {});
    },
    [],
  );

  const edit = (next: Sheet) => {
    setCurrent(next);
    setSaved(false);
  };

  const adjust = (label: string, adjustment: Adjustment) => {
    if (!current) return;
    const next = { ...current.adjustments };
    // 修正歸零又沒有註記就把整筆拿掉 —— 留著會讓 JSON 塞滿沒有意義的零。
    if (adjustment.delta === 0 && adjustment.note.trim() === "") delete next[label];
    else next[label] = adjustment;
    edit({ ...current, adjustments: next });
  };

  const create = async () => {
    try {
      const sheet = await newSheet("新角色");
      await refresh();
      setCurrent(sheet);
      setSaved(true);
    } catch (e) {
      setError(String(e));
    }
  };

  const remove = async (summary: SheetSummary) => {
    // 使用者自己產生的資料，刪掉沒得復原。
    if (!window.confirm(`確定刪除〈${summary.name}〉？刪掉沒得復原。`)) return;
    try {
      await deleteSheet(summary.id);
      if (current?.id === summary.id) setCurrent(null);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const doImport = async () => {
    const path = await open({
      filters: [{ name: "角色卡", extensions: ["json"] }],
    });
    if (typeof path !== "string") return;
    try {
      const sheet = await importSheet(path);
      await refresh();
      setCurrent(sheet);
      setSaved(true);
    } catch (e) {
      setError(String(e));
    }
  };

  const doExport = async () => {
    if (!current) return;
    const path = await save({
      defaultPath: `${current.name}.json`,
      filters: [{ name: "角色卡", extensions: ["json"] }],
    });
    if (!path) return;
    exportSheet(current.id, path).catch((e) => setError(String(e)));
  };

  return (
    <div className="chapter-layout sheet-layout">
      <aside className="sheet-list">
        <div className="sheet-list-head">
          <button className="edit-submit" onClick={create}>
            新增
          </button>
          <button className="edit-cancel" onClick={doImport}>
            匯入
          </button>
        </div>
        {sheets === null && <p className="loading">載入中⋯</p>}
        {sheets?.length === 0 && <p className="sheet-note">還沒有角色卡。</p>}
        {sheets?.map((s) => (
          <div
            key={s.id}
            className={s.id === current?.id ? "sheet-row on" : "sheet-row"}
          >
            <button
              className="sheet-open"
              disabled={s.error !== null}
              onClick={() =>
                loadSheet(s.id)
                  .then((sheet) => {
                    setCurrent(sheet);
                    setSaved(true);
                  })
                  .catch((e) => setError(String(e)))
              }
            >
              {s.name}
              {s.error && <i className="sheet-broken">{s.error}</i>}
            </button>
            <button className="sheet-remove" onClick={() => remove(s)}>
              刪除
            </button>
          </div>
        ))}
      </aside>

      <div className="chapter-body">
        {error && <p className="error">{error}</p>}

        {!current && (
          <p className="loading">
            從左側選一張角色卡，或按「新增」建立一張。
          </p>
        )}

        {current && (
          <>
            <div className="sheet-head">
              <h2 className="path-name">{current.name}</h2>
              <span className="sheet-saved">{saved ? "已儲存" : "儲存中⋯"}</span>
              <button className="edit-cancel" onClick={doExport}>
                匯出
              </button>
            </div>

            <div className="sheet-columns">
              <SheetForm sheet={current} derived={derived} onChange={edit} />
              {derived && (
                <DerivedPanel
                  derived={derived}
                  adjustments={current.adjustments}
                  onAdjust={adjust}
                />
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
