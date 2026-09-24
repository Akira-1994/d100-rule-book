import { ReactNode, createContext, useContext } from "react";

import type { EditingStatus } from "./api";

/**
 * 編輯狀態與「改完之後要做什麼」。
 *
 * 用 context 而不是一路往下傳 props：需要它的是最深處的卡片，中間的章節
 * 元件完全不關心編輯這回事，讓它們多帶兩個參數只是雜訊。
 */
export interface Editing {
  status: EditingStatus;
  /** 重建成功後呼叫，由外殼負責重新抓資料並保持目前位置 */
  onApplied: () => void;
  /**
   * 重建進行中。這段期間資料庫連線是關著的（Windows 上檔案被開著就寫不
   * 進去），任何查詢都會拿到「資料庫正在重建」—— 外殼靠這個旗標把可能
   * 觸發查詢的入口先擋住，而不是讓人看到一片紅字。
   */
  rebuilding: boolean;
  setRebuilding: (value: boolean) => void;
}

const DISABLED: Editing = {
  status: { enabled: false, python_ok: false, python_hint: null },
  onApplied: () => {},
  rebuilding: false,
  setRebuilding: () => {},
};

const EditingContext = createContext<Editing>(DISABLED);

export function EditingProvider({
  value,
  children,
}: {
  value: Editing;
  children: ReactNode;
}) {
  return <EditingContext.Provider value={value}>{children}</EditingContext.Provider>;
}

export function useEditing(): Editing {
  return useContext(EditingContext);
}
