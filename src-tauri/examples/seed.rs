// 开发用 seed 脚本 —— 向 dramadna 数据库写入三家供应商与模型骨架。
//
// 运行: cargo run --manifest-path src-tauri/Cargo.toml --example seed
//
// 放在 examples/ 而非 src/bin/ —— 避免被 tauri build 打包进发布包。
// 自包含:连接(或创建)应用数据库、跑 migration、幂等写入。
// 不写入 api_key —— 用 ON CONFLICT DO UPDATE 保护该字段,API key 完全由
// 用户在应用「设置」页管理,重复运行本脚本不会覆盖。

use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::SqlitePool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 与 Tauri app_data_dir(macOS / identifier=com.dramadna.app)一致。
    let home = std::env::var("HOME")?;
    let db_path = format!("{home}/Library/Application Support/com.dramadna.app/dramadna.db");
    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let options = SqliteConnectOptions::from_str(&format!("sqlite://{db_path}"))?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePool::connect_with(options).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    let now = chrono::Utc::now().to_rfc3339();

    // (id, 名称, kind, base_url) —— 不含 api_key。
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
    ];
    for (id, name, kind, base_url) in providers {
        // 首次插入 api_key 为空;冲突时只更新结构字段,不碰 api_key。
        sqlx::query(
            "INSERT INTO providers
             (id, name, kind, base_url, api_key, extra_config, created_at, updated_at)
             VALUES (?, ?, ?, ?, '', '{}', ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               name = excluded.name,
               kind = excluded.kind,
               base_url = excluded.base_url,
               updated_at = excluded.updated_at",
        )
        .bind(id)
        .bind(name)
        .bind(kind)
        .bind(base_url)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await?;
    }

    // (id, provider_id, model_id, 显示名)
    let models = [
        (
            "seed-volc-m",
            "seed-volc",
            "doubao-seed-2-0-lite-260428",
            "豆包 Seed 2.0 Lite",
        ),
        (
            "seed-ali-m",
            "seed-ali",
            "qwen3.6-plus",
            "通义千问 3.6 Plus",
        ),
        (
            "seed-yun-m",
            "seed-yun",
            "gemini-3.1-flash-lite",
            "Gemini 3.1 Flash Lite",
        ),
    ];
    for (id, provider_id, model_id, display_name) in models {
        // 冲突时只更新结构字段,保留用户对 enabled / constraints / params 的调整。
        sqlx::query(
            "INSERT INTO models
             (id, provider_id, model_id, display_name, video_input_method,
              constraints, params, enabled, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'base64', '{}', '{}', 1, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               provider_id = excluded.provider_id,
               model_id = excluded.model_id,
               display_name = excluded.display_name,
               updated_at = excluded.updated_at",
        )
        .bind(id)
        .bind(provider_id)
        .bind(model_id)
        .bind(display_name)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await?;
    }

    pool.close().await;
    println!("✓ seed 完成 → {db_path}");
    println!("  3 个供应商 + 3 个模型骨架已就绪");
    println!("  API key 未写入 —— 请在应用「设置 → 模型供应商」中为各供应商填写");
    Ok(())
}
