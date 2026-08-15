// DramaDNA 命令层 —— 薄转发到 drama / repo_dna / pipeline / writer。

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::models::{AssetSpec, DnaTask, Drama, Episode};
use crate::repo_dna::{self, SpecUpdate};
use crate::{drama, pipeline, writer};

// ────────────────────────────── 剧目 ──────────────────────────────

#[tauri::command]
pub async fn import_drama(db: State<'_, Db>, dir_path: String) -> AppResult<Drama> {
    drama::import_drama_dir(db.inner(), &dir_path).await
}

#[tauri::command]
pub async fn list_dramas(db: State<'_, Db>) -> AppResult<Vec<Drama>> {
    repo_dna::list_dramas(db.inner()).await
}

#[tauri::command]
pub async fn delete_drama(db: State<'_, Db>, id: String) -> AppResult<()> {
    if pipeline::pipeline_running(&id) {
        return Err(AppError::Msg("该剧管线正在运行,请先停止".into()));
    }
    repo_dna::delete_drama(db.inner(), &id).await
}

#[tauri::command]
pub async fn list_drama_episodes(db: State<'_, Db>, drama_id: String) -> AppResult<Vec<Episode>> {
    repo_dna::list_episodes(db.inner(), &drama_id).await
}

// ────────────────────────────── 资产规格 ──────────────────────────────

#[tauri::command]
pub async fn list_asset_specs(db: State<'_, Db>) -> AppResult<Vec<AssetSpec>> {
    repo_dna::list_specs(db.inner()).await
}

#[tauri::command]
pub async fn update_asset_spec(
    db: State<'_, Db>,
    id: String,
    update: SpecUpdate,
) -> AppResult<AssetSpec> {
    repo_dna::update_spec(db.inner(), &id, update).await
}

#[tauri::command]
pub async fn reset_asset_spec(db: State<'_, Db>, id: String) -> AppResult<AssetSpec> {
    repo_dna::reset_spec_to_builtin(db.inner(), &id).await
}

// ────────────────────────────── 管线 ──────────────────────────────

#[tauri::command]
pub async fn run_dna_pipeline(
    app: AppHandle,
    db: State<'_, Db>,
    drama_id: String,
) -> AppResult<()> {
    if pipeline::pipeline_running(&drama_id) {
        return Err(AppError::Msg("该剧管线已在运行".into()));
    }
    let db = db.inner().clone();
    tokio::spawn(async move {
        if let Err(e) = pipeline::run_pipeline(app, db, drama_id).await {
            log::error!("管线运行失败: {e}");
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn cancel_dna_pipeline(drama_id: String) -> AppResult<bool> {
    Ok(pipeline::cancel_pipeline(&drama_id))
}

#[tauri::command]
pub async fn dna_pipeline_running(drama_id: String) -> AppResult<bool> {
    Ok(pipeline::pipeline_running(&drama_id))
}

#[tauri::command]
pub async fn list_dna_tasks(db: State<'_, Db>, drama_id: String) -> AppResult<Vec<DnaTask>> {
    repo_dna::list_tasks(db.inner(), &drama_id).await
}

#[tauri::command]
pub async fn retry_failed_tasks(db: State<'_, Db>, drama_id: String) -> AppResult<u64> {
    repo_dna::reset_failed_tasks(db.inner(), &drama_id).await
}

/// 重新拆解:重置整剧全部任务(破坏性,前端需二次确认)。
#[tauri::command]
pub async fn reset_drama_tasks(db: State<'_, Db>, drama_id: String) -> AppResult<u64> {
    if pipeline::pipeline_running(&drama_id) {
        return Err(AppError::Msg("管线正在运行,请先停止".into()));
    }
    repo_dna::reset_all_tasks(db.inner(), &drama_id).await
}

#[tauri::command]
pub async fn rerun_spec(db: State<'_, Db>, drama_id: String, spec_id: String) -> AppResult<u64> {
    if pipeline::pipeline_running(&drama_id) {
        return Err(AppError::Msg("管线正在运行,请先停止再重跑".into()));
    }
    repo_dna::reset_tasks_of_spec(db.inner(), &drama_id, &spec_id).await
}

// ────────────────────────────── 产出浏览 ──────────────────────────────

#[tauri::command]
pub async fn list_recent_dna_tasks(
    db: State<'_, Db>,
    limit: Option<i64>,
) -> AppResult<Vec<repo_dna::DnaTaskView>> {
    repo_dna::list_recent_tasks(db.inner(), limit.unwrap_or(300)).await
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityInfo {
    pub text: String,
    pub age_secs: u64,
}

/// 管线当前活动状态行(全局单行,前端轮询)。
#[tauri::command]
pub async fn dna_activity() -> AppResult<ActivityInfo> {
    let (text, age_secs) = crate::activity::get();
    Ok(ActivityInfo { text, age_secs })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputFile {
    pub rel_path: String,
    pub abs_path: String,
    pub size_bytes: u64,
}

#[tauri::command]
pub async fn list_outputs(db: State<'_, Db>, drama_id: String) -> AppResult<Vec<OutputFile>> {
    let drama = repo_dna::get_drama(db.inner(), &drama_id).await?;
    let root = writer::output_root(&drama.dir_path);
    let mut files = Vec::new();
    collect_md(&root, &root, &mut files);
    files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(files)
}

fn collect_md(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<OutputFile>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_md(root, &path, out);
        } else if path.extension().map(|e| e == "md").unwrap_or(false) {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            out.push(OutputFile {
                rel_path: path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string(),
                abs_path: path.to_string_lossy().to_string(),
                size_bytes: size,
            });
        }
    }
}

/// 读取产出 md 内容 —— 仅允许读该剧拆解目录内的文件。
#[tauri::command]
pub async fn read_output(
    db: State<'_, Db>,
    drama_id: String,
    rel_path: String,
) -> AppResult<String> {
    let drama = repo_dna::get_drama(db.inner(), &drama_id).await?;
    let root = writer::output_root(&drama.dir_path);
    let path = root.join(&rel_path);
    let canonical = path
        .canonicalize()
        .map_err(|e| AppError::Msg(format!("文件不存在: {e}")))?;
    let canonical_root = root
        .canonicalize()
        .map_err(|e| AppError::Msg(format!("拆解目录不存在: {e}")))?;
    if !canonical.starts_with(&canonical_root) {
        return Err(AppError::Msg("非法路径".into()));
    }
    std::fs::read_to_string(&canonical).map_err(|e| AppError::Msg(format!("读取失败: {e}")))
}

#[tauri::command]
pub async fn output_dir(db: State<'_, Db>, drama_id: String) -> AppResult<String> {
    let drama = repo_dna::get_drama(db.inner(), &drama_id).await?;
    Ok(writer::output_root(&drama.dir_path)
        .to_string_lossy()
        .to_string())
}
