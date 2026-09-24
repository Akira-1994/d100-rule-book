import { useState } from "react";

import EditDialog from "./EditDialog";
import { useEditing } from "../editing";

/**
 * 條目上的編輯入口。
 *
 * 把「只在開發模式出現、Python 不在時停用、按下去開對話框」這組邏輯收在
 * 一個地方 —— 專長、詞綴、素材詞綴三種卡片都用它，散在三處遲早會有一處
 * 忘了做 dev 判定。
 */
export default function EditButton({
  entryKind,
  entryName,
  sheet,
  row,
  col = null,
  current,
}: {
  entryKind: string;
  entryName: string;
  sheet: string;
  row: number;
  /** 只有並排區塊的條目（專長）有欄號，詞綴類沒有。 */
  col?: number | null;
  current: Record<string, unknown>;
}) {
  const [open, setOpen] = useState(false);
  const edit = useEditing();

  if (!edit.status.enabled) return null;

  return (
    <>
      <button
        className="edit-open"
        disabled={!edit.status.python_ok || edit.rebuilding}
        title={edit.status.python_hint ?? undefined}
        onClick={() => setOpen(true)}
      >
        編輯
      </button>
      {open && (
        <EditDialog
          entryKind={entryKind}
          entryName={entryName}
          sheet={sheet}
          row={row}
          col={col}
          current={current}
          onClose={() => setOpen(false)}
          onApplied={edit.onApplied}
          onRebuilding={edit.setRebuilding}
        />
      )}
    </>
  );
}
