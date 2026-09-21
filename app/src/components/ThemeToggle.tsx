import { useEffect, useState } from "react";

const KEY = "d100.theme";

type Choice = "light" | "dark" | null;

function read(): Choice {
  const v = localStorage.getItem(KEY);
  return v === "light" || v === "dark" ? v : null;
}

/**
 * 亮暗切換。
 *
 * 預設跟隨系統（不寫 data-theme，由 theme.css 的 prefers-color-scheme 決定）；
 * 使用者按過之後才寫死選擇並記進 localStorage。
 */
export default function ThemeToggle() {
  const [choice, setChoice] = useState<Choice>(read);

  useEffect(() => {
    const root = document.documentElement;
    if (choice) root.setAttribute("data-theme", choice);
    else root.removeAttribute("data-theme");
  }, [choice]);

  const isDark = choice
    ? choice === "dark"
    : window.matchMedia("(prefers-color-scheme: dark)").matches;

  return (
    <button
      className="icon-button"
      title={isDark ? "切換到羊皮紙暖白" : "切換到燭光皮革"}
      aria-label="切換主題"
      onClick={() => {
        const next: Choice = isDark ? "light" : "dark";
        localStorage.setItem(KEY, next);
        setChoice(next);
      }}
    >
      {isDark ? "☀" : "☾"}
    </button>
  );
}
