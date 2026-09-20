/**
 * Rust 後端指令的型別化包裝。
 *
 * 這裡的型別必須與 src-tauri/src/db.rs 的 Serialize 結構一致；
 * 兩邊不同步時 TypeScript 不會報錯，只會在執行期拿到 undefined，
 * 因此新增欄位時務必兩邊一起改。
 */

import { invoke } from "@tauri-apps/api/core";

export type FeatGroup =
  | "basic"
  | "general"
  | "advanced"
  | "metamagic"
  | "crafting"
  | "legendary"
  | "class";

/** 專長分組的中文名稱與排序。資料庫存的是英文代碼，顯示一律用中文。 */
export const GROUP_LABELS: Record<string, string> = {
  basic: "基本專長",
  general: "一般專長",
  advanced: "高級專長",
  metamagic: "超魔專長",
  crafting: "製作專長",
  legendary: "傳奇專長",
  class: "職業專長",
};

export const GROUP_ORDER: FeatGroup[] = [
  "basic",
  "general",
  "advanced",
  "metamagic",
  "crafting",
  "legendary",
  "class",
];

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

export interface FeatSummary {
  id: string;
  name: string;
  feat_group: string;
  difficulty: number | null;
  difficulty_raw: string | null;
  difficulty_scale: string;
  categories: string[];
  class_path: string | null;
  effect_preview: string;
}

export interface Prereq {
  kind: string;
  ref_feat_id: string | null;
  ref_feat_name: string | null;
  ref_code: string | null;
  min_level: number | null;
  raw_text: string;
}

export interface Dependent {
  id: string;
  name: string;
  min_level: number | null;
}

export interface CpStep {
  level: number;
  step_cost: number;
  cumulative: number;
}

export interface ErrataNote {
  action: string;
  field: string | null;
  issue: string | null;
  reason: string;
}

export interface FeatDetail {
  id: string;
  name: string;
  feat_group: string;
  difficulty: number | null;
  difficulty_raw: string | null;
  difficulty_scale: string;
  effect: string;
  categories: string[];
  tags: string[];
  class_name: string | null;
  class_path: string | null;
  parent_name: string | null;
  prereqs: Prereq[];
  dependents: Dependent[];
  cp_table: CpStep[];
  source_sheet: string;
  source_row: number;
  errata: ErrataNote[];
}

export interface Facet {
  value: string;
  count: number;
}

export interface Facets {
  groups: Facet[];
  categories: Facet[];
  classes: Facet[];
}

export const getBuildInfo = () => invoke<BuildInfo>("build_info");
export const getFacets = () => invoke<Facets>("facets");

export const searchFeats = (
  query: string,
  groups: string[],
  categories: string[],
) => invoke<FeatSummary[]>("search_feats", { query, groups, categories });

export const getFeatDetail = (id: string) =>
  invoke<FeatDetail>("feat_detail", { id });
