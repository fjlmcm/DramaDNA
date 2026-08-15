-- DramaDNA 领域层 —— 短剧逆向拆解。
-- 设计原则:每次模型调用只产出一类资产(asset_spec);dna_task 是最小恢复单元。

-- 剧目(一部剧 = 一个目录)。
CREATE TABLE dramas (
    id                 TEXT PRIMARY KEY,
    name               TEXT NOT NULL,               -- 剧名(目录名)
    dir_path           TEXT NOT NULL UNIQUE,        -- 剧目录绝对路径
    episode_count      INTEGER NOT NULL DEFAULT 0,
    total_duration_sec REAL NOT NULL DEFAULT 0,
    meta               TEXT NOT NULL DEFAULT '{}',  -- json: 备注等
    created_at         TEXT NOT NULL,
    updated_at         TEXT NOT NULL
);

-- 分集(从文件名"第NN集_标题 #标签.mp4"解析)。
CREATE TABLE episodes (
    id           TEXT PRIMARY KEY,
    drama_id     TEXT NOT NULL REFERENCES dramas(id) ON DELETE CASCADE,
    ep_no        INTEGER NOT NULL,
    title        TEXT NOT NULL DEFAULT '',          -- 标题钩子文案(去 #标签)
    file_path    TEXT NOT NULL,
    duration_sec REAL,
    width        INTEGER,
    height       INTEGER,
    UNIQUE(drama_id, ep_no)
);

-- 资产规格 —— 管线里"一件事"的定义。builtin 种子由代码插入(prompts.rs)。
CREATE TABLE asset_specs (
    id              TEXT PRIMARY KEY,
    stage           TEXT NOT NULL,        -- global(A) | episode(B) | synth(C) | adapt(D)
    sort_no         INTEGER NOT NULL,     -- 展示与输出排序
    name            TEXT NOT NULL,        -- 人物档案 / 台词原文 / …
    scope           TEXT NOT NULL,        -- per_segment | per_episode | per_drama
    prompt          TEXT NOT NULL,
    merge_prompt    TEXT,                 -- per_segment 资产的分段合并 prompt(空用内置模板)
    model_id        TEXT REFERENCES models(id) ON DELETE SET NULL,  -- 空 = 管线默认模型
    inputs          TEXT NOT NULL DEFAULT '[]',
        -- json 数组,依赖资产:"spec_id"(同集/最终稿) 或 "spec_id:all"(聚合全部集)
    output_template TEXT NOT NULL,        -- 相对拆解目录,如 "分集/第{ep}集-台词原文.md"
    needs_video     INTEGER NOT NULL DEFAULT 0,  -- 1=调用携带视频(A/B);0=纯文本(C/D)
    user_input      INTEGER NOT NULL DEFAULT 0,  -- 1=需用户输入触发(如新剧大纲的新设定)
    enabled         INTEGER NOT NULL DEFAULT 1,
    builtin         INTEGER NOT NULL DEFAULT 0,
    params          TEXT NOT NULL DEFAULT '{}',  -- json: 覆盖请求参数(max_tokens 等)
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

-- 拆解任务单元:剧 × 资产 × (集|段|合并)。
--   per_episode → episode_id 非空;
--   per_segment → segment_no 非空(1 起),合并任务 segment_no = 0;
--   per_drama   → 两者皆空。
CREATE TABLE dna_tasks (
    id          TEXT PRIMARY KEY,
    drama_id    TEXT NOT NULL REFERENCES dramas(id) ON DELETE CASCADE,
    spec_id     TEXT NOT NULL REFERENCES asset_specs(id) ON DELETE CASCADE,
    episode_id  TEXT REFERENCES episodes(id) ON DELETE CASCADE,
    segment_no  INTEGER,
    user_input  TEXT,                     -- adapt 阶段的用户输入(如新设定)
    status      TEXT NOT NULL DEFAULT 'pending',  -- pending|processing|done|failed
    result_text TEXT,
    error       TEXT,
    attempts    INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- 同一(剧,资产,集,段)唯一 —— NULL 参与唯一性需借助表达式索引。
CREATE UNIQUE INDEX idx_dna_tasks_unit
    ON dna_tasks(drama_id, spec_id, ifnull(episode_id, ''), ifnull(segment_no, -1));

CREATE INDEX idx_episodes_drama    ON episodes(drama_id, ep_no);
CREATE INDEX idx_dna_tasks_drama   ON dna_tasks(drama_id, status);
CREATE INDEX idx_dna_tasks_spec    ON dna_tasks(drama_id, spec_id);
