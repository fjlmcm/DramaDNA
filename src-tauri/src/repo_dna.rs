// DramaDNA 领域仓储 —— dramas / episodes / asset_specs / dna_tasks。
// 通用表(providers/models/settings)的访问仍在 repo.rs。

use chrono::Utc;
use uuid::Uuid;

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::models::{AssetSpec, DnaTask, Drama, Episode};

fn now() -> String {
    Utc::now().to_rfc3339()
}
fn new_id() -> String {
    Uuid::new_v4().to_string()
}

// ────────────────────────────── dramas ──────────────────────────────

pub async fn list_dramas(db: &Db) -> AppResult<Vec<Drama>> {
    Ok(
        sqlx::query_as::<_, Drama>("SELECT * FROM dramas ORDER BY created_at DESC")
            .fetch_all(&db.pool)
            .await?,
    )
}

pub async fn get_drama(db: &Db, id: &str) -> AppResult<Drama> {
    Ok(
        sqlx::query_as::<_, Drama>("SELECT * FROM dramas WHERE id = ?")
            .bind(id)
            .fetch_one(&db.pool)
            .await?,
    )
}

pub async fn delete_drama(db: &Db, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM dramas WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await?;
    Ok(())
}

/// 导入(或重新扫描)剧目:按 dir_path 幂等。集列表未变时保留 episodes 原行
/// (拆解任务不受影响);集列表真的变化才整体替换(旧任务随级联作废)。
pub struct EpisodeMeta {
    pub ep_no: i64,
    pub title: String,
    pub file_path: String,
    pub duration_sec: Option<f64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
}

pub async fn upsert_drama(
    db: &Db,
    name: &str,
    dir_path: &str,
    episodes: Vec<EpisodeMeta>,
) -> AppResult<Drama> {
    let existing: Option<Drama> = sqlx::query_as("SELECT * FROM dramas WHERE dir_path = ?")
        .bind(dir_path)
        .fetch_optional(&db.pool)
        .await?;
    let ts = now();
    let total: f64 = episodes.iter().filter_map(|e| e.duration_sec).sum();
    let count = episodes.len() as i64;

    let drama_id = match existing {
        Some(d) => {
            sqlx::query(
                "UPDATE dramas SET name = ?, episode_count = ?, total_duration_sec = ?, updated_at = ? WHERE id = ?",
            )
            .bind(name)
            .bind(count)
            .bind(total)
            .bind(&ts)
            .bind(&d.id)
            .execute(&db.pool)
            .await?;
            d.id
        }
        None => {
            let id = new_id();
            sqlx::query(
                "INSERT INTO dramas (id, name, dir_path, episode_count, total_duration_sec, meta, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, '{}', ?, ?)",
            )
            .bind(&id)
            .bind(name)
            .bind(dir_path)
            .bind(count)
            .bind(total)
            .bind(&ts)
            .bind(&ts)
            .execute(&db.pool)
            .await?;
            id
        }
    };

    // 集列表未变(逐集 ep_no+file_path 一致)时保留原行 —— 只刷新元数据。
    // 重新导入同一目录不能作废已有拆解任务(episodes 级联会清空 per_episode 任务)。
    let existing_eps: Vec<Episode> =
        sqlx::query_as("SELECT * FROM episodes WHERE drama_id = ? ORDER BY ep_no")
            .bind(&drama_id)
            .fetch_all(&db.pool)
            .await?;
    let unchanged = existing_eps.len() == episodes.len()
        && existing_eps
            .iter()
            .zip(episodes.iter())
            .all(|(a, b)| a.ep_no == b.ep_no && a.file_path == b.file_path);
    if unchanged {
        for e in &episodes {
            sqlx::query(
                "UPDATE episodes SET title = ?, duration_sec = ?, width = ?, height = ?
                 WHERE drama_id = ? AND ep_no = ?",
            )
            .bind(&e.title)
            .bind(e.duration_sec)
            .bind(e.width)
            .bind(e.height)
            .bind(&drama_id)
            .bind(e.ep_no)
            .execute(&db.pool)
            .await?;
        }
        return get_drama(db, &drama_id).await;
    }
    // 集列表真的变了:整体替换,旧任务随级联作废(符合预期)。
    sqlx::query("DELETE FROM episodes WHERE drama_id = ?")
        .bind(&drama_id)
        .execute(&db.pool)
        .await?;
    for e in &episodes {
        sqlx::query(
            "INSERT INTO episodes (id, drama_id, ep_no, title, file_path, duration_sec, width, height)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(new_id())
        .bind(&drama_id)
        .bind(e.ep_no)
        .bind(&e.title)
        .bind(&e.file_path)
        .bind(e.duration_sec)
        .bind(e.width)
        .bind(e.height)
        .execute(&db.pool)
        .await?;
    }
    get_drama(db, &drama_id).await
}

pub async fn list_episodes(db: &Db, drama_id: &str) -> AppResult<Vec<Episode>> {
    Ok(
        sqlx::query_as::<_, Episode>("SELECT * FROM episodes WHERE drama_id = ? ORDER BY ep_no")
            .bind(drama_id)
            .fetch_all(&db.pool)
            .await?,
    )
}

// ────────────────────────────── asset_specs ──────────────────────────────

pub async fn list_specs(db: &Db) -> AppResult<Vec<AssetSpec>> {
    Ok(
        sqlx::query_as::<_, AssetSpec>("SELECT * FROM asset_specs ORDER BY sort_no")
            .fetch_all(&db.pool)
            .await?,
    )
}

pub async fn get_spec(db: &Db, id: &str) -> AppResult<AssetSpec> {
    Ok(
        sqlx::query_as::<_, AssetSpec>("SELECT * FROM asset_specs WHERE id = ?")
            .bind(id)
            .fetch_one(&db.pool)
            .await?,
    )
}

/// 前端可编辑的字段:prompt / merge_prompt / model_id / enabled / params。
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecUpdate {
    pub prompt: String,
    pub merge_prompt: Option<String>,
    pub model_id: Option<String>,
    pub enabled: bool,
    pub params: String,
}

pub async fn update_spec(db: &Db, id: &str, u: SpecUpdate) -> AppResult<AssetSpec> {
    let affected = sqlx::query(
        "UPDATE asset_specs
         SET prompt = ?, merge_prompt = ?, model_id = ?, enabled = ?, params = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(&u.prompt)
    .bind(&u.merge_prompt)
    .bind(&u.model_id)
    .bind(u.enabled)
    .bind(&u.params)
    .bind(now())
    .bind(id)
    .execute(&db.pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AppError::Msg(format!("资产规格不存在: {id}")));
    }
    get_spec(db, id).await
}

/// 恢复内置 spec 的默认 prompt(用户改乱后一键还原)。
pub async fn reset_spec_to_builtin(db: &Db, id: &str) -> AppResult<AssetSpec> {
    let Some(b) = crate::prompts::BUILTIN_SPECS.iter().find(|s| s.id == id) else {
        return Err(AppError::Msg(format!("非内置资产,无默认可恢复: {id}")));
    };
    sqlx::query(
        "UPDATE asset_specs SET prompt = ?, merge_prompt = ?, params = ?, updated_at = ? WHERE id = ?",
    )
    .bind(b.prompt)
    .bind(b.merge_prompt)
    .bind(b.params)
    .bind(now())
    .bind(id)
    .execute(&db.pool)
    .await?;
    get_spec(db, id).await
}

// ────────────────────────────── dna_tasks ──────────────────────────────

/// 「有效启用」的资产集合:启用且依赖链上没有停用项。
/// 依赖被停用的资产建了任务也只会永远卡 pending(依赖永不满足),不如不建。
pub fn effective_enabled_ids(specs: &[AssetSpec]) -> std::collections::HashSet<String> {
    use std::collections::HashSet;
    let mut ok: HashSet<String> = specs
        .iter()
        .filter(|s| s.enabled)
        .map(|s| s.id.clone())
        .collect();
    // 迭代剔除依赖不在集合内的资产,直到收敛(依赖链最深不过几层)。
    loop {
        let snapshot = ok.clone();
        ok.retain(|id| {
            let spec = specs.iter().find(|s| &s.id == id).unwrap();
            let deps: Vec<String> = serde_json::from_str(&spec.inputs).unwrap_or_default();
            deps.iter()
                .map(|d| d.strip_suffix(":all").unwrap_or(d))
                .all(|d| snapshot.contains(d))
        });
        if ok.len() == snapshot.len() {
            return ok;
        }
    }
}

/// 幂等补建任务单元(靠唯一索引 INSERT OR IGNORE)。
/// 返回新建数量。segment_count 决定 per_segment 资产的段任务数(外加 segment_no=0 的合并任务)。
pub async fn ensure_tasks(
    db: &Db,
    drama_id: &str,
    specs: &[AssetSpec],
    episode_ids: &[String],
    segment_count: i64,
) -> AppResult<u64> {
    let runnable = effective_enabled_ids(specs);
    for s in specs
        .iter()
        .filter(|s| s.enabled && !runnable.contains(&s.id))
    {
        log::warn!("资产「{}」的依赖链上有停用项,本轮不建任务", s.name);
    }
    let ts = now();
    let mut created = 0u64;
    for spec in specs
        .iter()
        .filter(|s| runnable.contains(&s.id) && !s.user_input)
    {
        let units: Vec<(Option<String>, Option<i64>)> = match spec.scope.as_str() {
            "per_episode" => episode_ids
                .iter()
                .map(|e| (Some(e.clone()), None))
                .collect(),
            "per_segment" => {
                let mut v: Vec<(Option<String>, Option<i64>)> =
                    (1..=segment_count).map(|n| (None, Some(n))).collect();
                if segment_count > 1 {
                    v.push((None, Some(0))); // 合并任务
                }
                v
            }
            _ => vec![(None, None)], // per_drama
        };
        for (episode_id, segment_no) in units {
            let r = sqlx::query(
                "INSERT OR IGNORE INTO dna_tasks
                 (id, drama_id, spec_id, episode_id, segment_no, status, attempts, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, 'pending', 0, ?, ?)",
            )
            .bind(new_id())
            .bind(drama_id)
            .bind(&spec.id)
            .bind(&episode_id)
            .bind(segment_no)
            .bind(&ts)
            .bind(&ts)
            .execute(&db.pool)
            .await?;
            created += r.rows_affected();
        }
    }
    Ok(created)
}

/// 启动时为所有剧补建缺失任务 —— 新版本新增的资产,老剧无需「重新拆解」即出现在
/// 进度里(按钮随之变回「继续拆解」)。段数恒 1,与管线的「全局恒全集」一致。
pub async fn ensure_tasks_all_dramas(db: &Db) -> AppResult<u64> {
    let specs = list_specs(db).await?;
    let mut created = 0u64;
    for d in list_dramas(db).await? {
        let eps: Vec<String> = list_episodes(db, &d.id)
            .await?
            .into_iter()
            .map(|e| e.id)
            .collect();
        if eps.is_empty() {
            continue;
        }
        created += ensure_tasks(db, &d.id, &specs, &eps, 1).await?;
    }
    Ok(created)
}

/// 任务列表(调度与 UI 进度用)—— 不拖 result_text 大字段:
/// 130 行全文可达数 MB,UI 每 2.5s 轮询 + 调度每完成一个重扫,会把 SQLite 压到秒级。
pub async fn list_tasks(db: &Db, drama_id: &str) -> AppResult<Vec<DnaTask>> {
    Ok(sqlx::query_as::<_, DnaTask>(
        "SELECT t.id, t.drama_id, t.spec_id, t.episode_id, t.segment_no, t.user_input,
                t.status, NULL AS result_text, t.error, t.attempts, t.duration_ms,
                t.created_at, t.updated_at
         FROM dna_tasks t
         LEFT JOIN episodes e ON e.id = t.episode_id
         WHERE t.drama_id = ?
         ORDER BY t.spec_id, e.ep_no, t.segment_no",
    )
    .bind(drama_id)
    .fetch_all(&db.pool)
    .await?)
}

/// 管线启动时重置本剧遗留的 processing(热重载/崩溃产生的孤儿任务)——
/// 与应用启动时的全局重置互补,保证「继续拆解」总能救活孤儿。
pub async fn reset_processing_tasks_of(db: &Db, drama_id: &str) -> AppResult<u64> {
    let r = sqlx::query(
        "UPDATE dna_tasks SET status = 'pending', updated_at = ? WHERE drama_id = ? AND status = 'processing'",
    )
    .bind(now())
    .bind(drama_id)
    .execute(&db.pool)
    .await?;
    Ok(r.rows_affected())
}

/// 全剧拆解耗时统计:(任务数, 调用累计毫秒, 首任务创建→末任务完成的全程毫秒)。
pub async fn drama_time_stats(db: &Db, drama_id: &str) -> AppResult<(i64, i64, i64)> {
    let row: (i64, Option<i64>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT COUNT(*), SUM(duration_ms), MIN(created_at), MAX(updated_at)
         FROM dna_tasks WHERE drama_id = ? AND status = 'done'",
    )
    .bind(drama_id)
    .fetch_one(&db.pool)
    .await?;
    let wall = match (&row.2, &row.3) {
        (Some(a), Some(b)) => {
            let pa = chrono::DateTime::parse_from_rfc3339(a).ok();
            let pb = chrono::DateTime::parse_from_rfc3339(b).ok();
            match (pa, pb) {
                (Some(a), Some(b)) => (b - a).num_milliseconds().max(0),
                _ => 0,
            }
        }
        _ => 0,
    };
    Ok((row.0, row.1.unwrap_or(0), wall))
}

pub async fn set_task_processing(db: &Db, id: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE dna_tasks SET status = 'processing', attempts = attempts + 1, updated_at = ? WHERE id = ?",
    )
    .bind(now())
    .bind(id)
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn set_task_done(db: &Db, id: &str, result: &str, duration_ms: i64) -> AppResult<()> {
    sqlx::query(
        "UPDATE dna_tasks SET status = 'done', result_text = ?, error = NULL, duration_ms = ?, updated_at = ? WHERE id = ?",
    )
    .bind(result)
    .bind(duration_ms)
    .bind(now())
    .bind(id)
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn set_task_failed(db: &Db, id: &str, error: &str) -> AppResult<()> {
    sqlx::query("UPDATE dna_tasks SET status = 'failed', error = ?, updated_at = ? WHERE id = ?")
        .bind(error)
        .bind(now())
        .bind(id)
        .execute(&db.pool)
        .await?;
    Ok(())
}

/// 启动时中断恢复:全库 processing → pending。
pub async fn reset_processing_tasks(db: &Db) -> AppResult<u64> {
    let r = sqlx::query(
        "UPDATE dna_tasks SET status = 'pending', updated_at = ? WHERE status = 'processing'",
    )
    .bind(now())
    .execute(&db.pool)
    .await?;
    Ok(r.rows_affected())
}

/// 重跑:把指定剧的 failed(或指定 spec 的全部)任务重置为 pending。
pub async fn reset_failed_tasks(db: &Db, drama_id: &str) -> AppResult<u64> {
    let r = sqlx::query(
        "UPDATE dna_tasks SET status = 'pending', error = NULL, updated_at = ?
         WHERE drama_id = ? AND status = 'failed'",
    )
    .bind(now())
    .bind(drama_id)
    .execute(&db.pool)
    .await?;
    Ok(r.rows_affected())
}

/// 整剧全部任务重置(重新拆解)。
pub async fn reset_all_tasks(db: &Db, drama_id: &str) -> AppResult<u64> {
    let r = sqlx::query(
        "UPDATE dna_tasks SET status = 'pending', result_text = NULL, error = NULL, updated_at = ?
         WHERE drama_id = ?",
    )
    .bind(now())
    .bind(drama_id)
    .execute(&db.pool)
    .await?;
    Ok(r.rows_affected())
}

pub async fn reset_tasks_of_spec(db: &Db, drama_id: &str, spec_id: &str) -> AppResult<u64> {
    let r = sqlx::query(
        "UPDATE dna_tasks SET status = 'pending', result_text = NULL, error = NULL, updated_at = ?
         WHERE drama_id = ? AND spec_id = ?",
    )
    .bind(now())
    .bind(drama_id)
    .bind(spec_id)
    .execute(&db.pool)
    .await?;
    Ok(r.rows_affected())
}

/// 某资产在该剧的全部已完成结果(per_episode 按集号升序)。
pub async fn done_results_of_spec(
    db: &Db,
    drama_id: &str,
    spec_id: &str,
) -> AppResult<Vec<DnaTask>> {
    Ok(sqlx::query_as::<_, DnaTask>(
        "SELECT t.* FROM dna_tasks t
         LEFT JOIN episodes e ON e.id = t.episode_id
         WHERE t.drama_id = ? AND t.spec_id = ? AND t.status = 'done'
         ORDER BY e.ep_no, t.segment_no",
    )
    .bind(drama_id)
    .bind(spec_id)
    .fetch_all(&db.pool)
    .await?)
}

/// 执行日志用的任务视图(带剧名/资产名/集号)。
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DnaTaskView {
    pub id: String,
    pub drama_name: String,
    pub spec_name: String,
    pub ep_no: Option<i64>,
    pub segment_no: Option<i64>,
    pub status: String,
    pub error: Option<String>,
    pub duration_ms: Option<i64>,
    pub updated_at: String,
    pub result_chars: i64,
}

/// 跨剧最近的拆解任务(按更新时间倒序)。
pub async fn list_recent_tasks(db: &Db, limit: i64) -> AppResult<Vec<DnaTaskView>> {
    Ok(sqlx::query_as::<_, DnaTaskView>(
        "SELECT t.id, d.name AS drama_name, s.name AS spec_name, e.ep_no, t.segment_no,
                t.status, t.error, t.duration_ms, t.updated_at,
                length(ifnull(t.result_text, '')) AS result_chars
         FROM dna_tasks t
         JOIN dramas d ON d.id = t.drama_id
         JOIN asset_specs s ON s.id = t.spec_id
         LEFT JOIN episodes e ON e.id = t.episode_id
         ORDER BY t.updated_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&db.pool)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    async fn temp_db() -> (Db, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("dramadna-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Db::connect(&dir.join("t.db")).await.unwrap();
        (db, dir)
    }

    fn ep(no: i64) -> EpisodeMeta {
        EpisodeMeta {
            ep_no: no,
            title: String::new(),
            file_path: format!("/tmp/剧x/{no}.mp4"),
            duration_sec: Some(60.0),
            width: None,
            height: None,
        }
    }

    #[tokio::test]
    async fn ensure_tasks_skips_specs_with_disabled_dependency_chain() {
        let (db, dir) = temp_db().await;
        let drama = upsert_drama(&db, "剧", "/tmp/剧z", vec![ep(1)])
            .await
            .unwrap();
        let eps: Vec<String> = list_episodes(&db, &drama.id)
            .await
            .unwrap()
            .into_iter()
            .map(|e| e.id)
            .collect();
        // 停用台词原文:直接与传递依赖它的资产都不应建任务。
        sqlx::query("UPDATE asset_specs SET enabled = 0 WHERE id = 'b-transcript'")
            .execute(&db.pool)
            .await
            .unwrap();
        let specs = list_specs(&db).await.unwrap();
        ensure_tasks(&db, &drama.id, &specs, &eps, 1).await.unwrap();
        let tasks = list_tasks(&db, &drama.id).await.unwrap();
        for skipped in [
            "b-transcript",
            "b-breakdown",
            "c-annotated",
            "c-scriptback",
            "c-voice",
        ] {
            assert!(
                tasks.iter().all(|t| t.spec_id != skipped),
                "{skipped} 不应建任务"
            );
        }
        // 不依赖台词链的资产照常建任务。
        for kept in ["a-characters", "a-cinematography", "c-beatsheet"] {
            assert!(tasks.iter().any(|t| t.spec_id == kept), "{kept} 应建任务");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn ensure_all_dramas_backfills_new_specs() {
        let (db, dir) = temp_db().await;
        let _drama = upsert_drama(&db, "剧", "/tmp/剧y", vec![ep(1)])
            .await
            .unwrap();
        let n1 = ensure_tasks_all_dramas(&db).await.unwrap();
        assert!(n1 > 0);
        // 再跑一遍:全部已存在,不新建。
        assert_eq!(ensure_tasks_all_dramas(&db).await.unwrap(), 0);
        std::fs::remove_dir_all(&dir).ok();
    }
}
