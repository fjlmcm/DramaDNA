mod activity;
mod batch;
mod commands;
mod commands_dna;
mod db;
#[cfg(test)]
mod dna_spike;
mod drama;
mod error;
mod ffmpeg;
mod models;
mod pipeline;
mod prompts;
mod provider;
mod repo;
mod repo_dna;
mod writer;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                // 全局 Info;sqlx/hyper/reqwest 的 Debug 噪音会淹没管线日志,静音到 Warn;
                // 自家日志保留 Debug 便于排查。
                .level(log::LevelFilter::Info)
                .level_for("sqlx", log::LevelFilter::Warn)
                .level_for("hyper_util", log::LevelFilter::Warn)
                .level_for("reqwest", log::LevelFilter::Warn)
                .level_for("dramadna_lib", log::LevelFilter::Debug)
                // 持久化:固定文件+容量上限,超限轮转 —— 保证问题排查时历史可查。
                .max_file_size(50 * 1024 * 1024)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("dramadna".into()),
                    }),
                ])
                .build(),
        )
        .setup(|app| {
            log::info!("dramadna 启动");
            // 解析随包 ffmpeg sidecar(打包后与主程序同目录),开发期回落系统 PATH。
            resolve_sidecars();
            // 启动即初始化数据库,确保 command 调用前 schema 就绪。
            let handle = app.handle().clone();
            let db = tauri::async_runtime::block_on(db::Db::init(&handle))?;
            log::info!("数据库初始化完成");
            // 中断恢复:重置上次未跑完的批处理单元。
            if let Ok(n) = tauri::async_runtime::block_on(repo::reset_processing_items(&db)) {
                if n > 0 {
                    log::info!("中断恢复:重置了 {n} 个未完成的批处理单元");
                }
            }
            // 中断恢复:重置上次未跑完的拆解任务单元。
            if let Ok(n) = tauri::async_runtime::block_on(repo_dna::reset_processing_tasks(&db)) {
                if n > 0 {
                    log::info!("中断恢复:重置了 {n} 个未完成的拆解任务");
                }
            }
            // 新版本新增的资产:为已有剧补建任务,「继续拆解」即可增量补齐,无需重新拆解。
            if let Ok(n) = tauri::async_runtime::block_on(repo_dna::ensure_tasks_all_dramas(&db)) {
                if n > 0 {
                    log::info!("为已有剧目补建了 {n} 个新资产任务");
                }
            }
            app.manage(db);
            log::info!("应用就绪");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_providers,
            commands::create_provider,
            commands::update_provider,
            commands::delete_provider,
            commands::list_models,
            commands::create_model,
            commands::update_model,
            commands::delete_model,
            commands::understand_video,
            commands::understand_video_stream,
            commands::cancel_understand_video,
            commands::list_schemes,
            commands::create_scheme,
            commands::update_scheme,
            commands::delete_scheme,
            commands::create_batch_job,
            commands::run_batch_job,
            commands::list_batch_jobs,
            commands::list_job_items,
            commands::cancel_batch_job,
            commands::delete_batch_job,
            commands::export_job_results,
            commands::list_runs,
            commands::read_debug_log,
            commands::debug_log_path,
            commands::clear_debug_log,
            commands::clear_runs,
            commands::get_setting,
            commands::set_setting,
            commands::cache_stats,
            commands::clear_cache,
            commands_dna::import_drama,
            commands_dna::list_dramas,
            commands_dna::delete_drama,
            commands_dna::list_drama_episodes,
            commands_dna::list_asset_specs,
            commands_dna::update_asset_spec,
            commands_dna::reset_asset_spec,
            commands_dna::run_dna_pipeline,
            commands_dna::cancel_dna_pipeline,
            commands_dna::dna_pipeline_running,
            commands_dna::list_dna_tasks,
            commands_dna::retry_failed_tasks,
            commands_dna::rerun_spec,
            commands_dna::reset_drama_tasks,
            commands_dna::list_outputs,
            commands_dna::read_output,
            commands_dna::output_dir,
            commands_dna::dna_activity,
            commands_dna::list_recent_dna_tasks,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 打包后 ffmpeg/ffprobe sidecar 与主程序同目录,解析并设入环境变量。
fn resolve_sidecars() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = exe.parent() else {
        return;
    };
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    for (name, key) in [
        ("ffmpeg", "DRAMADNA_FFMPEG"),
        ("ffprobe", "DRAMADNA_FFPROBE"),
    ] {
        let path = dir.join(format!("{name}{suffix}"));
        if path.exists() {
            log::debug!("ffmpeg sidecar: {name} -> {}", path.display());
            std::env::set_var(key, path);
        }
    }
}
