import { useEffect, useState } from "react";

import {
  EditRequest,
  EditableField,
  RebuildResult,
  appendErrata,
  getEditableFields,
} from "../api";

/**
 * 單一條目的編輯表單。
 *
 * 送出後會追加一筆勘誤到 `data/errata/<工作表>.yaml` 並重跑
 * `tools/build_db.py` —— 也就是專案原本的流程，只是不必離開應用。
 *
 * **只送出有改動的欄位。** 沒動的欄位不該出現在勘誤裡，否則清單上會是
 * 一堆「把 X 改成 X」的雜訊，而那份清單是要拿去跟規則書作者對帳的。
 */
export default function EditDialog({
  entryKind,
  entryName,
  sheet,
  row,
  col,
  current,
  onClose,
  onApplied,
  onRebuilding,
}: {
  entryKind: string;
  entryName: string;
  sheet: string;
  row: number;
  col: number | null;
  /** 條目的現值，用來預填表單並判斷哪些欄位真的改了 */
  current: Record<string, unknown>;
  onClose: () => void;
  onApplied: () => void;
  /** 回報重建進行中，讓外殼擋住其他會觸發查詢的入口 */
  onRebuilding: (value: boolean) => void;
}) {
  const [fields, setFields] = useState<EditableField[] | null>(null);
  const [draft, setDraft] = useState<Record<string, string>>({});
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<RebuildResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getEditableFields(entryKind)
      .then((f) => {
        setFields(f);
        const initial: Record<string, string> = {};
        for (const field of f) initial[field.name] = toText(current[field.name]);
        setDraft(initial);
      })
      .catch((e) => setError(String(e)));
  }, [entryKind, current]);

  const changed = fields
    ? fields.filter((f) => draft[f.name] !== toText(current[f.name]))
    : [];

  const submit = async () => {
    if (!fields) return;
    setBusy(true);
    onRebuilding(true);
    setError(null);
    setResult(null);
    try {
      const changes: Record<string, unknown> = {};
      for (const field of changed) {
        changes[field.name] = fromText(field, draft[field.name]);
      }
      const request: EditRequest = { entry_kind: entryKind, sheet, row, col, reason, changes };
      const res = await appendErrata(request);
      setResult(res);
      if (res.ok) onApplied();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      onRebuilding(false);
    }
  };

  return (
    <div className="palette-backdrop" onClick={busy ? undefined : onClose}>
      <div
        className="edit-dialog"
        role="dialog"
        aria-label={`編輯 ${entryName}`}
        onClick={(e) => e.stopPropagation()}
      >
        <header className="edit-head">
          <b>編輯〈{entryName}〉</b>
          <span className="source">
            {sheet} R{row}
            {col !== null && ` · C${col}`}
          </span>
        </header>

        {!fields && <p className="loading">載入中⋯</p>}

        {fields && (
          <div className="edit-body">
            {fields.map((field) => (
              <label key={field.name} className="edit-field">
                <span className="edit-label">
                  {field.label}
                  {field.hint && <i>{field.hint}</i>}
                </span>
                {field.options ? (
                  <MultiSelect
                    options={field.options}
                    value={draft[field.name] ?? ""}
                    onChange={(v) => setDraft({ ...draft, [field.name]: v })}
                  />
                ) : field.kind === "text" ? (
                  <textarea
                    rows={field.name === "effect" ? 5 : 2}
                    value={draft[field.name] ?? ""}
                    onChange={(e) => setDraft({ ...draft, [field.name]: e.target.value })}
                  />
                ) : (
                  <input
                    type="number"
                    step="0.5"
                    value={draft[field.name] ?? ""}
                    onChange={(e) => setDraft({ ...draft, [field.name]: e.target.value })}
                  />
                )}
              </label>
            ))}

            <label className="edit-field">
              <span className="edit-label">
                理由<i>必填。這份清單是要拿去跟規則書作者對帳的</i>
              </span>
              <textarea
                rows={3}
                value={reason}
                onChange={(e) => setReason(e.target.value)}
                placeholder="為什麼要改？寫給人看，不是寫給程式看。"
              />
            </label>
          </div>
        )}

        {error && <p className="error">{error}</p>}

        {result && (
          <p className={result.ok ? "edit-ok" : "error"}>
            {result.ok ? "已重建資料庫。" : "重建失敗，勘誤已還原："}
            {result.output && <pre className="edit-output">{result.output}</pre>}
          </p>
        )}

        <footer className="edit-foot">
          <span className="edit-summary">
            {changed.length === 0
              ? "尚未改動任何欄位"
              : `將寫入 ${changed.map((f) => f.label).join("、")}`}
          </span>
          <button className="edit-cancel" onClick={onClose} disabled={busy}>
            {result?.ok ? "關閉" : "取消"}
          </button>
          <button
            className="edit-submit"
            onClick={submit}
            disabled={busy || changed.length === 0 || reason.trim() === ""}
          >
            {busy ? "重建中⋯" : "寫入勘誤並重建"}
          </button>
        </footer>
      </div>
    </div>
  );
}

/** 分類這種有權威清單的欄位做成多選，不讓人自由輸入打錯字。 */
function MultiSelect({
  options,
  value,
  onChange,
}: {
  options: string[];
  value: string;
  onChange: (value: string) => void;
}) {
  const selected = value === "" ? [] : value.split("、");
  return (
    <div className="edit-options">
      {options.map((option) => {
        const on = selected.includes(option);
        return (
          <button
            key={option}
            type="button"
            className={on ? "chip on" : "chip"}
            onClick={() =>
              onChange(
                (on
                  ? selected.filter((s) => s !== option)
                  : [...selected, option]
                ).join("、"),
              )
            }
          >
            {option}
          </button>
        );
      })}
    </div>
  );
}

/** 現值轉成輸入框裡的文字。清單以「、」相接，與規則書的寫法一致。 */
function toText(value: unknown): string {
  if (value === null || value === undefined) return "";
  if (Array.isArray(value)) return value.join("、");
  return String(value);
}

/** 輸入框的文字轉回送給後端的型別。型別不對的話後端會擋下來。 */
function fromText(field: EditableField, text: string): unknown {
  if (field.kind === "number") return Number(text);
  if (field.kind === "string_list") {
    return text === "" ? [] : text.split("、").map((s) => s.trim()).filter(Boolean);
  }
  return text;
}
