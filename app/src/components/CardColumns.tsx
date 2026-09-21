import { ReactNode, useMemo } from "react";

/**
 * 兩欄卡片。
 *
 * 不用 CSS grid 排兩欄 —— grid 的每一列高度等於該列較高的那張卡，矮的那張
 * 下方會留白，列與列之間因此出現參差的溝；而且展開左欄的卡片會把右欄整個
 * 往下推。
 *
 * 改成把清單切成兩段、左右各自是一疊獨立的卡：
 *   - 沒有「列」的概念，就沒有參差的溝
 *   - 展開只推動同一欄下方的卡，另一欄完全不動
 *
 * 切法是「前 k 筆給左欄、其餘給右欄」而不是交錯分配，所以閱讀順序仍然是
 * 原表順序：難度 1、2、3 在左欄由上往下，4、5 在右欄，跟報紙分欄一樣。
 *
 * 平衡點只用基準內容算一次，展開後不重算 —— 否則卡片會在展開的瞬間跳位。
 */
export default function CardColumns<T>({
  items,
  weight,
  keyOf,
  render,
}: {
  items: T[];
  /** 這張卡大概佔多少高度，用來找平衡點。粗估即可。 */
  weight: (item: T) => number;
  keyOf: (item: T) => string;
  render: (item: T) => ReactNode;
}) {
  // 只看 items：weight 每次 render 都是新的箭頭函式，放進相依陣列等於沒有
  // memo。切點本來就只該隨內容變，展開與否不影響。
  const split = useMemo(
    () => splitPoint(items.map(weight)),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [items],
  );

  const left = items.slice(0, split);
  const right = items.slice(split);

  return (
    <div className="card-columns">
      <div className="card-column">
        {left.map((item) => (
          <div key={keyOf(item)}>{render(item)}</div>
        ))}
      </div>
      {right.length > 0 && (
        <div className="card-column">
          {right.map((item) => (
            <div key={keyOf(item)}>{render(item)}</div>
          ))}
        </div>
      )}
    </div>
  );
}

/**
 * 找出讓左右兩段總高度最接近的切點。
 *
 * 保持順序（不是裝箱問題），所以掃一遍前綴和取差值最小的位置即可。
 * 至少留一筆給左欄，否則只有一張卡時會整個跑到右欄去。
 */
function splitPoint(weights: number[]): number {
  if (weights.length <= 1) return weights.length;

  const total = weights.reduce((a, b) => a + b, 0);
  let running = 0;
  let best = 1;
  let bestGap = Infinity;

  for (let i = 1; i < weights.length; i++) {
    running += weights[i - 1];
    const gap = Math.abs(running - (total - running));
    if (gap < bestGap) {
      bestGap = gap;
      best = i;
    }
  }
  return best;
}

/**
 * 以字數粗估卡片高度。
 *
 * 一欄大約容得下 38 個中文字，再加上標題列與間距約當三行。不必準，
 * 只要能把「八行的卡」和「兩行的卡」分出來就夠了。
 */
export function estimateLines(text: string): number {
  const hard = text.split("\n").length - 1;
  return Math.ceil(text.length / 38) + hard + 3;
}
