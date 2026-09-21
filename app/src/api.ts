/**
 * Rust 後端指令的型別化包裝。
 *
 * 這裡的型別必須與 src-tauri/src/db/ 底下的 Serialize 結構一致；
 * 兩邊不同步時 TypeScript 不會報錯，只會在執行期拿到 undefined，
 * 因此新增欄位時務必兩邊一起改。
 *
 * 欄位名一律是 Rust 那側的 snake_case（serde 沒有改名）。指令參數則相反，
 * Tauri 會把 JS 的 camelCase 轉成 Rust 的 snake_case。
 */

import { invoke } from "@tauri-apps/api/core";

// 目錄 --------------------------------------------------------------------

/** 決定該用哪個章節元件渲染。與 db/toc.rs 的 Tab.kind 一致。 */
export type TabKind =
  | "prose"
  | "feats"
  | "class"
  | "race"
  | "affix"
  | "material"
  | "ref_sheet"
  | "affix_distribution"
  | "ref_index"
  | "cp_calc"
  | "errata";

export interface Tab {
  key: string;
  title: string;
  kind: TabKind;
  count: number;
}

export interface Chapter {
  key: string;
  title: string;
  tabs: Tab[];
}

export interface Toc {
  chapters: Chapter[];
}

// 條目 --------------------------------------------------------------------

export interface ChapterFeat {
  id: string;
  name: string;
  difficulty: number | null;
  difficulty_raw: string | null;
  difficulty_scale: string;
  categories: string[];
  tags: string[];
  effect: string;
  prereq_count: number;
  source_sheet: string;
  source_row: number;
}

export interface ClassTrait {
  id: string;
  name: string;
  description: string;
  source_sheet: string;
  source_row: number;
}

export interface Invocation {
  id: string;
  name: string;
  cost: number | null;
  cost_raw: string | null;
  prereq_raw: string | null;
  effect: string;
}

export interface ClassPath {
  id: string;
  name: string;
  description: string | null;
  feats: ChapterFeat[];
  traits: ClassTrait[];
}

export interface ClassChapter {
  class_name: string;
  path_kind: string;
  paths: ClassPath[];
  invocations: Invocation[];
}

export interface CategorySection {
  category: string;
  feats: ChapterFeat[];
}

export interface FeatChapter {
  group: string;
  intro: string[];
  sections: CategorySection[];
  uncategorized: ChapterFeat[];
}

// 展開區 ------------------------------------------------------------------

/** 前置條件的種類。kind 決定這一條能不能自動驗證。 */
export const PREREQ_KIND_LABELS: Record<string, string> = {
  feat: "專長",
  attribute: "屬性門檻",
  caster_ring: "施法環數",
  path_level: "流派等級",
  stat: "判定數值",
  bloodline: "血脈",
  free: "需 DM 裁定",
};

export interface Prereq {
  kind: string;
  ref_feat_id: string | null;
  ref_feat_name: string | null;
  ref_code: string | null;
  min_level: number | null;
  raw_text: string;
  /** 被指向的專長在哪一章哪一頁。前置可能跨章，光有 id 跳不過去。 */
  ref_chapter: string | null;
  ref_tab: string | null;
}

export interface Dependent {
  id: string;
  name: string;
  min_level: number | null;
  chapter: string;
  tab: string;
}

export interface EntryDetail {
  id: string;
  kind: string;
  prereqs: Prereq[];
  dependents: Dependent[];
  source_sheet: string;
  source_row: number;
}

// 搜尋 --------------------------------------------------------------------

export interface SearchHit {
  kind: string;
  id: string;
  name: string;
  chapter: string;
  tab: string;
  context: string;
  preview: string;
  matched_name: boolean;
}

// 建置資訊 ----------------------------------------------------------------

export interface BuildInfo {
  source_file: string;
  source_sha256: string;
  schema_version: string;
  feats: number;
  races: number;
  affixes: number;
  class_paths: number;
  rule_texts: number;
}

// 指令 --------------------------------------------------------------------

export const getBuildInfo = () => invoke<BuildInfo>("build_info");
export const getToc = () => invoke<Toc>("toc");

export const getClassChapter = (className: string) =>
  invoke<ClassChapter>("class_chapter", { className });

export const getFeatChapter = (group: string) =>
  invoke<FeatChapter>("feat_chapter", { group });

export const getEntryDetail = (kind: string, id: string) =>
  invoke<EntryDetail>("entry_detail", { kind, id });

export const searchAll = (query: string) =>
  invoke<SearchHit[]>("search", { query });

// 顯示用的格式化 ----------------------------------------------------------

/**
 * 難度的顯示字串。
 *
 * 非數值難度（〈知識〉〈語言〉的 `1or2`）保留原始字串 —— 那兩條是元技能，
 * 依 2026-09-20 的裁示由 DM 個案裁定為 1 或 2，程式不該擅自填一個數字。
 * `legend` 是傳奇技能點而非 CP 難度，前綴「傳」以示區別。
 */
export function formatDifficulty(feat: {
  difficulty: number | null;
  difficulty_raw: string | null;
  difficulty_scale: string;
}): string {
  if (feat.difficulty === null) return feat.difficulty_raw ?? "—";
  const value = Number.isInteger(feat.difficulty)
    ? String(feat.difficulty)
    : feat.difficulty.toFixed(1);
  return feat.difficulty_scale === "legend" ? `傳${value}` : value;
}
