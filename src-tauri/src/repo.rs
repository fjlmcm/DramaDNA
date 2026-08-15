use chrono::Utc;
use uuid::Uuid;

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::models::{
    BatchJob, JobItem, Model, ModelInput, Provider, ProviderInput, Run, RunInput, Scheme,
    SchemeInput,
};

fn now() -> String {
    Utc::now().to_rfc3339()
}
fn new_id() -> String {
    Uuid::new_v4().to_string()
}

// ────────────────────────────── providers ──────────────────────────────

pub async fn list_providers(db: &Db) -> AppResult<Vec<Provider>> {
    let rows = sqlx::query_as::<_, Provider>("SELECT * FROM providers ORDER BY created_at")
        .fetch_all(&db.pool)
        .await?;
    Ok(rows)
}

pub async fn get_provider(db: &Db, id: &str) -> AppResult<Provider> {
    let row = sqlx::query_as::<_, Provider>("SELECT * FROM providers WHERE id = ?")
        .bind(id)
        .fetch_one(&db.pool)
        .await?;
    Ok(row)
}

pub async fn create_provider(db: &Db, input: ProviderInput) -> AppResult<Provider> {
    let id = new_id();
    let ts = now();
    sqlx::query(
        "INSERT INTO providers (id, name, kind, base_url, api_key, extra_config, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&input.name)
    .bind(&input.kind)
    .bind(&input.base_url)
    .bind(&input.api_key)
    .bind(&input.extra_config)
    .bind(&ts)
    .bind(&ts)
    .execute(&db.pool)
    .await?;
    get_provider(db, &id).await
}

pub async fn update_provider(db: &Db, id: &str, input: ProviderInput) -> AppResult<Provider> {
    let affected = sqlx::query(
        "UPDATE providers
         SET name = ?, kind = ?, base_url = ?, api_key = ?, extra_config = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(&input.name)
    .bind(&input.kind)
    .bind(&input.base_url)
    .bind(&input.api_key)
    .bind(&input.extra_config)
    .bind(now())
    .bind(id)
    .execute(&db.pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AppError::Msg(format!("供应商不存在: {id}")));
    }
    get_provider(db, id).await
}

pub async fn delete_provider(db: &Db, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM providers WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await?;
    Ok(())
}

// ────────────────────────────── models ──────────────────────────────

pub async fn list_models(db: &Db) -> AppResult<Vec<Model>> {
    let rows = sqlx::query_as::<_, Model>("SELECT * FROM models ORDER BY created_at")
        .fetch_all(&db.pool)
        .await?;
    Ok(rows)
}

pub async fn get_model(db: &Db, id: &str) -> AppResult<Model> {
    let row = sqlx::query_as::<_, Model>("SELECT * FROM models WHERE id = ?")
        .bind(id)
        .fetch_one(&db.pool)
        .await?;
    Ok(row)
}

pub async fn create_model(db: &Db, input: ModelInput) -> AppResult<Model> {
    let id = new_id();
    let ts = now();
    sqlx::query(
        "INSERT INTO models
         (id, provider_id, model_id, display_name, video_input_method, constraints, params, enabled, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&input.provider_id)
    .bind(&input.model_id)
    .bind(&input.display_name)
    .bind(&input.video_input_method)
    .bind(&input.constraints)
    .bind(&input.params)
    .bind(input.enabled)
    .bind(&ts)
    .bind(&ts)
    .execute(&db.pool)
    .await?;
    get_model(db, &id).await
}

pub async fn update_model(db: &Db, id: &str, input: ModelInput) -> AppResult<Model> {
    let affected = sqlx::query(
        "UPDATE models
         SET provider_id = ?, model_id = ?, display_name = ?, video_input_method = ?,
             constraints = ?, params = ?, enabled = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(&input.provider_id)
    .bind(&input.model_id)
    .bind(&input.display_name)
    .bind(&input.video_input_method)
    .bind(&input.constraints)
    .bind(&input.params)
    .bind(input.enabled)
    .bind(now())
    .bind(id)
    .execute(&db.pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AppError::Msg(format!("模型不存在: {id}")));
    }
    get_model(db, id).await
}

pub async fn delete_model(db: &Db, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM models WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await?;
    Ok(())
}

// ────────────────────────────── schemes ──────────────────────────────

pub async fn list_schemes(db: &Db) -> AppResult<Vec<Scheme>> {
    let rows = sqlx::query_as::<_, Scheme>("SELECT * FROM schemes ORDER BY created_at DESC")
        .fetch_all(&db.pool)
        .await?;
    Ok(rows)
}

pub async fn get_scheme(db: &Db, id: &str) -> AppResult<Scheme> {
    let row = sqlx::query_as::<_, Scheme>("SELECT * FROM schemes WHERE id = ?")
        .bind(id)
        .fetch_one(&db.pool)
        .await?;
    Ok(row)
}

pub async fn create_scheme(db: &Db, input: SchemeInput) -> AppResult<Scheme> {
    let id = new_id();
    let ts = now();
    sqlx::query(
        "INSERT INTO schemes (id, name, model_id, prompt, params, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&input.name)
    .bind(&input.model_id)
    .bind(&input.prompt)
    .bind(&input.params)
    .bind(&ts)
    .bind(&ts)
    .execute(&db.pool)
    .await?;
    get_scheme(db, &id).await
}

pub async fn update_scheme(db: &Db, id: &str, input: SchemeInput) -> AppResult<Scheme> {
    let affected = sqlx::query(
        "UPDATE schemes SET name = ?, model_id = ?, prompt = ?, params = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(&input.name)
    .bind(&input.model_id)
    .bind(&input.prompt)
    .bind(&input.params)
    .bind(now())
    .bind(id)
    .execute(&db.pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AppError::Msg(format!("方案不存在: {id}")));
    }
    get_scheme(db, id).await
}

pub async fn delete_scheme(db: &Db, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM schemes WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await?;
    Ok(())
}

// ────────────────────────────── batch jobs ──────────────────────────────

pub async fn create_batch_job(
    db: &Db,
    name: &str,
    scheme_id: &str,
    file_paths: &[String],
) -> AppResult<BatchJob> {
    let job_id = new_id();
    let ts = now();
    sqlx::query(
        "INSERT INTO batch_jobs
         (id, name, scheme_id, status, total_items, done_items, created_at, updated_at)
         VALUES (?, ?, ?, 'pending', ?, 0, ?, ?)",
    )
    .bind(&job_id)
    .bind(name)
    .bind(scheme_id)
    .bind(file_paths.len() as i64)
    .bind(&ts)
    .bind(&ts)
    .execute(&db.pool)
    .await?;

    for path in file_paths {
        sqlx::query(
            "INSERT INTO job_items (id, job_id, file_path, status, attempts, created_at, updated_at)
             VALUES (?, ?, ?, 'pending', 0, ?, ?)",
        )
        .bind(new_id())
        .bind(&job_id)
        .bind(path)
        .bind(&ts)
        .bind(&ts)
        .execute(&db.pool)
        .await?;
    }
    get_batch_job(db, &job_id).await
}

pub async fn list_batch_jobs(db: &Db) -> AppResult<Vec<BatchJob>> {
    let rows = sqlx::query_as::<_, BatchJob>("SELECT * FROM batch_jobs ORDER BY created_at DESC")
        .fetch_all(&db.pool)
        .await?;
    Ok(rows)
}

/// 删除批量任务 —— job_items 由外键 ON DELETE CASCADE 自动级联删除。
/// 运行中的任务先要求取消,避免与 worker 并发冲突导致脏写。
pub async fn delete_batch_job(db: &Db, id: &str) -> AppResult<()> {
    if let Ok(s) = job_status(db, id).await {
        if s == "running" {
            return Err(AppError::Msg("运行中的任务不能删除,请先取消".into()));
        }
    }
    sqlx::query("DELETE FROM batch_jobs WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await?;
    Ok(())
}

pub async fn get_batch_job(db: &Db, id: &str) -> AppResult<BatchJob> {
    let row = sqlx::query_as::<_, BatchJob>("SELECT * FROM batch_jobs WHERE id = ?")
        .bind(id)
        .fetch_one(&db.pool)
        .await?;
    Ok(row)
}

pub async fn job_status(db: &Db, id: &str) -> AppResult<String> {
    let row: (String,) = sqlx::query_as("SELECT status FROM batch_jobs WHERE id = ?")
        .bind(id)
        .fetch_one(&db.pool)
        .await?;
    Ok(row.0)
}

pub async fn set_job_status(db: &Db, id: &str, status: &str) -> AppResult<()> {
    // 所有终态(done / cancelled / failed)都要记 finished_at,前端可据此显示完成时刻。
    let finished = if matches!(status, "done" | "cancelled" | "failed") {
        Some(now())
    } else {
        None
    };
    sqlx::query("UPDATE batch_jobs SET status = ?, updated_at = ?, finished_at = ? WHERE id = ?")
        .bind(status)
        .bind(now())
        .bind(finished)
        .bind(id)
        .execute(&db.pool)
        .await?;
    Ok(())
}

/// 单个任务下各状态单元数 —— 用于 batch worker 收尾判定整体结果(全失败 vs 部分成功)。
#[derive(Debug, Default, Clone, Copy)]
pub struct JobItemCounts {
    pub pending: i64,
    pub processing: i64,
    pub done: i64,
    pub failed: i64,
}

pub async fn job_item_counts(db: &Db, job_id: &str) -> AppResult<JobItemCounts> {
    let rows: Vec<(String, i64)> =
        sqlx::query_as("SELECT status, COUNT(*) FROM job_items WHERE job_id = ? GROUP BY status")
            .bind(job_id)
            .fetch_all(&db.pool)
            .await?;
    let mut c = JobItemCounts::default();
    for (s, n) in rows {
        match s.as_str() {
            "pending" => c.pending = n,
            "processing" => c.processing = n,
            "done" => c.done = n,
            "failed" => c.failed = n,
            _ => {}
        }
    }
    Ok(c)
}

/// 重置某个 job 下停在 processing 的单元为 pending,以便 worker 再次扫到。
/// 每次 run_job 启动时调用,以及 cancel 时(配合下次「继续」)。
pub async fn reset_processing_items_of(db: &Db, job_id: &str) -> AppResult<u64> {
    let r = sqlx::query(
        "UPDATE job_items SET status = 'pending', updated_at = ?
         WHERE job_id = ? AND status = 'processing'",
    )
    .bind(now())
    .bind(job_id)
    .execute(&db.pool)
    .await?;
    Ok(r.rows_affected())
}

/// 统计引用某 scheme 的 batch_jobs(用于删除前检查,避免外键悬空)。
pub async fn count_batch_jobs_by_scheme(db: &Db, scheme_id: &str) -> AppResult<i64> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM batch_jobs WHERE scheme_id = ?")
        .bind(scheme_id)
        .fetch_one(&db.pool)
        .await?;
    Ok(n)
}

/// 统计经 schemes 间接引用某 model 的 batch_jobs。
pub async fn count_batch_jobs_by_model(db: &Db, model_id: &str) -> AppResult<i64> {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM batch_jobs bj
         JOIN schemes s ON s.id = bj.scheme_id
         WHERE s.model_id = ?",
    )
    .bind(model_id)
    .fetch_one(&db.pool)
    .await?;
    Ok(n)
}

/// 统计经 models → schemes 间接引用某 provider 的 batch_jobs。
pub async fn count_batch_jobs_by_provider(db: &Db, provider_id: &str) -> AppResult<i64> {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM batch_jobs bj
         JOIN schemes s ON s.id = bj.scheme_id
         JOIN models m ON m.id = s.model_id
         WHERE m.provider_id = ?",
    )
    .bind(provider_id)
    .fetch_one(&db.pool)
    .await?;
    Ok(n)
}

/// 重算 done_items = 已结束(done/failed)单元数。
pub async fn recompute_job_done(db: &Db, job_id: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE batch_jobs SET done_items =
           (SELECT COUNT(*) FROM job_items WHERE job_id = ? AND status IN ('done','failed')),
           updated_at = ?
         WHERE id = ?",
    )
    .bind(job_id)
    .bind(now())
    .bind(job_id)
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn list_job_items(db: &Db, job_id: &str) -> AppResult<Vec<JobItem>> {
    let rows = sqlx::query_as::<_, JobItem>(
        "SELECT * FROM job_items WHERE job_id = ? ORDER BY created_at",
    )
    .bind(job_id)
    .fetch_all(&db.pool)
    .await?;
    Ok(rows)
}

pub async fn pending_items(db: &Db, job_id: &str) -> AppResult<Vec<JobItem>> {
    let rows = sqlx::query_as::<_, JobItem>(
        "SELECT * FROM job_items WHERE job_id = ? AND status = 'pending' ORDER BY created_at",
    )
    .bind(job_id)
    .fetch_all(&db.pool)
    .await?;
    Ok(rows)
}

pub async fn set_item_status(db: &Db, id: &str, status: &str) -> AppResult<()> {
    sqlx::query("UPDATE job_items SET status = ?, updated_at = ? WHERE id = ?")
        .bind(status)
        .bind(now())
        .bind(id)
        .execute(&db.pool)
        .await?;
    Ok(())
}

pub async fn set_item_done(db: &Db, id: &str, result: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE job_items SET status = 'done', result_text = ?, error = NULL,
           attempts = attempts + 1, updated_at = ? WHERE id = ?",
    )
    .bind(result)
    .bind(now())
    .bind(id)
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn set_item_failed(db: &Db, id: &str, error: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE job_items SET status = 'failed', error = ?,
           attempts = attempts + 1, updated_at = ? WHERE id = ?",
    )
    .bind(error)
    .bind(now())
    .bind(id)
    .execute(&db.pool)
    .await?;
    Ok(())
}

/// 启动恢复:把中断时停在 processing 的单元重置为 pending,
/// running 的任务标回 pending(等待用户「继续」)。返回重置的单元数。
pub async fn reset_processing_items(db: &Db) -> AppResult<u64> {
    let affected = sqlx::query(
        "UPDATE job_items SET status = 'pending', updated_at = ? WHERE status = 'processing'",
    )
    .bind(now())
    .execute(&db.pool)
    .await?
    .rows_affected();
    sqlx::query(
        "UPDATE batch_jobs SET status = 'pending', updated_at = ? WHERE status = 'running'",
    )
    .bind(now())
    .execute(&db.pool)
    .await?;
    Ok(affected)
}

// ────────────────────────────── runs(执行日志) ──────────────────────────────

pub async fn create_run(db: &Db, input: &RunInput) -> AppResult<()> {
    let ts = now();
    sqlx::query(
        "INSERT INTO runs
         (id, scheme_id, scheme_name, model_label, file_path, prompt, status,
          result_text, error, duration_ms, created_at, updated_at)
         VALUES (?, NULL, '', ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(new_id())
    .bind(&input.model_label)
    .bind(&input.file_path)
    .bind(&input.prompt)
    .bind(&input.status)
    .bind(&input.result_text)
    .bind(&input.error)
    .bind(input.duration_ms)
    .bind(&ts)
    .bind(&ts)
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn list_runs(db: &Db, limit: i64) -> AppResult<Vec<Run>> {
    let rows = sqlx::query_as::<_, Run>("SELECT * FROM runs ORDER BY created_at DESC LIMIT ?")
        .bind(limit)
        .fetch_all(&db.pool)
        .await?;
    Ok(rows)
}

// ────────────────────────────── settings ──────────────────────────────

pub async fn get_setting(db: &Db, key: &str) -> AppResult<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(&db.pool)
        .await?;
    Ok(row.map(|r| r.0))
}

pub async fn set_setting(db: &Db, key: &str, value: &str) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(&db.pool)
    .await?;
    Ok(())
}

/// 清空模型测试运行记录。
pub async fn clear_runs(db: &Db) -> AppResult<()> {
    sqlx::query("DELETE FROM runs").execute(&db.pool).await?;
    Ok(())
}

// ────────────────────────────── tests ──────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用空库 —— 只跑 migration,**不跑 seed_defaults**(避免预设数据干扰断言)。
    async fn test_db() -> Db {
        let path = std::env::temp_dir().join(format!("dramadna-test-{}.db", Uuid::new_v4()));
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = sqlx::SqlitePool::connect_with(options)
            .await
            .expect("连接测试库失败");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("跑 migration 失败");
        Db { pool }
    }

    fn provider_input(name: &str) -> ProviderInput {
        ProviderInput {
            name: name.to_string(),
            kind: "volcengine".to_string(),
            base_url: "https://ark.example.com/api/v3".to_string(),
            api_key: "test-key".to_string(),
            extra_config: "{}".to_string(),
        }
    }

    #[tokio::test]
    async fn migration_and_provider_crud() {
        let db = test_db().await;

        let created = create_provider(&db, provider_input("火山引擎"))
            .await
            .unwrap();
        assert_eq!(created.name, "火山引擎");
        assert_eq!(list_providers(&db).await.unwrap().len(), 1);

        let mut next = provider_input("火山改名");
        next.api_key = "key-2".to_string();
        let updated = update_provider(&db, &created.id, next).await.unwrap();
        assert_eq!(updated.name, "火山改名");
        assert_eq!(updated.api_key, "key-2");

        delete_provider(&db, &created.id).await.unwrap();
        assert!(list_providers(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn deleting_provider_cascades_to_models() {
        let db = test_db().await;
        let provider = create_provider(&db, provider_input("阿里百炼"))
            .await
            .unwrap();

        create_model(
            &db,
            ModelInput {
                provider_id: provider.id.clone(),
                model_id: "qwen3.6-plus".to_string(),
                display_name: "Qwen".to_string(),
                video_input_method: "file_api".to_string(),
                constraints: "{}".to_string(),
                params: "{}".to_string(),
                enabled: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(list_models(&db).await.unwrap().len(), 1);

        // 外键级联:删供应商应连带删除其模型。
        delete_provider(&db, &provider.id).await.unwrap();
        assert!(list_models(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn scheme_crud() {
        let db = test_db().await;
        let provider = create_provider(&db, provider_input("p")).await.unwrap();
        let model = create_model(
            &db,
            ModelInput {
                provider_id: provider.id.clone(),
                model_id: "m".into(),
                display_name: "M".into(),
                video_input_method: "base64".into(),
                constraints: "{}".into(),
                params: "{}".into(),
                enabled: true,
            },
        )
        .await
        .unwrap();

        let scheme = create_scheme(
            &db,
            SchemeInput {
                name: "方案一".into(),
                model_id: model.id.clone(),
                prompt: "描述视频".into(),
                params: "{}".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(scheme.name, "方案一");
        assert_eq!(list_schemes(&db).await.unwrap().len(), 1);

        let updated = update_scheme(
            &db,
            &scheme.id,
            SchemeInput {
                name: "方案改名".into(),
                model_id: model.id.clone(),
                prompt: "新的提示词".into(),
                params: "{}".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(updated.name, "方案改名");
        assert_eq!(updated.prompt, "新的提示词");

        delete_scheme(&db, &scheme.id).await.unwrap();
        assert!(list_schemes(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn batch_job_lifecycle_and_recovery() {
        let db = test_db().await;
        let provider = create_provider(&db, provider_input("p")).await.unwrap();
        let model = create_model(
            &db,
            ModelInput {
                provider_id: provider.id.clone(),
                model_id: "m".into(),
                display_name: "M".into(),
                video_input_method: "base64".into(),
                constraints: "{}".into(),
                params: "{}".into(),
                enabled: true,
            },
        )
        .await
        .unwrap();
        let scheme = create_scheme(
            &db,
            SchemeInput {
                name: "s".into(),
                model_id: model.id.clone(),
                prompt: "p".into(),
                params: "{}".into(),
            },
        )
        .await
        .unwrap();

        let job = create_batch_job(
            &db,
            "批量一",
            &scheme.id,
            &["/a.mp4".into(), "/b.mp4".into()],
        )
        .await
        .unwrap();
        assert_eq!(job.total_items, 2);
        assert_eq!(job.done_items, 0);

        let items = list_job_items(&db, &job.id).await.unwrap();
        assert_eq!(items.len(), 2);

        // 一个完成、一个停在 processing(模拟中断)。
        set_item_done(&db, &items[0].id, "结果A").await.unwrap();
        set_item_status(&db, &items[1].id, "processing")
            .await
            .unwrap();
        recompute_job_done(&db, &job.id).await.unwrap();
        assert_eq!(get_batch_job(&db, &job.id).await.unwrap().done_items, 1);

        // 中断恢复:processing 单元重置为 pending,running 任务回 pending。
        set_job_status(&db, &job.id, "running").await.unwrap();
        let reset = reset_processing_items(&db).await.unwrap();
        assert_eq!(reset, 1);
        assert_eq!(pending_items(&db, &job.id).await.unwrap().len(), 1);
        assert_eq!(job_status(&db, &job.id).await.unwrap(), "pending");
    }
}
