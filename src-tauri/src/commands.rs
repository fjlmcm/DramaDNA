use std::path::PathBuf;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::ffmpeg;
use crate::models::{
    BatchJob, JobItem, Model, ModelInput, Provider, ProviderInput, Run, RunInput, Scheme,
    SchemeInput,
};
use crate::provider;
use crate::repo;

// ────────────────────────────── providers ──────────────────────────────

#[tauri::command]
pub async fn list_providers(db: State<'_, Db>) -> AppResult<Vec<Provider>> {
    repo::list_providers(db.inner()).await
}

#[tauri::command]
pub async fn create_provider(db: State<'_, Db>, input: ProviderInput) -> AppResult<Provider> {
    repo::create_provider(db.inner(), input).await
}

#[tauri::command]
pub async fn update_provider(
    db: State<'_, Db>,
    id: String,
    input: ProviderInput,
) -> AppResult<Provider> {
    repo::update_provider(db.inner(), &id, input).await
}

#[tauri::command]
pub async fn delete_provider(db: State<'_, Db>, id: String) -> AppResult<()> {
    // 防止级联(provider → models → schemes)后 batch_jobs.scheme_id 悬空。
    let n = repo::count_batch_jobs_by_provider(db.inner(), &id).await?;
    if n > 0 {
        return Err(AppError::Msg(format!(
            "此供应商下的方案被 {n} 个批量任务引用,请先删除这些任务"
        )));
    }
    repo::delete_provider(db.inner(), &id).await
}

// ────────────────────────────── models ──────────────────────────────

#[tauri::command]
pub async fn list_models(db: State<'_, Db>) -> AppResult<Vec<Model>> {
    repo::list_models(db.inner()).await
}

#[tauri::command]
pub async fn create_model(db: State<'_, Db>, input: ModelInput) -> AppResult<Model> {
    repo::create_model(db.inner(), input).await
}

#[tauri::command]
pub async fn update_model(db: State<'_, Db>, id: String, input: ModelInput) -> AppResult<Model> {
    repo::update_model(db.inner(), &id, input).await
}

#[tauri::command]
pub async fn delete_model(db: State<'_, Db>, id: String) -> AppResult<()> {
    let n = repo::count_batch_jobs_by_model(db.inner(), &id).await?;
    if n > 0 {
        return Err(AppError::Msg(format!(
            "此模型下的方案被 {n} 个批量任务引用,请先删除这些任务"
        )));
    }
    repo::delete_model(db.inner(), &id).await
}

// ────────────────────────────── 视频理解 ──────────────────────────────

#[tauri::command]
pub async fn understand_video(
    app: AppHandle,
    db: State<'_, Db>,
    model_id: String,
    prompt: String,
    video_path: String,
) -> AppResult<String> {
    let model = repo::get_model(db.inner(), &model_id).await?;
    let provider_row = repo::get_provider(db.inner(), &model.provider_id).await?;
    let ready = preprocess(&app, &provider_row.kind, &model, &video_path).await?;
    provider::understand_video(&provider_row, &model, &prompt, &ready).await
}

#[tauri::command]
pub async fn understand_video_stream(
    app: AppHandle,
    db: State<'_, Db>,
    model_id: String,
    prompt: String,
    video_path: String,
    run_id: String,
    on_event: Channel<provider::StreamEvent>,
) -> AppResult<()> {
    let result = run_stream(
        &app,
        &db,
        &model_id,
        &prompt,
        &video_path,
        &run_id,
        &on_event,
    )
    .await;
    if let Err(e) = &result {
        let _ = on_event.send(provider::StreamEvent::Error {
            message: e.to_string(),
        });
    }
    result
}

/// 前端发起的中途取消 —— 仅发信号,实际状态切换由 stream 函数返回 Err 后自然落库。
#[tauri::command]
pub fn cancel_understand_video(run_id: String) -> bool {
    provider::cancel_run(&run_id)
}

async fn run_stream(
    app: &AppHandle,
    db: &Db,
    model_id: &str,
    prompt: &str,
    video_path: &str,
    run_id: &str,
    on_event: &Channel<provider::StreamEvent>,
) -> AppResult<()> {
    let model = repo::get_model(db, model_id).await?;
    let provider_row = repo::get_provider(db, &model.provider_id).await?;
    let ready = preprocess(app, &provider_row.kind, &model, video_path).await?;

    let started = std::time::Instant::now();
    let result =
        provider::understand_video_stream(&provider_row, &model, prompt, &ready, run_id, on_event)
            .await;
    let elapsed = started.elapsed().as_millis() as i64;

    // 写入执行日志(runs),成功 / 取消 / 失败 三种终态分别记录。
    let run = match &result {
        Ok(text) => RunInput {
            model_label: model.display_name.clone(),
            file_path: video_path.to_string(),
            prompt: prompt.to_string(),
            status: "done".into(),
            result_text: Some(text.clone()),
            error: None,
            duration_ms: Some(elapsed),
        },
        Err(AppError::Cancelled) => RunInput {
            model_label: model.display_name.clone(),
            file_path: video_path.to_string(),
            prompt: prompt.to_string(),
            status: "cancelled".into(),
            result_text: None,
            error: Some("已取消".into()),
            duration_ms: Some(elapsed),
        },
        Err(e) => RunInput {
            model_label: model.display_name.clone(),
            file_path: video_path.to_string(),
            prompt: prompt.to_string(),
            status: "failed".into(),
            result_text: None,
            error: Some(e.to_string()),
            duration_ms: Some(elapsed),
        },
    };
    if let Err(e) = repo::create_run(db, &run).await {
        log::warn!("执行日志写入失败: {e}");
    }

    result.map(|_| ())
}

/// 视频预处理:不符合模型约束则用 ffmpeg 本地转码。
async fn preprocess(
    app: &AppHandle,
    provider_kind: &str,
    model: &Model,
    video_path: &str,
) -> AppResult<String> {
    let constraints = ffmpeg::VideoConstraints::from_json_with(
        &model.constraints,
        provider::video_constraint_defaults(provider_kind),
    );
    let cache_dir = cache_root(app)?;
    ffmpeg::ensure_compliant(video_path, &constraints, &cache_dir).await
}

/// 预处理缓存目录:`<app_cache_dir>/preprocessed`。
fn cache_root(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(app
        .path()
        .app_cache_dir()
        .map_err(|e| AppError::Msg(format!("获取缓存目录失败: {e}")))?
        .join("preprocessed"))
}

// ────────────────────────────── 缓存管理 ──────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStats {
    pub total_bytes: u64,
    pub file_count: u32,
    pub path: String,
}

#[tauri::command]
pub fn cache_stats(app: AppHandle) -> AppResult<CacheStats> {
    let dir = cache_root(&app)?;
    let mut total_bytes = 0u64;
    let mut file_count = 0u32;
    if dir.exists() {
        for entry in std::fs::read_dir(&dir)
            .map_err(|e| AppError::Msg(format!("读取缓存目录失败: {e}")))?
            .flatten()
        {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    total_bytes += meta.len();
                    file_count += 1;
                }
            }
        }
    }
    Ok(CacheStats {
        total_bytes,
        file_count,
        path: dir.to_string_lossy().into_owned(),
    })
}

/// 清空预处理缓存目录(只删文件,保留目录本身)。返回删除的总字节数。
#[tauri::command]
pub fn clear_cache(app: AppHandle) -> AppResult<u64> {
    let dir = cache_root(&app)?;
    if !dir.exists() {
        return Ok(0);
    }
    let mut removed_bytes = 0u64;
    for entry in std::fs::read_dir(&dir)
        .map_err(|e| AppError::Msg(format!("读取缓存目录失败: {e}")))?
        .flatten()
    {
        let path = entry.path();
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                let len = meta.len();
                if std::fs::remove_file(&path).is_ok() {
                    removed_bytes += len;
                }
            }
        }
    }
    Ok(removed_bytes)
}

// ────────────────────────────── schemes ──────────────────────────────

#[tauri::command]
pub async fn list_schemes(db: State<'_, Db>) -> AppResult<Vec<Scheme>> {
    repo::list_schemes(db.inner()).await
}

#[tauri::command]
pub async fn create_scheme(db: State<'_, Db>, input: SchemeInput) -> AppResult<Scheme> {
    repo::create_scheme(db.inner(), input).await
}

#[tauri::command]
pub async fn update_scheme(db: State<'_, Db>, id: String, input: SchemeInput) -> AppResult<Scheme> {
    repo::update_scheme(db.inner(), &id, input).await
}

#[tauri::command]
pub async fn delete_scheme(db: State<'_, Db>, id: String) -> AppResult<()> {
    let n = repo::count_batch_jobs_by_scheme(db.inner(), &id).await?;
    if n > 0 {
        return Err(AppError::Msg(format!(
            "此方案被 {n} 个批量任务引用,请先删除这些任务"
        )));
    }
    repo::delete_scheme(db.inner(), &id).await
}

// ────────────────────────────── 批量处理 ──────────────────────────────

#[tauri::command]
pub async fn create_batch_job(
    db: State<'_, Db>,
    name: String,
    scheme_id: String,
    file_paths: Vec<String>,
) -> AppResult<BatchJob> {
    repo::create_batch_job(db.inner(), &name, &scheme_id, &file_paths).await
}

#[tauri::command]
pub async fn run_batch_job(app: AppHandle, db: State<'_, Db>, job_id: String) -> AppResult<()> {
    // 后台异步处理,command 立即返回;前端轮询进度。
    let db = db.inner().clone();
    tokio::spawn(async move {
        let _ = crate::batch::run_job(app, db, job_id).await;
    });
    Ok(())
}

#[tauri::command]
pub async fn list_batch_jobs(db: State<'_, Db>) -> AppResult<Vec<BatchJob>> {
    repo::list_batch_jobs(db.inner()).await
}

#[tauri::command]
pub async fn list_job_items(db: State<'_, Db>, job_id: String) -> AppResult<Vec<JobItem>> {
    repo::list_job_items(db.inner(), &job_id).await
}

#[tauri::command]
pub async fn cancel_batch_job(db: State<'_, Db>, job_id: String) -> AppResult<()> {
    repo::set_job_status(db.inner(), &job_id, "cancelled").await
}

#[tauri::command]
pub async fn delete_batch_job(db: State<'_, Db>, job_id: String) -> AppResult<()> {
    repo::delete_batch_job(db.inner(), &job_id).await
}

#[tauri::command]
pub async fn export_job_results(
    db: State<'_, Db>,
    job_id: String,
    out_path: String,
) -> AppResult<()> {
    let items = repo::list_job_items(db.inner(), &job_id).await?;
    let export: Vec<_> = items
        .iter()
        .map(|i| {
            serde_json::json!({
                "file": i.file_path,
                "status": i.status,
                "result": i.result_text,
                "error": i.error,
            })
        })
        .collect();
    let json = serde_json::to_string_pretty(&export)
        .map_err(|e| AppError::Msg(format!("结果序列化失败: {e}")))?;
    std::fs::write(&out_path, json).map_err(|e| AppError::Msg(format!("写入导出文件失败: {e}")))?;
    Ok(())
}

// ────────────────────────────── 执行日志 ──────────────────────────────

#[tauri::command]
pub async fn list_runs(db: State<'_, Db>) -> AppResult<Vec<Run>> {
    repo::list_runs(db.inner(), 200).await
}

/// 读取调试日志文件(尾部最多 2000 行)。
#[tauri::command]
pub async fn read_debug_log(app: AppHandle) -> AppResult<String> {
    let path = app
        .path()
        .app_log_dir()
        .map_err(|e| AppError::Msg(format!("获取日志目录失败: {e}")))?
        .join("dramadna.log");
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(2000);
    Ok(lines[start..].join("\n"))
}

/// 调试日志文件路径(界面展示,便于定位排查)。
#[tauri::command]
pub async fn debug_log_path(app: AppHandle) -> AppResult<String> {
    Ok(app
        .path()
        .app_log_dir()
        .map_err(|e| AppError::Msg(format!("获取日志目录失败: {e}")))?
        .join("dramadna.log")
        .to_string_lossy()
        .to_string())
}

/// 清空调试日志文件。
#[tauri::command]
pub async fn clear_debug_log(app: AppHandle) -> AppResult<()> {
    let path = app
        .path()
        .app_log_dir()
        .map_err(|e| AppError::Msg(format!("获取日志目录失败: {e}")))?
        .join("dramadna.log");
    std::fs::write(&path, "").map_err(|e| AppError::Msg(format!("清空日志失败: {e}")))?;
    log::info!("调试日志已手动清空");
    Ok(())
}

/// 清空模型测试记录(runs 表)。拆解任务是断点状态数据,不提供清空。
#[tauri::command]
pub async fn clear_runs(db: State<'_, Db>) -> AppResult<()> {
    repo::clear_runs(db.inner()).await
}

// ────────────────────────────── settings ──────────────────────────────

#[tauri::command]
pub async fn get_setting(db: State<'_, Db>, key: String) -> AppResult<Option<String>> {
    repo::get_setting(db.inner(), &key).await
}

#[tauri::command]
pub async fn set_setting(db: State<'_, Db>, key: String, value: String) -> AppResult<()> {
    repo::set_setting(db.inner(), &key, &value).await
}
