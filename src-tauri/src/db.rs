use std::path::Path;
use std::str::FromStr;

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};

/// 数据库句柄。作为 Tauri managed state 注入,供所有 command 使用。
/// Clone 仅克隆连接池句柄(内部 Arc),供后台批处理任务持有。
#[derive(Clone)]
pub struct Db {
    pub pool: SqlitePool,
}

impl Db {
    /// 连接(或创建)指定路径的 SQLite 库并跑完所有 migration。
    pub async fn connect(db_path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let url = format!("sqlite://{}", db_path.display());
        let options = SqliteConnectOptions::from_str(&url)?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

        let pool = SqlitePool::connect_with(options).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        seed_defaults(&pool).await?;
        seed_asset_specs(&pool).await?;
        Ok(Self { pool })
    }

    /// 在应用数据目录下初始化 dramadna.db。
    pub async fn init(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let dir = app.path().app_data_dir()?;
        std::fs::create_dir_all(&dir)?;
        let db_path = dir.join("dramadna.db");
        log::info!("数据库路径: {}", db_path.display());
        Self::connect(&db_path).await
    }
}

/// 首次启动写入三家预设供应商与模型骨架(不含 api_key —— 由用户在设置页填写)。
///
/// 用 settings 标志保证只执行一次:用户删除某个预设后不会重新出现。
/// 若库中已有供应商(老用户升级到本版本),只打标志、不写入,避免叠加。
async fn seed_defaults(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    const SEED_FLAG: &str = "default_seeded";

    let done: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
        .bind(SEED_FLAG)
        .fetch_optional(pool)
        .await?;
    if done.is_some() {
        return Ok(());
    }

    let (provider_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM providers")
        .fetch_one(pool)
        .await?;

    if provider_count == 0 {
        let now = chrono::Utc::now().to_rfc3339();

        // (id, 名称, kind, base_url) —— api_key 留空。
        let providers = [
            (
                "seed-volc",
                "火山引擎 Ark",
                "volcengine",
                "https://ark.cn-beijing.volces.com/api/v3",
            ),
            (
                "seed-ali",
                "阿里百炼",
                "dashscope",
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
            ),
            ("seed-yun", "云雾中转站", "gemini", "https://yunwu.ai/v1"),
            (
                "seed-xiaomi",
                "小米 MiMo",
                "xiaomi",
                "https://api.xiaomimimo.com/v1",
            ),
        ];
        for (id, name, kind, base_url) in providers {
            sqlx::query(
                "INSERT INTO providers
                 (id, name, kind, base_url, api_key, extra_config, created_at, updated_at)
                 VALUES (?, ?, ?, ?, '', '{}', ?, ?)",
            )
            .bind(id)
            .bind(name)
            .bind(kind)
            .bind(base_url)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await?;
        }

        // (id, provider_id, model_id, 显示名, 视频限制 JSON)
        // 各家 max_bytes 按 2026-05 实测的 API 上限反推(留余量):
        //   豆包请求体 64MB → 原始 ≤ 45MB,设 43 MiB
        //   通义 data-uri 20MB → 原始 ≤ 15MB,设 14 MiB
        //   云雾(中转)base64 实测 ~13MB ✓ / 15MB ✗ → 原始 ≤ 10MB,设 8 MiB 给中转抖动留余量
        //   小米 base64 文档上限 50MB → 原始 ≤ 37MB,设 35 MiB 留余量
        let models = [
            (
                "seed-volc-m",
                "seed-volc",
                "doubao-seed-2-0-lite-260428",
                "豆包 Seed 2.0 Lite",
                "base64",
                r#"{"maxBytes":45088768}"#, // 43 MiB
            ),
            (
                "seed-volc-pro",
                "seed-volc",
                "doubao-seed-2-1-pro-260628",
                "豆包 Seed 2.1 Pro(全剧 Files API)",
                "file_api",
                r#"{"maxBytes":536870912}"#, // 512 MiB —— Files API 上限,超限才本地压缩
            ),
            (
                "seed-ali-m",
                "seed-ali",
                "qwen3.6-plus",
                "通义千问 3.6 Plus",
                "base64",
                r#"{"maxBytes":14680064}"#, // 14 MiB
            ),
            (
                "seed-yun-m",
                "seed-yun",
                "gemini-3.1-flash-lite",
                "Gemini 3.1 Flash Lite",
                "base64",
                r#"{"maxBytes":8388608}"#, // 8 MiB
            ),
            (
                "seed-xiaomi-m",
                "seed-xiaomi",
                "mimo-v2.5",
                "小米 MiMo 2.5",
                "base64",
                r#"{"maxBytes":36700160}"#, // 35 MiB
            ),
        ];
        // 输出预算顶格设在模型层(资产层不限):正确与完整优先于省 token,
        // reasoning 模型的思考也计入输出预算,预算小会吃光正文。
        for (id, provider_id, model_id, display_name, method, constraints) in models {
            let max_out = if id == "seed-volc-pro" { 32000 } else { 16000 };
            sqlx::query(
                "INSERT INTO models
                 (id, provider_id, model_id, display_name, video_input_method,
                  constraints, params, enabled, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
            )
            .bind(id)
            .bind(provider_id)
            .bind(model_id)
            .bind(display_name)
            .bind(method)
            .bind(constraints)
            .bind(format!(r#"{{"max_tokens":{max_out}}}"#))
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await?;
        }
        log::info!("已写入三家预设供应商与模型(api_key 留空,待用户在设置页填写)");
    }

    sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES (?, '1')")
        .bind(SEED_FLAG)
        .execute(pool)
        .await?;
    Ok(())
}

/// 已下线的内置资产(2026-07 移除 D 阶段二创:素材包交下游自行分析改编方向)。
/// 种子是 INSERT OR IGNORE,不删的话老库里的旧行会继续显示并执行。
const RETIRED_BUILTIN_SPECS: &[&str] = &[
    "d-skeleton",
    "d-guide",
    "d-outline",
    "d-visuals",
    "d-script",
    "d-seedance",
];

/// 内置资产规格种子 —— 每次启动补齐缺失项(INSERT OR IGNORE):
/// 新版本新增的资产老库也能拿到;用户已改过的 prompt 不会被覆盖。
async fn seed_asset_specs(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    for id in RETIRED_BUILTIN_SPECS {
        sqlx::query("DELETE FROM dna_tasks WHERE spec_id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM asset_specs WHERE id = ? AND builtin = 1")
            .bind(id)
            .execute(pool)
            .await?;
    }
    let now = chrono::Utc::now().to_rfc3339();
    for s in crate::prompts::BUILTIN_SPECS {
        sqlx::query(
            "INSERT OR IGNORE INTO asset_specs
             (id, stage, sort_no, name, scope, prompt, merge_prompt, model_id, inputs,
              output_template, needs_video, user_input, enabled, builtin, params, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?, ?, 1, 1, ?, ?, ?)",
        )
        .bind(s.id)
        .bind(s.stage)
        .bind(s.sort_no)
        .bind(s.name)
        .bind(s.scope)
        .bind(s.prompt)
        .bind(s.merge_prompt)
        .bind(s.inputs)
        .bind(s.output_template)
        .bind(s.needs_video)
        .bind(s.user_input)
        .bind(s.params)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 每个测试用独立的临时库,返回 (库路径, 清理用目录)。
    fn temp_db() -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("dramadna-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        (dir.join("dramadna.db"), dir)
    }

    async fn count(pool: &SqlitePool, table: &str) -> i64 {
        let (n,): (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(pool)
            .await
            .unwrap();
        n
    }

    #[tokio::test]
    async fn seeds_default_providers_and_models_on_fresh_db() {
        let (path, dir) = temp_db();

        let db = Db::connect(&path).await.unwrap();
        assert_eq!(count(&db.pool, "providers").await, 4);
        assert_eq!(count(&db.pool, "models").await, 5);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn seeding_is_idempotent_across_restarts() {
        let (path, dir) = temp_db();

        let db = Db::connect(&path).await.unwrap();
        drop(db);
        // 第二次「启动」同一个库 —— 预设不应重复写入。
        let db = Db::connect(&path).await.unwrap();
        assert_eq!(count(&db.pool, "providers").await, 4);
        assert_eq!(count(&db.pool, "models").await, 5);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn skips_seeding_when_providers_already_exist() {
        let (path, dir) = temp_db();

        // 模拟老用户:已有自建供应商、但还没有 seed 标志。
        let db = Db::connect(&path).await.unwrap();
        sqlx::query("DELETE FROM providers")
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM settings WHERE key = 'default_seeded'")
            .execute(&db.pool)
            .await
            .unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO providers (id, name, kind, base_url, created_at, updated_at)
             VALUES ('mine', '我的供应商', 'volcengine', 'https://x', ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&db.pool)
        .await
        .unwrap();
        drop(db);

        // 重新启动 —— 已有供应商,不应叠加 3 个预设。
        let db = Db::connect(&path).await.unwrap();
        assert_eq!(count(&db.pool, "providers").await, 1);

        std::fs::remove_dir_all(&dir).ok();
    }
}
