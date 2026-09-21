import { useEffect, useRef, useState } from "react";

export interface NavItem {
  id: string;
  label: string;
  count: number;
}

/**
 * 右側小目錄，跟著捲動高亮目前所在的小節。
 *
 * 用 IntersectionObserver 而不是監聽 scroll 事件 —— scroll 會在每一幀
 * 重算所有小節的位置，法師那一章有 9 個小節、牧師 11 個，逐幀重算是白費。
 */
export default function SectionNav({
  title,
  items,
  scrollRoot,
}: {
  title: string;
  items: NavItem[];
  scrollRoot: React.RefObject<HTMLElement | null>;
}) {
  const [active, setActive] = useState<string | null>(null);
  const visible = useRef<Set<string>>(new Set());

  useEffect(() => {
    const root = scrollRoot.current;
    if (!root || items.length === 0) return;

    visible.current.clear();
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          const id = entry.target.getAttribute("data-section-id");
          if (!id) continue;
          if (entry.isIntersecting) visible.current.add(id);
          else visible.current.delete(id);
        }
        // 同時有數個小節在畫面上時取最靠前的那個，順序以 items 為準。
        const first = items.find((i) => visible.current.has(i.id));
        if (first) setActive(first.id);
      },
      { root, rootMargin: "-80px 0px -60% 0px", threshold: 0 },
    );

    for (const item of items) {
      const el = root.querySelector(`[data-section-id="${CSS.escape(item.id)}"]`);
      if (el) observer.observe(el);
    }
    return () => observer.disconnect();
  }, [items, scrollRoot]);

  if (items.length === 0) return null;

  return (
    <nav className="section-nav" aria-label={title}>
      <div className="expand-label">{title}</div>
      {items.map((item) => (
        <button
          key={item.id}
          className={item.id === active ? "section-link on" : "section-link"}
          onClick={() => {
            const el = scrollRoot.current?.querySelector(
              `[data-section-id="${CSS.escape(item.id)}"]`,
            );
            el?.scrollIntoView({ behavior: "smooth", block: "start" });
          }}
        >
          <span>{item.label}</span>
          <i>{item.count}</i>
        </button>
      ))}
    </nav>
  );
}
