-- DramaDNA 初始 schema —— 7 张表。
-- 时间戳统一存 RFC3339 文本(UTC)。json 字段存字符串,Rust 侧按需解析。

-- 模型供应商(含中转站)。api_key 明文存储 —— 用户明确选择。
CREATE TABLE providers (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    kind         TEXT NOT NULL,                 -- openai_compatible | volcengine | dashscope | gemini
    base_url     TEXT NOT NULL,
    api_key      TEXT NOT NULL DEFAULT '',
    extra_config TEXT NOT NULL DEFAULT '{}',    -- json
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

-- 某供应商下的具体模型。
CREATE TABLE models (
    id                 TEXT PRIMARY KEY,
    provider_id        TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    model_id           TEXT NOT NULL,            -- 供应商侧标识,如 doubao-seed-2-0-lite-260428
    display_name       TEXT NOT NULL,
    video_input_method TEXT NOT NULL DEFAULT 'file_api', -- file_api | base64 | url
    constraints        TEXT NOT NULL DEFAULT '{}',       -- json: VideoConstraints
    params             TEXT NOT NULL DEFAULT '{}',       -- json: 默认请求参数
    enabled            INTEGER NOT NULL DEFAULT 1,
    created_at         TEXT NOT NULL,
    updated_at         TEXT NOT NULL
);

-- 方案 = 单个模型 + 提示词。
CREATE TABLE schemes (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    model_id   TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE,
    prompt     TEXT NOT NULL,
    params     TEXT NOT NULL DEFAULT '{}',       -- json: 覆盖模型默认参数
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- 批量任务。
CREATE TABLE batch_jobs (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    scheme_id   TEXT NOT NULL REFERENCES schemes(id),
    status      TEXT NOT NULL DEFAULT 'pending', -- pending|running|paused|done|failed|cancelled
    total_items INTEGER NOT NULL DEFAULT 0,
    done_items  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    finished_at TEXT
);

-- 批量任务的最小恢复单元:一个(文件 × 方案)。
CREATE TABLE job_items (
    id                TEXT PRIMARY KEY,
    job_id            TEXT NOT NULL REFERENCES batch_jobs(id) ON DELETE CASCADE,
    file_path         TEXT NOT NULL,
    file_hash         TEXT,
    status            TEXT NOT NULL DEFAULT 'pending',
        -- pending|preprocessing|uploading|running|done|failed|cancelled
    preprocessed_path TEXT,
    uploaded_ref      TEXT,
    result_text       TEXT,
    error             TEXT,
    attempts          INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL
);

-- 视频理解 Tab 的单次运行记录(执行日志用)。
CREATE TABLE runs (
    id          TEXT PRIMARY KEY,
    scheme_id   TEXT REFERENCES schemes(id) ON DELETE SET NULL,
    scheme_name TEXT NOT NULL DEFAULT '',        -- 快照,方案删除后仍可读
    model_label TEXT NOT NULL DEFAULT '',
    file_path   TEXT NOT NULL,
    prompt      TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL DEFAULT 'running', -- running|done|failed|cancelled
    result_text TEXT,
    error       TEXT,
    duration_ms INTEGER,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- 应用日志。
CREATE TABLE logs (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    level      TEXT NOT NULL,                    -- info|warn|error
    source     TEXT NOT NULL DEFAULT '',
    message    TEXT NOT NULL,
    context    TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL
);

CREATE INDEX idx_models_provider   ON models(provider_id);
CREATE INDEX idx_schemes_model     ON schemes(model_id);
CREATE INDEX idx_job_items_job     ON job_items(job_id);
CREATE INDEX idx_job_items_status  ON job_items(status);
CREATE INDEX idx_runs_created      ON runs(created_at);
CREATE INDEX idx_logs_created      ON logs(created_at);
