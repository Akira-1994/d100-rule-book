import { useEffect, useState } from "react";

import { ErrataHistory, getErrataHistory } from "../api";

/**
 * 修改紀錄。
 *
 * 變更歷史就是 git —— `data/errata/` 在 git 裡，歷史已經有了，自己再做一份
 * journal 只會多一個會跟 git 不同步的真相來源。
 *
 * **不自動 commit。** 應用只改檔案，`git add/commit` 由人決定：自動提交
 * 會讓一次誤按變成歷史的一部分。
 */
export default function HistoryChapter() {
  const [history, setHistory] = useState<ErrataHistory | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getErrataHistory().then(setHistory).catch((e) => setError(String(e)));
  }, []);

  if (error) return <p className="error">{error}</p>;
  if (!history) return <p className="loading">載入中⋯</p>;

  return (
    <div className="chapter-layout">
      <div className="chapter-body prose">
        {history.unavailable && (
          <p className="error">讀不到 git 紀錄：{history.unavailable}</p>
        )}

        <section className="path">
          <div className="eyebrow">尚未提交</div>
          <h2 className="path-name">工作目錄的變更</h2>
          {history.uncommitted.trim() === "" ? (
            <p className="path-desc">沒有未提交的勘誤變更。</p>
          ) : (
            <>
              <p className="path-desc">
                應用只改檔案，不會自動 commit —— 自動提交會讓一次誤按變成
                歷史的一部分。確認無誤後在終端機提交：
              </p>
              <pre className="history-command">
                git add data/errata && git commit
              </pre>
              <pre className="history-diff">{history.uncommitted}</pre>
            </>
          )}
        </section>

        <section className="path">
          <div className="eyebrow">已提交</div>
          <h2 className="path-name">最近的勘誤紀錄</h2>
          {history.commits.length === 0 ? (
            <p className="path-desc">還沒有動過 data/errata。</p>
          ) : (
            <div className="card-stack">
              {history.commits.map((c) => (
                <article key={c.hash} className="feat-card">
                  <div className="feat-head static">
                    <b className="feat-name">{c.subject}</b>
                    <span className="chip tag">{c.date}</span>
                    <span className="source">{c.hash}</span>
                  </div>
                </article>
              ))}
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
