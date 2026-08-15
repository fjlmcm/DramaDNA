// 批量处理 —— 有界并发 worker,逐单元预处理 + 视频理解,支持中断恢复。
//
// 一个 batch_job 含 N 个 job_item(每个 = 一个视频文件)。run_job 用信号量
// 限制并发,逐单元处理。中断后单元停在 processing,启动时由
// repo::reset_processing_items 重置为 pending,用户「继续」即重跑。

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tokio::sync::Semaphore;

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::models::JobItem;
use crate::{ffmpeg, provider, repo};

/// 批量并发数默认值;实际值从 settings 表读取(键 batch_concurrency)。
const DEFAULT_CONCURRENCY: usize = 10;

/// 单元请求失败后的最大重试次数(首次失败后额外尝试;合计最多 1 + MAX_RETRIES 次请求)。
const MAX_RETRIES: usize = 3;
/// 每次重试前的固定等待间隔。
const RETRY_DELAY: Duration = Duration::from_secs(3);

/// 处理一个批量任务的所有 pending 单元(有界并发)。
pub async fn run_job(app: AppHandle, db: Db, job_id: String) -> AppResult<()> {
    // 启动前先把本 job 残留的 processing 单元重置为 pending —— 覆盖三种场景:
    // 上次中断、上次取消、首次新建(空操作)。让「继续/重新运行」都从一致状态开始。
    let _ = repo::reset_processing_items_of(&db, &job_id).await;
    repo::set_job_status(&db, &job_id, "running").await?;
    let items = repo::pending_items(&db, &job_id).await?;
    // 并发数从设置读取(完成一个即补入下一个,始终维持该并发)。
    let concurrency = repo::get_setting(&db, "batch_concurrency")
        .await
        .ok()
        .flatten()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(DEFAULT_CONCURRENCY)
        .max(1);
    log::info!(
        "批量任务开始: job={job_id},待处理 {} 个单元,并发 {concurrency}",
        items.len()
    );
    let sem = Arc::new(Semaphore::new(concurrency));

    let mut handles = Vec::new();
    for item in items {
        let permit = sem
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| AppError::Msg(format!("并发信号量错误: {e}")))?;
        let app = app.clone();
        let db = db.clone();
        let job_id = job_id.clone();
        handles.push(tokio::spawn(async move {
            let _permit = permit;
            // 任务被取消则跳过剩余单元。
            if let Ok(status) = repo::job_status(&db, &job_id).await {
                if status == "cancelled" {
                    return;
                }
            }
            process_item(&app, &db, &item).await;
        }));
    }
    for h in handles {
        let _ = h.await;
    }

    // 收尾:取消的保持 cancelled;否则按 item 状态分布决定 done / failed / pending。
    // 注意 done_items 字段包含 done + failed —— 进度条满不代表全成功,要看实际计数。
    if let Ok(status) = repo::job_status(&db, &job_id).await {
        if status != "cancelled" {
            let c = repo::job_item_counts(&db, &job_id).await?;
            let final_status = if c.pending == 0 && c.processing == 0 {
                if c.done == 0 && c.failed > 0 {
                    "failed" // 全失败 —— 没有一个单元成功
                } else {
                    "done"
                }
            } else {
                "pending" // 还有待处理,等用户「继续」
            };
            repo::set_job_status(&db, &job_id, final_status).await?;
        }
    }
    Ok(())
}

/// 处理单个单元:预处理 → 视频理解 → 落库结果。
/// 请求失败自动重试,最多 MAX_RETRIES 次;全部失败才标记跳过(failed)。
async fn process_item(app: &AppHandle, db: &Db, item: &JobItem) {
    let _ = repo::set_item_status(db, &item.id, "processing").await;
    log::debug!("批量单元开始: {}", item.file_path);

    let mut last_err = String::new();
    for attempt in 0..=MAX_RETRIES {
        match understand(app, db, item).await {
            Ok(text) => {
                let _ = repo::set_item_done(db, &item.id, &text).await;
                log::info!("批量单元完成: {}", item.file_path);
                let _ = repo::recompute_job_done(db, &item.job_id).await;
                return;
            }
            Err(e) => {
                last_err = e.to_string();
                if attempt < MAX_RETRIES {
                    // 任务已取消则中止重试,单元留待中断恢复重跑。
                    if matches!(
                        repo::job_status(db, &item.job_id).await.as_deref(),
                        Ok("cancelled")
                    ) {
                        log::info!("任务已取消,中止重试: {}", item.file_path);
                        return;
                    }
                    log::warn!(
                        "批量单元失败(第 {}/{} 次尝试),{}s 后重试: {} — {e}",
                        attempt + 1,
                        MAX_RETRIES + 1,
                        RETRY_DELAY.as_secs(),
                        item.file_path
                    );
                    tokio::time::sleep(RETRY_DELAY).await;
                }
            }
        }
    }

    let _ = repo::set_item_failed(db, &item.id, &last_err).await;
    log::warn!(
        "批量单元已重试 {} 次仍失败,跳过: {} — {last_err}",
        MAX_RETRIES,
        item.file_path
    );
    let _ = repo::recompute_job_done(db, &item.job_id).await;
}

async fn understand(app: &AppHandle, db: &Db, item: &JobItem) -> AppResult<String> {
    let job = repo::get_batch_job(db, &item.job_id).await?;
    let scheme = repo::get_scheme(db, &job.scheme_id).await?;
    let model = repo::get_model(db, &scheme.model_id).await?;
    let provider_row = repo::get_provider(db, &model.provider_id).await?;

    let constraints = ffmpeg::VideoConstraints::from_json_with(
        &model.constraints,
        provider::video_constraint_defaults(&provider_row.kind),
    );
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| AppError::Msg(format!("获取缓存目录失败: {e}")))?
        .join("preprocessed");
    let ready = ffmpeg::ensure_compliant(&item.file_path, &constraints, &cache_dir).await?;

    provider::understand_video(&provider_row, &model, &scheme.prompt, &ready).await
}
