// 真素材端到端 spike —— 需要真实 key、网络与本地素材,默认 #[ignore]。
//
// 运行（使用自己的测试密钥与素材）:
//   VOLC_KEY=... DNA_DIR="/path/to/example-drama" \
//   cargo test --manifest-path src-tauri/Cargo.toml dna_spike -- --include-ignored --nocapture

#![cfg(test)]

use crate::ffmpeg::{self, VideoConstraints};
use crate::models::{Model, Provider};
use crate::prompts::{BUILTIN_SPECS, CONTEXT_HEADER};
use crate::provider::complete;

fn drama_dir() -> String {
    std::env::var("DNA_DIR").expect("DNA_DIR 未设置(剧目录)")
}

fn episode_files(n: usize) -> Vec<String> {
    let mut files: Vec<(i64, String)> = std::fs::read_dir(drama_dir())
        .expect("读剧目录失败")
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension()?.to_str()? != "mp4" {
                return None;
            }
            let stem = p.file_stem()?.to_str()?.to_string();
            let (ep, _) = crate::drama::parse_episode_filename(&stem)?;
            Some((ep, p.to_string_lossy().to_string()))
        })
        .collect();
    files.sort();
    files.into_iter().take(n).map(|(_, p)| p).collect()
}

fn volc() -> (Provider, Model) {
    let key = std::env::var("VOLC_KEY").expect("VOLC_KEY 未设置");
    (
        Provider {
            id: "spike".into(),
            name: "火山".into(),
            kind: "volcengine".into(),
            base_url: "https://ark.cn-beijing.volces.com/api/v3".into(),
            api_key: key,
            extra_config: "{}".into(),
            created_at: String::new(),
            updated_at: String::new(),
        },
        Model {
            id: "spike".into(),
            provider_id: "spike".into(),
            model_id: "doubao-seed-2-0-lite-260428".into(),
            display_name: "豆包".into(),
            video_input_method: "base64".into(),
            constraints: r#"{"maxBytes":45088768}"#.into(),
            params: "{}".into(),
            enabled: true,
            created_at: String::new(),
            updated_at: String::new(),
        },
    )
}

fn spec_prompt(id: &str) -> (&'static str, serde_json::Value) {
    let s = BUILTIN_SPECS.iter().find(|s| s.id == id).unwrap();
    (s.prompt, s.params.parse().unwrap())
}

fn fill(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut t = format!("{CONTEXT_HEADER}{template}");
    for (k, v) in pairs {
        t = t.replace(k, v);
    }
    t
}

/// 风险点 1:B1 台词逐字转录 —— 单集原片直接提交。
#[tokio::test]
#[ignore = "需要真实 key 与素材"]
async fn dna_spike_b1_transcript() {
    let (provider, model) = volc();
    let ep1 = episode_files(1).pop().expect("没有第 1 集");
    let c = VideoConstraints::from_json_with(&model.constraints, VideoConstraints::default());
    let cache = std::env::temp_dir().join("dna-spike-cache");
    let ready = ffmpeg::ensure_compliant(&ep1, &c, &cache).await.unwrap();

    let (tpl, params) = spec_prompt("b-transcript");
    let prompt = fill(
        tpl,
        &[
            ("{drama_name}", "示例短剧"),
            ("{episode_count}", "57"),
            ("{ep_no}", "1"),
        ],
    );
    let t = std::time::Instant::now();
    let out = complete(&provider, &model, &prompt, Some(&ready), &params)
        .await
        .expect("B1 调用失败");
    println!("\n── B1 台词原文(第1集,耗时 {:?})──\n{out}\n", t.elapsed());
    assert!(out.len() > 100, "台词输出过短");
}

/// 风险点 2:极限压缩 —— 10 集拼接压进豆包上限后,人物档案还拆得动吗。
#[tokio::test]
#[ignore = "需要真实 key 与素材"]
async fn dna_spike_a1_segment() {
    let (provider, model) = volc();
    let paths = episode_files(10);
    assert!(paths.len() >= 10, "素材不足 10 集");
    let cache = std::env::temp_dir().join("dna-spike-cache");

    let t0 = std::time::Instant::now();
    let merged = ffmpeg::concat_videos(&paths, &cache.join("concat"))
        .await
        .expect("拼接失败");
    let c = VideoConstraints::from_json_with(&model.constraints, VideoConstraints::default());
    let ready = ffmpeg::ensure_compliant(&merged, &c, &cache).await.unwrap();
    let probe = ffmpeg::probe(&ready).await.unwrap();
    println!(
        "\n拼接+压缩耗时 {:?} —— 产物 {}x{} {:.1}fps {:.1}min {:.1}MB",
        t0.elapsed(),
        probe.width,
        probe.height,
        probe.fps,
        probe.duration_s / 60.0,
        probe.size_bytes as f64 / 1048576.0
    );

    let (tpl, params) = spec_prompt("a-characters");
    let prompt = fill(
        tpl,
        &[
            ("{drama_name}", "示例短剧"),
            ("{episode_count}", "57"),
            ("{ep_range}", "1-10"),
            ("{segment_no}", "1"),
            ("{segment_count}", "6"),
        ],
    );
    let t = std::time::Instant::now();
    let out = complete(&provider, &model, &prompt, Some(&ready), &params)
        .await
        .expect("A1 调用失败");
    println!("\n── A1 人物档案(段1,耗时 {:?})──\n{out}\n", t.elapsed());
    assert!(out.contains("##"), "人物档案缺少结构");
}

/// 风险点 3:全集极限压缩。实测结论(2026-07):豆包对视频时长硬限 2h30m,
/// 本剧全集 3h14m 超限(体积 11.65MB 不是瓶颈)→ 管线按时长自动分段(默认 ≤140min)。
/// 本测试验证半剧段(29 集,约 97 分钟)—— 自动分段后的实际单段规格。
#[tokio::test]
#[ignore = "需要真实 key 与素材"]
async fn dna_spike_a1_full_series() {
    let (provider, model) = volc();
    let paths = episode_files(29);
    println!("\n全集拼接: {} 集", paths.len());
    let cache = std::env::temp_dir().join("dna-spike-cache");

    let t0 = std::time::Instant::now();
    let merged = ffmpeg::concat_videos(&paths, &cache.join("concat"))
        .await
        .expect("拼接失败");
    let c = VideoConstraints::from_json_with(&model.constraints, VideoConstraints::default());
    let ready = ffmpeg::ensure_compliant(&merged, &c, &cache)
        .await
        .expect("极限压缩失败");
    let probe = ffmpeg::probe(&ready).await.unwrap();
    println!(
        "拼接+压缩耗时 {:?} —— 产物 {}x{} {:.2}fps {:.1}min {:.2}MB",
        t0.elapsed(),
        probe.width,
        probe.height,
        probe.fps,
        probe.duration_s / 60.0,
        probe.size_bytes as f64 / 1048576.0
    );

    let (tpl, params) = spec_prompt("a-characters");
    let n = paths.len().to_string();
    let prompt = fill(
        tpl,
        &[
            ("{drama_name}", "示例短剧"),
            ("{episode_count}", &n),
            ("{ep_range}", &format!("1-{n}")),
            ("{segment_no}", "1"),
            ("{segment_count}", "1"),
        ],
    );
    let t = std::time::Instant::now();
    let out = complete(&provider, &model, &prompt, Some(&ready), &params)
        .await
        .expect("全集 A1 调用失败");
    println!("\n── A1 人物档案(全集,耗时 {:?})──\n{out}\n", t.elapsed());
}

/// 全流程监督运行:真实素材 + 真实 key + 完整管线(A/B/C 全任务)。
/// 运行:
///   VOLC_KEY=.. ALI_KEY=.. DNA_DIR=.. DNA_CACHE=.. \
///   cargo test dna_full_pipeline -- --include-ignored --nocapture
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "需要真实 key、素材与较长运行时间"]
async fn dna_full_pipeline_run() {
    let tmp = std::env::temp_dir().join("dramadna-fullrun");
    let _ = std::fs::create_dir_all(&tmp);
    let db = crate::db::Db::connect(&tmp.join("run.db"))
        .await
        .expect("建库失败");

    // 注入真实 key 与管线设置。
    for (id, env) in [("seed-volc", "VOLC_KEY"), ("seed-ali", "ALI_KEY")] {
        let key = std::env::var(env).unwrap_or_default();
        sqlx::query("UPDATE providers SET api_key = ? WHERE id = ?")
            .bind(&key)
            .bind(id)
            .execute(&db.pool)
            .await
            .unwrap();
    }
    for (k, v) in [
        ("dna_video_model", "seed-volc-pro"),
        ("dna_text_model", "seed-ali-m"),
        ("dna_concurrency", "4"),
    ] {
        sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)")
            .bind(k)
            .bind(v)
            .execute(&db.pool)
            .await
            .unwrap();
    }

    // 导入 → 全管线。缓存目录可用 DNA_CACHE 复用既有逐集转码产物。
    let drama = crate::drama::import_drama_dir(&db, &drama_dir())
        .await
        .expect("导入失败");
    println!("导入: {} —— {} 集", drama.name, drama.episode_count);
    let cache = std::env::var("DNA_CACHE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| tmp.join("cache"));
    let t0 = std::time::Instant::now();
    crate::pipeline::run_pipeline_with(db.clone(), drama.id.clone(), cache)
        .await
        .expect("管线运行失败");

    // 统计。
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT spec_id, status, COUNT(*) FROM dna_tasks WHERE drama_id = ? GROUP BY spec_id, status ORDER BY spec_id",
    )
    .bind(&drama.id)
    .fetch_all(&db.pool)
    .await
    .unwrap();
    println!(
        "\n══ 管线完成,耗时 {:.1} 分钟 ══",
        t0.elapsed().as_secs_f64() / 60.0
    );
    for (spec, status, n) in &rows {
        println!("  {spec:<14} {status:<10} {n}");
    }
    let failed: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT spec_id, error FROM dna_tasks WHERE drama_id = ? AND status = 'failed' LIMIT 10",
    )
    .bind(&drama.id)
    .fetch_all(&db.pool)
    .await
    .unwrap();
    for (spec, err) in &failed {
        println!(
            "  ✗ {spec}: {}",
            err.as_deref()
                .unwrap_or("")
                .chars()
                .take(150)
                .collect::<String>()
        );
    }
}
