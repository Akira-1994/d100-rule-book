import { RefTable } from "../api";

/**
 * 對照表的通用渲染。
 *
 * 資料庫把 31 張查表統一收在 `ref_table` / `ref_row`，欄名與儲存格各是一個
 * 陣列 —— 欄位各不相同，硬要各建一張資料表並不划算。這裡就照著陣列畫，
 * 不對內容做任何判斷。
 */
export default function RefTableView({ table }: { table: RefTable }) {
  return (
    <figure className="ref-table" data-entry-id={table.id}>
      <figcaption>
        <b>{table.name}</b>
        {table.note && <span className="ref-note">{table.note}</span>}
      </figcaption>
      <div className="ref-scroll">
        <table>
          <thead>
            <tr>
              {table.columns.map((c, i) => (
                <th key={i}>{c}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {table.rows.map((row, i) => (
              <tr key={i}>
                {row.map((cell, j) => (
                  <td key={j}>{cell}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </figure>
  );
}
