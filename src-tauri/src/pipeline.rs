// 拆解管线 —— DramaDNA 的执行引擎。
//
// 模型:任务单元 = 剧 × 资产规格 × (集|段|合并)。调度按「波次」推进:
// 每轮取出依赖已满足的 pending 任务并发执行(有界),直到没有可跑任务。
// 依赖以资产的「最终产出」为准:per_segment 资产 = 合并任务(段数为 1 时即该段),
// per_episode 依赖同集结果(或 ":all" 聚合全部集),per_drama = 其唯一任务。
//
// 中断恢复与 batch.rs 同款:启动时 processing → pending,重跑即续。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use tauri::{AppHandle, Manager};
use tokio::sync::Semaphore;

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::models::{AssetSpec, DnaTask, Drama, Episode, Model, Provider};
use crate::prompts::{CONTEXT_HEADER, DEFAULT_MERGE_PROMPT};
use crate::{ffmpeg, provider, repo, repo_dna, writer};

const DEFAULT_CONCURRENCY: usize = 10;
/// 每分钟任务启动数上限 —— 方舟 TPM(1M/分钟)按滑动窗口计,视频任务 input 达
/// 数万 token,并发集中启动会瞬时打爆窗口。100 万 ÷ ~8 万/视频任务 ≈ 12。
/// settings 键 dna_starts_per_min 可调;429 阶梯退避兜底。
const DEFAULT_STARTS_PER_MIN: usize = 12;
const MAX_RETRIES: usize = 2;

/// 启动限速闸(跨剧全局 —— 同账号共享 TPM)。
fn start_gate() -> &'static Mutex<std::collections::VecDeque<Instant>> {
    static GATE: OnceLock<Mutex<std::collections::VecDeque<Instant>>> = OnceLock::new();
    GATE.get_or_init(|| Mutex::new(std::collections::VecDeque::new()))
}

async fn acquire_start_slot(per_min: usize) {
    loop {
        let wait_ms = {
            let mut q = start_gate().lock().unwrap();
            let now = Instant::now();
            while q
                .front()
                .map(|t| now.duration_since(*t).as_secs() >= 60)
                .unwrap_or(false)
            {
                q.pop_front();
            }
            if q.len() < per_min.max(1) {
                q.push_back(now);
                return;
            }
            // 最早一条滑出窗口还需多久。
            60_000u64.saturating_sub(now.duration_since(*q.front().unwrap()).as_millis() as u64)
        };
        tokio::time::sleep(std::time::Duration::from_millis(wait_ms.clamp(200, 5_000))).await;
    }
}

/// 进行中管线的取消标志(drama_id → flag)。
fn cancel_flags() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    static FLAGS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();
    FLAGS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn cancel_pipeline(drama_id: &str) -> bool {
    if let Some(flag) = cancel_flags().lock().unwrap().get(drama_id) {
        flag.store(true, Ordering::Relaxed);
        true
    } else {
        false
    }
}

pub fn pipeline_running(drama_id: &str) -> bool {
    cancel_flags().lock().unwrap().contains_key(drama_id)
}

/// 管线执行上下文 —— 一次 run 内不变的数据。
struct Ctx {
    cache_root: std::path::PathBuf,
    db: Db,
    drama: Drama,
    episodes: Vec<Episode>,
    specs: Vec<AssetSpec>,
    segment_size: i64,
    segment_count: i64,
    video_model: Option<(Provider, Model)>,
    /// 分集视频任务的默认模型(空则跟随 video_model)。全片/分集分工:
    /// 全片只有豆包 file_api 能吃(>60 分钟墙),分集实测 gemini 画外音标注最强。
    episode_video_model: Option<(Provider, Model)>,
    text_model: Option<(Provider, Model)>,
    cancel: Arc<AtomicBool>,
}

/// 启动(或继续)一部剧的拆解管线(GUI 入口:缓存目录取应用缓存目录)。
pub async fn run_pipeline(app: AppHandle, db: Db, drama_id: String) -> AppResult<()> {
    let cache_root = app
        .path()
        .app_cache_dir()
        .map_err(|e| AppError::Msg(format!("获取缓存目录失败: {e}")))?
        .join("preprocessed");
    run_pipeline_with(db, drama_id, cache_root).await
}

/// 通用入口:显式指定缓存目录(测试/CLI 驱动全流程用)。已在跑则报错。
pub async fn run_pipeline_with(
    db: Db,
    drama_id: String,
    cache_root: std::path::PathBuf,
) -> AppResult<()> {
    {
        let mut flags = cancel_flags().lock().unwrap();
        if flags.contains_key(&drama_id) {
            return Err(AppError::Msg("该剧的拆解管线已在运行".into()));
        }
        flags.insert(drama_id.clone(), Arc::new(AtomicBool::new(false)));
    }
    let result = run_pipeline_inner(&db, &drama_id, cache_root).await;
    cancel_flags().lock().unwrap().remove(&drama_id);
    result
}

async fn run_pipeline_inner(
    db: &Db,
    drama_id: &str,
    cache_root: std::path::PathBuf,
) -> AppResult<()> {
    let drama = repo_dna::get_drama(db, drama_id).await?;
    let episodes = repo_dna::list_episodes(db, drama_id).await?;
    if episodes.is_empty() {
        return Err(AppError::Msg("该剧没有分集,请先重新扫描目录".into()));
    }
    let specs = repo_dna::list_specs(db).await?;
    let concurrency = setting_i64(db, "dna_concurrency", DEFAULT_CONCURRENCY as i64).await as usize;
    let starts_per_min =
        setting_i64(db, "dna_starts_per_min", DEFAULT_STARTS_PER_MIN as i64).await as usize;

    // 默认模型(资产未单独绑定时使用)。缺配置不在此处失败 —— 具体任务执行时报错,
    // 便于「先跑纯文本资产」之类的部分推进。
    let video_model = resolve_setting_model(db, "dna_video_model").await;
    let episode_video_model = resolve_setting_model(db, "dna_video_model_episode").await;
    let text_model = resolve_setting_model(db, "dna_text_model").await;

    // 全局资产恒用全集:单集的事情用单集,全局的事情用全集(不分段)。
    // 全集素材经逐集重编码+拼接后时长常在 1 小时级,未触达各家官方输入限制。
    let ep_count = episodes.len() as i64;
    let (segment_size, segment_count) = (ep_count.max(1), 1);

    // 救活孤儿:热重载/崩溃遗留的 processing 在此重置,「继续拆解」即恢复。
    let orphans = repo_dna::reset_processing_tasks_of(db, drama_id)
        .await
        .unwrap_or(0);
    if orphans > 0 {
        log::info!("重置了 {orphans} 个中断遗留的任务");
    }
    let episode_ids: Vec<String> = episodes.iter().map(|e| e.id.clone()).collect();
    let created = repo_dna::ensure_tasks(db, drama_id, &specs, &episode_ids, segment_count).await?;
    log::info!(
        "管线启动: {} —— {} 集 / {} 段(每段 {} 集),新建 {} 个任务单元,并发 {}",
        drama.name,
        episodes.len(),
        segment_count,
        segment_size,
        created,
        concurrency
    );

    let cancel = cancel_flags()
        .lock()
        .unwrap()
        .get(drama_id)
        .cloned()
        .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
    let ctx = Arc::new(Ctx {
        cache_root,
        db: db.clone(),
        drama,
        episodes,
        specs,
        segment_size,
        segment_count,
        video_model,
        episode_video_model,
        text_model,
        cancel,
    });

    // 连续调度:任一任务完成即重扫依赖、立即补位派发 —— 消除波次门闩,
    // 慢的全局任务不再阻塞已逐集就绪的分集链(台词完成即可跑该集拆解卡/标注)。
    let sem = Arc::new(Semaphore::new(concurrency.max(1)));
    let mut dispatched: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut inflight: tokio::task::JoinSet<()> = tokio::task::JoinSet::new();
    loop {
        if ctx.cancel.load(Ordering::Relaxed) {
            log::info!("管线已取消: {}", ctx.drama.name);
            break;
        }
        let tasks = repo_dna::list_tasks(db, drama_id).await?;
        let ready: Vec<DnaTask> = tasks
            .iter()
            .filter(|t| {
                t.status == "pending"
                    && !dispatched.contains(&t.id)
                    && deps_satisfied(&ctx, t, &tasks)
            })
            .cloned()
            .collect();
        for task in ready {
            // 信号量满时在此等待空位 —— 效果即滑动窗口。
            let permit = sem
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| AppError::Msg(format!("并发信号量错误: {e}")))?;
            if ctx.cancel.load(Ordering::Relaxed) {
                break;
            }
            // TPM 平滑:限制每分钟启动数,避免并发集中提交瞬时打爆配额。
            acquire_start_slot(starts_per_min).await;
            dispatched.insert(task.id.clone());
            let ctx = ctx.clone();
            inflight.spawn(async move {
                let _permit = permit;
                process_task(&ctx, &task).await;
            });
        }
        if inflight.is_empty() {
            let stranded = tasks
                .iter()
                .filter(|t| t.status == "pending" && !dispatched.contains(&t.id))
                .count();
            if stranded > 0 {
                log::warn!(
                    "管线结束但仍有 {stranded} 个任务因依赖未满足无法执行(上游失败?)—— 修复后可「继续」"
                );
            } else {
                // 全部完成:程序统计节奏数据落盘,再输出本剧拆解总耗时(活动行 + 日志)。
                write_pacing_stats(&ctx).await;
                if let Ok((n, call_ms, wall_ms)) = repo_dna::drama_time_stats(db, drama_id).await {
                    let msg = format!(
                        "《{}》拆解完成:{} 个任务,调用累计 {:.0} 分钟,全程 {:.0} 分钟",
                        ctx.drama.name,
                        n,
                        call_ms as f64 / 60000.0,
                        wall_ms as f64 / 60000.0
                    );
                    crate::activity::set(msg.clone());
                    log::info!("{msg}");
                } else {
                    log::info!("管线全部完成: {}", ctx.drama.name);
                }
            }
            break;
        }
        // 等任一任务完成 —— 其产出可能解锁新的下游任务,随即重扫。
        let _ = inflight.join_next().await;
    }
    // 取消场景:等在跑的任务自然收尾。
    while inflight.join_next().await.is_some() {}
    Ok(())
}

/// 全剧完成后写「11-节奏数据.md」—— 程序统计的每集硬指标(时长/场次/台词句数/镜头数),
/// 不调模型;台词密度是二创剧本的容量红线。
async fn write_pacing_stats(ctx: &Ctx) {
    let fetch =
        |spec_id: &'static str| repo_dna::done_results_of_spec(&ctx.db, &ctx.drama.id, spec_id);
    let transcripts = fetch("b-transcript").await.unwrap_or_default();
    if transcripts.is_empty() {
        return;
    }
    let breakdowns = fetch("b-breakdown").await.unwrap_or_default();
    let shots = fetch("b-shotlist").await.unwrap_or_default();
    let text_of = |list: &[DnaTask], ep_id: &str| {
        list.iter()
            .find(|t| t.episode_id.as_deref() == Some(ep_id))
            .and_then(|t| t.result_text.clone())
            .unwrap_or_default()
    };
    let rows: Vec<writer::PacingRow> = ctx
        .episodes
        .iter()
        .map(|e| writer::PacingRow {
            ep_no: e.ep_no,
            duration_sec: e.duration_sec.unwrap_or(0.0),
            scene_count: writer::count_scene_lines(&text_of(&breakdowns, &e.id)),
            line_count: writer::count_numbered_lines(&text_of(&transcripts, &e.id)),
            shot_count: writer::count_shot_rows(&text_of(&shots, &e.id)),
        })
        .collect();
    let md = writer::build_pacing_md(&ctx.drama.name, &rows);
    let path = writer::render_output_path(&ctx.drama.dir_path, "11-节奏数据.md", None);
    if let Err(e) = writer::write_output(&path, &md) {
        log::warn!("写节奏数据失败: {e}");
    }
}

async fn setting_i64(db: &Db, key: &str, default: i64) -> i64 {
    repo::get_setting(db, key)
        .await
        .ok()
        .flatten()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(default)
}

/// settings 里存的模型 id → (Provider, Model)。
async fn resolve_setting_model(db: &Db, key: &str) -> Option<(Provider, Model)> {
    let model_id = repo::get_setting(db, key).await.ok().flatten()?;
    resolve_model(db, &model_id).await
}

async fn resolve_model(db: &Db, model_id: &str) -> Option<(Provider, Model)> {
    let model = repo::get_model(db, model_id).await.ok()?;
    let provider = repo::get_provider(db, &model.provider_id).await.ok()?;
    Some((provider, model))
}

// ────────────────────────────── 依赖判定 ──────────────────────────────

fn spec_of<'a>(ctx: &'a Ctx, spec_id: &str) -> Option<&'a AssetSpec> {
    ctx.specs.iter().find(|s| s.id == spec_id)
}

fn parse_inputs(spec: &AssetSpec) -> Vec<(String, bool)> {
    // → [(依赖 spec_id, 是否 ":all" 全集聚合)]
    serde_json::from_str::<Vec<String>>(&spec.inputs)
        .unwrap_or_default()
        .into_iter()
        .map(|s| match s.strip_suffix(":all") {
            Some(id) => (id.to_string(), true),
            None => (s, false),
        })
        .collect()
}

/// 依赖资产的「最终产出」是否已完成。
fn final_done(ctx: &Ctx, dep: &AssetSpec, tasks: &[DnaTask]) -> bool {
    match dep.scope.as_str() {
        "per_segment" => {
            let target = if ctx.segment_count > 1 { 0 } else { 1 };
            tasks
                .iter()
                .any(|t| t.spec_id == dep.id && t.segment_no == Some(target) && t.status == "done")
        }
        "per_episode" => tasks
            .iter()
            .filter(|t| t.spec_id == dep.id)
            .all(|t| t.status == "done"),
        _ => tasks
            .iter()
            .any(|t| t.spec_id == dep.id && t.status == "done"),
    }
}

fn deps_satisfied(ctx: &Ctx, task: &DnaTask, tasks: &[DnaTask]) -> bool {
    let Some(spec) = spec_of(ctx, &task.spec_id) else {
        return false;
    };
    // per_segment 合并任务:依赖 = 本资产全部段完成。
    if spec.scope == "per_segment" && task.segment_no == Some(0) {
        return tasks
            .iter()
            .filter(|t| t.spec_id == spec.id && t.segment_no.unwrap_or(0) >= 1)
            .all(|t| t.status == "done");
    }
    for (dep_id, all) in parse_inputs(spec) {
        let Some(dep) = spec_of(ctx, &dep_id) else {
            return false;
        };
        let ok = if all || dep.scope != "per_episode" {
            final_done(ctx, dep, tasks)
        } else {
            // 同集依赖(仅 per_episode → per_episode)。
            tasks.iter().any(|t| {
                t.spec_id == dep_id && t.episode_id == task.episode_id && t.status == "done"
            })
        };
        if !ok {
            return false;
        }
    }
    true
}

// ────────────────────────────── 任务执行 ──────────────────────────────

async fn process_task(ctx: &Ctx, task: &DnaTask) {
    let Some(spec) = spec_of(ctx, &task.spec_id) else {
        let _ = repo_dna::set_task_failed(&ctx.db, &task.id, "资产规格不存在").await;
        return;
    };
    let label = task_label(ctx, spec, task);
    let _ = repo_dna::set_task_processing(&ctx.db, &task.id).await;
    crate::activity::set(format!("{label}:准备输入…"));
    log::info!("任务开始: {label}");
    let start = Instant::now();

    let mut last_err = String::new();
    for attempt in 0..=MAX_RETRIES {
        if ctx.cancel.load(Ordering::Relaxed) {
            // 留在 processing,由启动恢复重置 —— 与 batch 语义一致。
            return;
        }
        match execute_task(ctx, spec, task).await {
            Ok(text) => {
                let ms = start.elapsed().as_millis() as i64;
                crate::activity::set(format!("{label}:完成({}s)", ms / 1000));
                let _ = repo_dna::set_task_done(&ctx.db, &task.id, &text, ms).await;
                // 最终产出落盘 md;per_segment 的段任务是中间产物,只有合并稿
                // (或段数为 1 时的唯一段)写盘。
                let is_final = match spec.scope.as_str() {
                    "per_segment" => {
                        task.segment_no == Some(0)
                            || (ctx.segment_count == 1 && task.segment_no == Some(1))
                    }
                    _ => true,
                };
                if is_final {
                    let ep_no = task
                        .episode_id
                        .as_ref()
                        .and_then(|id| ctx.episodes.iter().find(|e| &e.id == id))
                        .map(|e| e.ep_no);
                    let path = writer::render_output_path(
                        &ctx.drama.dir_path,
                        &spec.output_template,
                        ep_no,
                    );
                    if let Err(e) = writer::write_output(&path, &text) {
                        log::warn!("写产出文件失败({label}): {e}");
                    }
                }
                log::info!("任务完成: {label} ({ms}ms)");
                return;
            }
            Err(e) => {
                last_err = e.to_string();
                if attempt < MAX_RETRIES {
                    // 限流(429/TPM)按分钟滑动窗口计——3 秒重试毫无意义,
                    // 阶梯退避 60/120 秒等窗口滑过;其他错误快速重试。
                    let rate_limited = last_err.contains("429")
                        || last_err.contains("RateLimit")
                        || last_err.contains("TpmRateLimit");
                    let delay = if rate_limited {
                        60 * (attempt as u64 + 1)
                    } else {
                        3
                    };
                    log::warn!(
                        "任务失败将重试({}/{},{}s 后): {label} — {last_err}",
                        attempt + 1,
                        MAX_RETRIES + 1,
                        delay
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                }
            }
        }
    }
    let _ = repo_dna::set_task_failed(&ctx.db, &task.id, &last_err).await;
    log::warn!("任务最终失败: {label} — {last_err}");
}

fn task_label(ctx: &Ctx, spec: &AssetSpec, task: &DnaTask) -> String {
    let unit = if let Some(ep_id) = &task.episode_id {
        ctx.episodes
            .iter()
            .find(|e| &e.id == ep_id)
            .map(|e| format!("第{}集", e.ep_no))
            .unwrap_or_else(|| "?".into())
    } else if let Some(n) = task.segment_no {
        if n == 0 && spec.scope == "per_segment" {
            "合并".into()
        } else if spec.scope == "per_segment" {
            format!("段{n}")
        } else {
            format!("#{n}")
        }
    } else {
        "全剧".into()
    };
    format!("{}·{unit}", spec.name)
}

impl Ctx {
    /// 第 n 段(1 起)包含的集。
    fn segment_episodes(&self, n: i64) -> Vec<&Episode> {
        let start = ((n - 1) * self.segment_size) as usize;
        self.episodes
            .iter()
            .skip(start)
            .take(self.segment_size as usize)
            .collect()
    }
}

async fn execute_task(ctx: &Ctx, spec: &AssetSpec, task: &DnaTask) -> AppResult<String> {
    let is_merge = spec.scope == "per_segment" && task.segment_no == Some(0);
    let needs_video = spec.needs_video && !is_merge;

    // 1. 选模型:资产绑定优先,否则按 needs_video 用管线默认。
    let bound = match &spec.model_id {
        Some(id) => resolve_model(&ctx.db, id).await,
        None => None,
    };
    let default = if needs_video {
        if task.episode_id.is_some() {
            // 分集视频任务:优先「分集视频模型」,未设置则跟随视频模型。
            ctx.episode_video_model
                .clone()
                .or_else(|| ctx.video_model.clone())
        } else {
            ctx.video_model.clone()
        }
    } else {
        ctx.text_model.clone()
    };
    let Some((provider_row, model)) = bound.or(default) else {
        return Err(AppError::Msg(if needs_video {
            "未配置视频模型(设置页 → 拆解管线 → 视频模型)".into()
        } else {
            "未配置文本模型(设置页 → 拆解管线 → 文本模型)".into()
        }));
    };

    // 2. 组 prompt。
    let prompt = build_prompt(ctx, spec, task, is_merge).await?;

    // 3. 准备视频(需要时)。
    let video_path = if needs_video {
        Some(prepare_video(ctx, spec, task, &provider_row, &model).await?)
    } else {
        None
    };

    // 4. 调用。file_api 模型(豆包 Files API):上传拿 file_id 走 Responses API,
    // 支持全剧整段;其余走 base64 data-url 的 Chat API。
    let label = task_label(ctx, spec, task);
    let extra: serde_json::Value = spec.params.parse().unwrap_or(serde_json::json!({}));
    match video_path {
        Some(ref path) if model.video_input_method == "file_api" => {
            // 单集:高抽帧率保台词、默认 token 预算;段/全剧:1fps(被 1280 帧
            // 上限钳制为约 9 秒一帧)+ 200k 视频 token 预算提升单帧清晰度。
            // 单集 3fps:字幕 2-4 秒一条,3fps 足够,且大幅减轻平台预处理排队。
            let (fps, max_vt) = if task.episode_id.is_some() {
                (3.0, 0)
            } else {
                (1.0, 200_000)
            };
            let file_id = provider::ark_file_for(&ctx.db, &provider_row, path, fps, max_vt).await?;
            // 前缀缓存预热:同一 file_id 的多个任务(全剧的三个全局资产、
            // 单集的转录/拆解卡/标注)并发引用同一视频前缀 —— 确定性命中
            // 缓存价且任务上下文互不可见。
            let prev = provider::ark_prefix_warm(&ctx.db, &provider_row, &model, &file_id).await;
            crate::activity::set(format!("{label}:模型生成中({})…", model.display_name));
            provider::complete_file_api(
                &provider_row,
                &model,
                &prompt,
                &file_id,
                &extra,
                prev.as_deref(),
            )
            .await
        }
        _ => {
            crate::activity::set(format!("{label}:模型生成中({})…", model.display_name));
            provider::complete(
                &provider_row,
                &model,
                &prompt,
                video_path.as_deref(),
                &extra,
            )
            .await
        }
    }
}

/// 单集直接用原文件走 ensure_compliant;分段任务先拼接再压缩(自适应阶梯压到模型上限)。
async fn prepare_video(
    ctx: &Ctx,
    _spec: &AssetSpec,
    task: &DnaTask,
    provider_row: &Provider,
    model: &Model,
) -> AppResult<String> {
    let constraints = ffmpeg::VideoConstraints::from_json_with(
        &model.constraints,
        provider::video_constraint_defaults(&provider_row.kind),
    );
    let cache_dir = ctx.cache_root.clone();

    let is_file_api = model.video_input_method == "file_api";
    if let Some(ep_id) = &task.episode_id {
        let source = ctx
            .episodes
            .iter()
            .find(|e| &e.id == ep_id)
            .map(|e| e.file_path.clone())
            .ok_or_else(|| AppError::Msg("分集不存在".into()))?;
        // 单集原始文件时间戳干净:file_api 直接上传,base64 走压缩约束。
        if is_file_api {
            return Ok(source);
        }
        return ffmpeg::ensure_compliant(&source, &constraints, &cache_dir).await;
    }
    let n = task.segment_no.unwrap_or(1);
    let paths: Vec<String> = ctx
        .segment_episodes(n)
        .iter()
        .map(|e| e.file_path.clone())
        .collect();
    // 全集标准视频:逐集重编码 → 拼接(时间戳干净是全集理解质量的生命线)。
    let merged = ffmpeg::concat_normalized(&paths, &cache_dir).await?;
    if is_file_api {
        Ok(merged)
    } else {
        // 云雾 Gemini 实测(2026-07)拒绝 >60 分钟视频(与体积/media_resolution/音频无关),
        // 只预警不拦截:让上游报错保留完整现场,超长剧全集应绑定豆包 file_api 模型。
        if provider_row.kind == "gemini" {
            if let Ok(p) = ffmpeg::probe(&merged).await {
                if p.duration_s > 3600.0 {
                    log::warn!(
                        "全集视频 {:.0} 分钟,超过云雾 Gemini 实测约 60 分钟的时长墙,任务大概率失败;\
                         建议该资产绑定豆包 file_api 模型",
                        p.duration_s / 60.0
                    );
                }
            }
        }
        // base64 模型:继续压到该模型的 data-uri 体积约束内。
        ffmpeg::ensure_compliant(&merged, &constraints, &cache_dir).await
    }
}

// ────────────────────────────── prompt 组装 ──────────────────────────────

async fn build_prompt(
    ctx: &Ctx,
    spec: &AssetSpec,
    task: &DnaTask,
    is_merge: bool,
) -> AppResult<String> {
    let template = if is_merge {
        spec.merge_prompt
            .clone()
            .unwrap_or_else(|| DEFAULT_MERGE_PROMPT.to_string())
    } else {
        spec.prompt.clone()
    };
    let mut text = format!("{CONTEXT_HEADER}{template}");

    // 占位符。
    let ep = task
        .episode_id
        .as_ref()
        .and_then(|id| ctx.episodes.iter().find(|e| &e.id == id));
    let seg_no = task.segment_no.unwrap_or(0);
    let ep_range = if seg_no >= 1 {
        let eps = ctx.segment_episodes(seg_no);
        match (eps.first(), eps.last()) {
            (Some(a), Some(b)) => format!("{}-{}", a.ep_no, b.ep_no),
            _ => String::new(),
        }
    } else {
        String::new()
    };
    let ep_titles: String = ctx
        .episodes
        .iter()
        .map(|e| format!("第{}集:{}", e.ep_no, e.title))
        .collect::<Vec<_>>()
        .join("\n");
    // 集边界时间表 —— 全集拼接视频内各集的时间范围(供全局资产按集定位)。
    let ep_timeline: String = {
        let mut t = 0.0_f64;
        ctx.episodes
            .iter()
            .map(|e| {
                let d = e.duration_sec.unwrap_or(0.0);
                let line = format!(
                    "第{}集: {}:{:02} - {}:{:02}",
                    e.ep_no,
                    (t / 60.0) as i64,
                    (t % 60.0) as i64,
                    ((t + d) / 60.0) as i64,
                    ((t + d) % 60.0) as i64
                );
                t += d;
                line
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    for (key, value) in [
        ("{drama_name}", ctx.drama.name.clone()),
        ("{episode_count}", ctx.drama.episode_count.to_string()),
        (
            "{ep_no}",
            ep.map(|e| e.ep_no.to_string()).unwrap_or_default(),
        ),
        (
            "{ep_title}",
            ep.map(|e| e.title.clone()).unwrap_or_default(),
        ),
        ("{ep_range}", ep_range),
        ("{segment_no}", seg_no.to_string()),
        ("{segment_count}", ctx.segment_count.to_string()),
        ("{ep_titles}", ep_titles),
        ("{ep_timeline}", ep_timeline),
        (
            "{total_minutes}",
            format!("{:.0}", ctx.drama.total_duration_sec / 60.0),
        ),
        ("{asset_name}", spec.name.clone()),
    ] {
        text = text.replace(key, &value);
    }

    // 参考资料:合并任务 = 本资产各段结果;普通任务 = inputs 声明的依赖资产。
    if is_merge {
        let results = repo_dna::done_results_of_spec(&ctx.db, &ctx.drama.id, &spec.id).await?;
        for t in results.iter().filter(|t| t.segment_no.unwrap_or(0) >= 1) {
            let n = t.segment_no.unwrap_or(0);
            let eps = ctx.segment_episodes(n);
            let range = match (eps.first(), eps.last()) {
                (Some(a), Some(b)) => format!("第{}-{}集", a.ep_no, b.ep_no),
                _ => String::new(),
            };
            text.push_str(&format!(
                "\n\n---\n\n## 第 {n} 段草稿({range})\n\n{}",
                t.result_text.as_deref().unwrap_or("")
            ));
        }
        return Ok(text);
    }

    for (dep_id, all) in parse_inputs(spec) {
        let Some(dep) = spec_of(ctx, &dep_id) else {
            continue;
        };
        let content = reference_content(ctx, dep, task, all).await?;
        text.push_str(&format!("\n\n---\n\n## 参考资料:{}\n\n{content}", dep.name));
    }
    Ok(text)
}

/// 依赖资产的参考内容。
async fn reference_content(
    ctx: &Ctx,
    dep: &AssetSpec,
    task: &DnaTask,
    all: bool,
) -> AppResult<String> {
    let results = repo_dna::done_results_of_spec(&ctx.db, &ctx.drama.id, &dep.id).await?;
    let content = match dep.scope.as_str() {
        "per_segment" => {
            let target = if ctx.segment_count > 1 { 0 } else { 1 };
            results
                .iter()
                .find(|t| t.segment_no == Some(target))
                .and_then(|t| t.result_text.clone())
                .unwrap_or_default()
        }
        "per_episode" if all => results
            .iter()
            .map(|t| {
                let ep_no = t
                    .episode_id
                    .as_ref()
                    .and_then(|id| ctx.episodes.iter().find(|e| &e.id == id))
                    .map(|e| e.ep_no)
                    .unwrap_or(0);
                format!(
                    "### 第{}集\n\n{}",
                    ep_no,
                    t.result_text.as_deref().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
        "per_episode" => results
            .iter()
            .find(|t| t.episode_id == task.episode_id)
            .and_then(|t| t.result_text.clone())
            .unwrap_or_default(),
        _ => results
            .first()
            .and_then(|t| t.result_text.clone())
            .unwrap_or_default(),
    };
    Ok(content)
}
