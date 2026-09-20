-- D100 規則書 — SQLite 結構定義
--
-- 這份資料庫是「建置產物」：由 tools/build_db.py 從 data/raw 加上
-- data/errata 重新產生，任何時候都可以整個刪掉重建。不要直接改它，
-- 改 data/errata 底下的 YAML。
--
-- 設計原則
--   1. 每一筆資料都帶 source_sheet / source_row，永遠追得回原始試算表。
--   2. 原始字串一律保留（difficulty_raw、plus_cost_raw、prereq.raw_text），
--      結構化欄位解析不出來時留 NULL，絕不臆測。
--   3. 勘誤不是偷偷修掉，而是記在 errata 表裡，跟著資料庫一起出貨。

PRAGMA foreign_keys = ON;

-- 建置中繼資料：schema_version、source_sha256、extract_tool_version 等
CREATE TABLE build_info (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- 六大技能分類的權威清單。任何 feat_category.category 都必須在此。
CREATE TABLE category (
    code       TEXT PRIMARY KEY,
    formula    TEXT NOT NULL,
    sort_order INTEGER NOT NULL
);

-- 九大屬性
CREATE TABLE attribute (
    code       TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    sort_order INTEGER NOT NULL
);

-- 專長 ------------------------------------------------------------------

CREATE TABLE feat (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    feat_group     TEXT NOT NULL,   -- basic / general / advanced / metamagic
    difficulty     REAL,            -- 非固定難度時為 NULL
    difficulty_raw TEXT,            -- 原始字串，例如 '1or2'
    parent_id      TEXT REFERENCES feat(id),
    effect         TEXT NOT NULL,
    source_sheet   TEXT NOT NULL,
    source_row     INTEGER NOT NULL,
    UNIQUE (source_sheet, source_row)
);

CREATE INDEX idx_feat_group ON feat(feat_group);
CREATE INDEX idx_feat_name  ON feat(name);

-- 一個專長可屬於多個分類（原表的「戰鬥/運動」「知識、感知或交涉」）。
-- 以集合表達後，「感知/知識」與「知識/感知」自然收斂成同一組。
CREATE TABLE feat_category (
    feat_id  TEXT NOT NULL REFERENCES feat(id) ON DELETE CASCADE,
    category TEXT NOT NULL REFERENCES category(code),
    PRIMARY KEY (feat_id, category)
);

-- 前置條件。kind = feat 時 ref_feat_id / min_level 有值，可做自動驗證；
-- kind = attribute 時 ref_code 是屬性代碼、min_level 是門檻值（例如 DEX16）。
-- 尚未解析的 free 只保留 raw_text，僅供顯示不做驗證。
CREATE TABLE feat_prereq (
    feat_id     TEXT NOT NULL REFERENCES feat(id) ON DELETE CASCADE,
    seq         INTEGER NOT NULL,
    kind        TEXT NOT NULL,   -- feat / attribute / caster_ring / bloodline / free
    ref_feat_id TEXT REFERENCES feat(id),
    ref_code    TEXT,            -- 屬性代碼、血脈名稱，或尚未建檔的專長名稱
    min_level   INTEGER,
    raw_text    TEXT NOT NULL,
    PRIMARY KEY (feat_id, seq)
);

-- 種族 ------------------------------------------------------------------

CREATE TABLE race (
    id                TEXT PRIMARY KEY,
    name              TEXT NOT NULL,
    cp_cost           INTEGER,   -- 負值代表扣除；非數值時為 NULL
    cp_raw            TEXT NOT NULL,
    attr_text         TEXT,
    racial_feat_text  TEXT,
    skill_mod_text    TEXT,
    special_text      TEXT,
    source_sheet      TEXT NOT NULL,
    source_row        INTEGER NOT NULL,
    UNIQUE (source_sheet, source_row)
);

CREATE TABLE race_attr_modifier (
    race_id TEXT NOT NULL REFERENCES race(id) ON DELETE CASCADE,
    attr    TEXT NOT NULL REFERENCES attribute(code),
    delta   INTEGER NOT NULL,
    PRIMARY KEY (race_id, attr)
);

-- 詞綴 ------------------------------------------------------------------

CREATE TABLE affix (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    affix_tier    TEXT NOT NULL,   -- general / advanced / eternal
    plus_cost_raw TEXT,
    effect        TEXT NOT NULL,
    source_sheet  TEXT NOT NULL,
    source_row    INTEGER NOT NULL,
    UNIQUE (source_sheet, source_row)
);

CREATE INDEX idx_affix_tier ON affix(affix_tier);

CREATE TABLE affix_slot (
    affix_id TEXT NOT NULL REFERENCES affix(id) ON DELETE CASCADE,
    slot     TEXT NOT NULL,
    PRIMARY KEY (affix_id, slot)
);

-- 「2、4、6」代表三個由弱到強的階級，各自需要不同的魔法物品加值來兌換。
--
-- condition 用於依裝備類型而異的寫法，例如〈幽冥〉的「3（鎧甲、盾牌）／
-- 2（武器）」—— 那是同一階在不同裝備上的兩種價碼，不是兩個階級，
-- 所以它們的 rank 都是 1，差別只在 condition。
CREATE TABLE affix_rank (
    affix_id  TEXT NOT NULL REFERENCES affix(id) ON DELETE CASCADE,
    rank      INTEGER NOT NULL,
    plus_cost INTEGER NOT NULL,
    condition TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (affix_id, rank, condition)
);

-- 勘誤 ------------------------------------------------------------------

-- 每一筆對原始資料的修正或標記都留下紀錄，連同理由一起進資料庫，
-- 方便日後整理成清單回報給規則書作者。
CREATE TABLE errata (
    id           INTEGER PRIMARY KEY,
    sheet        TEXT NOT NULL,
    source_row   INTEGER,
    field        TEXT,
    action       TEXT NOT NULL,   -- set（修正）/ flag（僅標記）
    raw_value    TEXT,
    fixed_value  TEXT,
    issue        TEXT,            -- flag 的問題代碼
    reason       TEXT NOT NULL
);

-- 檢視 ------------------------------------------------------------------

-- 專長連同分類清單的常用檢視
CREATE VIEW feat_full AS
SELECT
    f.id,
    f.name,
    f.feat_group,
    f.difficulty,
    f.difficulty_raw,
    f.parent_id,
    (SELECT group_concat(fc.category, '/')
       FROM feat_category fc WHERE fc.feat_id = f.id) AS categories,
    (SELECT count(*) FROM feat_prereq p WHERE p.feat_id = f.id) AS prereq_count,
    f.effect,
    f.source_sheet,
    f.source_row
FROM feat f;
