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
  | "errata"
  | "history"
  | "sheets";

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

// 種族 --------------------------------------------------------------------

export interface AttrModifier {
  attr: string;
  delta: number;
}

export interface Race {
  id: string;
  name: string;
  /** 非數值時為 null，原文保留在 cp_raw（〈原初-星之幼體〉是「劇情取得」）。 */
  cp_cost: number | null;
  cp_raw: string;
  modifiers: AttrModifier[];
  attr_text: string | null;
  racial_feat_text: string | null;
  skill_mod_text: string | null;
  special_text: string | null;
  source_sheet: string;
  source_row: number;
}

export interface RaceChapter {
  races: Race[];
}

// 物品 --------------------------------------------------------------------

export interface AffixRank {
  rank: number;
  plus_cost: number;
  /** 非空代表這是同一階在不同裝備上的價碼，不是另一個階級。 */
  condition: string;
}

export interface Affix {
  id: string;
  name: string;
  slots: string[];
  ranks: AffixRank[];
  plus_cost_raw: string | null;
  effect: string;
  source_sheet: string;
  source_row: number;
}

export interface AffixChapter {
  tier: string;
  intro: string[];
  affixes: Affix[];
}

export interface MaterialAffix {
  id: string;
  name: string;
  tier_rank: number;
  rarity_multiplier: number | null;
  roll_min: number;
  roll_max: number;
  effect: string;
  /** 編輯時要用來定位勘誤。與母素材同一張工作表。 */
  source_sheet: string;
  source_row: number;
}

export interface Material {
  id: string;
  name: string;
  material_tier: number | null;
  cost_multiplier: number | null;
  slots: string[];
  affixes: MaterialAffix[];
  source_sheet: string;
  source_row: number;
}

export interface MaterialChapter {
  intro: string[];
  materials: Material[];
}

export interface DistributionGroup {
  slot: string;
  plus_label: string;
  affixes: string[];
}

// 對照表 ------------------------------------------------------------------

export interface RefTable {
  id: string;
  sheet: string;
  name: string;
  note: string | null;
  columns: string[];
  rows: string[][];
  source_row: number;
}

export interface RefTableSummary {
  id: string;
  sheet: string;
  name: string;
  row_count: number;
}

// 散文與勘誤 --------------------------------------------------------------

export interface ProseBlock {
  id: number;
  subsection: string | null;
  body: string;
}

export interface ProseSection {
  title: string | null;
  blocks: ProseBlock[];
}

export interface ProseChapter {
  sheet: string;
  sections: ProseSection[];
  tables: RefTable[];
}

export interface ErrataEntry {
  sheet: string;
  source_row: number | null;
  field: string | null;
  action: string;
  raw_value: string | null;
  fixed_value: string | null;
  issue: string | null;
  reason: string;
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
  /** 並排區塊的欄號。編輯時要寫進勘誤的 col: 才定位得到正確的條目。 */
  source_col: number;
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

// CP 試算 ----------------------------------------------------------------

/**
 * 專長分組的中文名稱。資料庫存英文代碼，顯示一律用中文。
 *
 * 目錄（toc）本身就帶得出各頁籤的中文標題，所以一般情況不需要這張表；
 * 這裡是給「不在章節脈絡裡」的地方用的 —— 例如試算工具的挑選器，
 * 它列出的專長橫跨所有分組。
 */
export const GROUP_LABELS: Record<string, string> = {
  basic: "基本專長",
  general: "一般專長",
  advanced: "高級專長",
  metamagic: "超魔專長",
  crafting: "製作專長",
  legendary: "傳奇專長",
  class: "職業專長",
};

export interface FeatDifficulty {
  id: string;
  name: string;
  feat_group: string;
  difficulty: number | null;
  difficulty_raw: string | null;
  difficulty_scale: string;
  class_name: string | null;
}

export interface CpStep {
  level: number;
  /** 這一級本身的花費。單位依 scale：cp 是 CP，legend 是傳奇技能點。 */
  step_cost: number;
  /** 從 1 級累計到本級。等級 0 不計入。 */
  cumulative: number;
  /** 累計換算成 CP。scale 為 cp 時與 cumulative 相同。 */
  cumulative_cp: number;
}

export interface CpPlan {
  difficulty: number;
  scale: string;
  steps: CpStep[];
}

// 編輯 --------------------------------------------------------------------
//
// 只在開發模式有作用。打包版的 editing_enabled 為 false，入口不會渲染。

export interface EditingStatus {
  enabled: boolean;
  python_ok: boolean;
  /** Python 不可用時的說明，讓按鈕在按下去之前就知道自己不能用。 */
  python_hint: string | null;
}

export type FieldKind = "text" | "number" | "string_list";

export interface EditableField {
  name: string;
  label: string;
  kind: FieldKind;
  /** 有值時要做成選項而不是自由輸入 */
  options: string[] | null;
  hint: string | null;
}

export interface EditRequest {
  entry_kind: string;
  sheet: string;
  row: number;
  /** 有 source_col 的條目才帶；詞綴與素材詞綴沒有這個概念。 */
  col: number | null;
  reason: string;
  /** 只含有改動的欄位。 */
  changes: Record<string, unknown>;
}

export interface RebuildResult {
  ok: boolean;
  /** 成功時是建置摘要，失敗時是 build_db.py 的 stderr 原文。 */
  output: string;
}

export interface ErrataCommit {
  hash: string;
  subject: string;
  date: string;
}

export interface ErrataHistory {
  uncommitted: string;
  commits: ErrataCommit[];
  unavailable: string | null;
}

// 角色卡 ------------------------------------------------------------------
//
// 這是本專案第一個「使用者資料」—— 規則書資料庫是建置產物、勘誤在 repo 裡，
// 角色卡兩者都不是：它屬於玩家，存在使用者資料夾。

/** 一條專長屬於哪一軌。見 docs/規則裁示紀錄.md 第 12 項。 */
export type Track = "melee" | "spell" | "none";

export const TRACK_LABELS: Record<Track, string> = {
  melee: "進戰",
  spell: "法術",
  none: "皆非",
};

export interface SheetFeat {
  feat_id: string;
  level: number;
  track: Track;
}

/** 獎勵 HP/SP 的擲骰結果。程式算得出該擲幾顆，算不出點數。 */
export interface BonusRolls {
  melee_hp: number;
  melee_sp: number;
  spell_hp: number;
  spell_sp: number;
}

export interface Adjustment {
  delta: number;
  note: string;
}

export interface Sheet {
  schema_version: number;
  id: string;
  name: string;
  starting_cp: number;
  attributes: Record<string, number>;
  race_id: string | null;
  feats: SheetFeat[];
  bonus: BonusRolls;
  manual_hp: number;
  manual_sp: number;
  extra_hp: number;
  extra_sp: number;
  adjustments: Record<string, Adjustment>;
  notes: string;
}

export interface SheetSummary {
  id: string;
  name: string;
  /** 讀不起來的卡也會列出來並說明原因，不會靜靜消失。 */
  error: string | null;
}

export interface DerivedValue {
  label: string;
  formula: number;
  adjustment: number;
  note: string;
  /** 顯示值 = 公式值 + 修正 */
  total: number;
}

export interface CpSummary {
  starting: number;
  attribute_adjustment: number;
  race: number;
  feats: number;
  legend_as_cp: number;
  extra_hp: number;
  extra_sp: number;
  available: number;
  spent: number;
  remaining: number;
  melee_invested: number;
  spell_invested: number;
}

export interface PoolSummary {
  attribute_part: number;
  rolled: number;
  manual: number;
  base: number;
  extra_target: number;
  extra_cost: number;
  total: number;
}

export interface Derived {
  modifiers: Record<string, number>;
  skills: DerivedValue[];
  resists: DerivedValue[];
  specials: DerivedValue[];
  cp: CpSummary;
  hp: PoolSummary;
  sp: PoolSummary;
  melee_dice_steps: number;
  spell_dice_steps: number;
  /** 軟性警告。一律只提示、不擋存檔。 */
  warnings: string[];
}

/** 九大屬性的顯示順序與中文名。資料庫的 attribute 表是權威清單。 */
export const ATTRIBUTES: { code: string; name: string }[] = [
  { code: "STR", name: "力量" },
  { code: "DEX", name: "敏捷" },
  { code: "SKI", name: "技巧" },
  { code: "CON", name: "體質" },
  { code: "RES", name: "抗力" },
  { code: "INT", name: "智力" },
  { code: "WIS", name: "智慧" },
  { code: "CHA", name: "魅力" },
  { code: "SPI", name: "精神" },
];

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

export const getRaceChapter = () => invoke<RaceChapter>("race_chapter");

export const getAffixChapter = (tier: string) =>
  invoke<AffixChapter>("affix_chapter", { tier });

export const getMaterialChapter = () =>
  invoke<MaterialChapter>("material_chapter");

export const getAffixDistribution = () =>
  invoke<DistributionGroup[]>("affix_distribution");

export const getRefSheet = (sheet: string) =>
  invoke<RefTable[]>("ref_sheet", { sheet });

export const getRefIndex = () => invoke<RefTableSummary[]>("ref_index");

export const getProseChapter = (sheet: string) =>
  invoke<ProseChapter>("prose_chapter", { sheet });

export const getErrataList = () => invoke<ErrataEntry[]>("errata_list");

export const getFeatDifficulties = () =>
  invoke<FeatDifficulty[]>("feat_difficulties");

export const getCpPlan = (difficulty: number, scale: string) =>
  invoke<CpPlan>("cp_plan", { difficulty, scale });

export const getEditingStatus = () => invoke<EditingStatus>("editing_status");

export const getEditableFields = (entryKind: string) =>
  invoke<EditableField[]>("editable_fields", { entryKind });

export const appendErrata = (request: EditRequest) =>
  invoke<RebuildResult>("append_errata", { request });

export const getErrataHistory = () => invoke<ErrataHistory>("errata_history");

export const listSheets = () => invoke<SheetSummary[]>("list_sheets");
export const loadSheet = (id: string) => invoke<Sheet>("load_sheet", { id });
export const saveSheet = (sheet: Sheet) => invoke<void>("save_sheet", { sheet });
export const newSheet = (name: string) => invoke<Sheet>("new_sheet", { name });
export const deleteSheet = (id: string) => invoke<void>("delete_sheet", { id });
export const importSheet = (path: string) => invoke<Sheet>("import_sheet", { path });
export const exportSheet = (id: string, path: string) =>
  invoke<void>("export_sheet", { id, path });
export const deriveSheet = (sheet: Sheet) => invoke<Derived>("derive_sheet", { sheet });

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
