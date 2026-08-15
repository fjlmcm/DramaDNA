// 视频理解的供应商适配器。
//
// spike 结论(2026-05-22):火山 Ark / 阿里百炼 / 云雾中转 三家都是
// OpenAI 兼容的 /chat/completions + base64 data URL。唯一差异是视频
// content part 的 type 字段:火山/百炼用 video_url,Gemini(云雾)用 image_url。
// 因此用一个统一客户端,由 ProviderKind 决定 part type。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use base64::Engine;
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::{json, Value};
use tauri::ipc::Channel;
use tokio::sync::oneshot;

use crate::error::{AppError, AppResult};
use crate::models::{Model, Provider};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(180);

/// 流式事件 —— 经 Tauri Channel 推送给前端。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StreamEvent {
    Delta { text: String },
    Done,
    Error { message: String },
}

/// reqwest 错误的完整原因链 —— Display 只有外层("error sending request"),
/// 超时/连接重置/DNS 等真实原因在 source 链里,排查必需。
fn error_chain(e: &dyn std::error::Error) -> String {
    let mut s = e.to_string();
    let mut cur = e.source();
    while let Some(src) = cur {
        s.push_str(&format!(" ← {src}"));
        cur = src.source();
    }
    s
}

/// 视频 content part 的 type 字段 —— 按供应商类型区分。
fn video_part_type(kind: &str) -> &'static str {
    match kind {
        "gemini" => "image_url",
        // volcengine / dashscope / openai_compatible
        _ => "video_url",
    }
}

/// 按供应商 kind 给出视频约束默认值(model.constraints JSON 里显式设置的字段优先)。
/// 体积上限为 2026-07 实测值:
/// - gemini(云雾中转):网关硬顶请求体 128MB(报错原文 "request body exceeds 128 MB"),
///   base64 膨胀 4/3 → 原始视频 50MB 时请求体约 67MB,余量充足。
///   注意另有约 1 小时的时长墙(60min 过、90min 拒),与体积/模型/media_resolution/音频均无关,
///   体积压不掉时长 —— 超长剧全集任务应绑定豆包 file_api 模型。
/// - xiaomi(MiMo):base64 硬顶 50MB(报错原文 "max: 50MB")→ 原始 ≤37.5MB,留余量取 36MB。
/// - 其余维持 13MB 默认(阿里百炼 data-uri 20MB 最严)。
pub fn video_constraint_defaults(kind: &str) -> crate::ffmpeg::VideoConstraints {
    let d = crate::ffmpeg::VideoConstraints::default();
    match kind {
        "gemini" => crate::ffmpeg::VideoConstraints {
            max_bytes: 50 * 1024 * 1024,
            ..d
        },
        "xiaomi" => crate::ffmpeg::VideoConstraints {
            max_bytes: 36 * 1024 * 1024,
            ..d
        },
        _ => d,
    }
}

/// 给 reqwest RequestBuilder 附加各家鉴权 —— 多数 OpenAI 兼容厂商用 Bearer Authorization,
/// 小米 MiMo 例外用自定义 `api-key` header(spike 实测 2026-05)。
trait AuthExt {
    fn with_provider_auth(self, kind: &str, api_key: &str) -> Self;
}

impl AuthExt for reqwest::RequestBuilder {
    fn with_provider_auth(self, kind: &str, api_key: &str) -> Self {
        if kind == "xiaomi" {
            self.header("api-key", api_key)
        } else {
            self.bearer_auth(api_key)
        }
    }
}

/// 进行中的视频理解流式调用的取消令牌注册表 —— key = 前端生成的 run_id。
fn cancel_registry() -> &'static Mutex<HashMap<String, oneshot::Sender<()>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, oneshot::Sender<()>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register_cancel(run_id: &str) -> oneshot::Receiver<()> {
    let (tx, rx) = oneshot::channel();
    cancel_registry()
        .lock()
        .unwrap()
        .insert(run_id.to_string(), tx);
    rx
}

fn unregister_cancel(run_id: &str) {
    cancel_registry().lock().unwrap().remove(run_id);
}

/// 发取消信号到对应 run。返回 true 表示信号已发送,false 表示 run_id 不存在
/// (可能已结束,或前端 run_id 错误)。
pub fn cancel_run(run_id: &str) -> bool {
    if let Some(sender) = cancel_registry().lock().unwrap().remove(run_id) {
        let _ = sender.send(());
        true
    } else {
        false
    }
}

/// 按文件扩展名猜测 MIME(data URL 用)。
fn guess_mime(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".mov") {
        "video/quicktime"
    } else if lower.ends_with(".webm") {
        "video/webm"
    } else if lower.ends_with(".avi") {
        "video/x-msvideo"
    } else if lower.ends_with(".mkv") {
        "video/x-matroska"
    } else {
        "video/mp4"
    }
}

fn chat_endpoint(base_url: &str) -> String {
    format!("{}/chat/completions", base_url.trim_end_matches('/'))
}

/// 读取本地视频并编码为 data URL。
fn video_data_url(path: &str) -> AppResult<String> {
    let bytes =
        std::fs::read(path).map_err(|e| AppError::Msg(format!("读取视频失败 ({path}): {e}")))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{};base64,{}", guess_mime(path), b64))
}

/// 构造 OpenAI 兼容 chat 请求体。
fn build_body(
    model_id: &str,
    prompt: &str,
    data_url: &str,
    part_type: &str,
    stream: bool,
) -> Value {
    json!({
        "model": model_id,
        "stream": stream,
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": prompt },
                { "type": part_type, part_type: { "url": data_url } }
            ]
        }]
    })
}

fn http_client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| AppError::Msg(format!("HTTP 客户端创建失败: {e}")))
}

/// 非流式:一次性返回完整文本结果。
pub async fn understand_video(
    provider: &Provider,
    model: &Model,
    prompt: &str,
    video_path: &str,
) -> AppResult<String> {
    let data_url = video_data_url(video_path)?;
    let part_type = video_part_type(&provider.kind);
    let body = build_body(&model.model_id, prompt, &data_url, part_type, false);

    let resp = http_client()?
        .post(chat_endpoint(&provider.base_url))
        .with_provider_auth(&provider.kind, &provider.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Msg(format!("请求失败: {}", error_chain(&e))))?;

    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| AppError::Msg(format!("读取响应失败: {e}")))?;
    if !status.is_success() {
        return Err(AppError::Msg(format!("API 返回 {status}: {text}")));
    }
    parse_content(&text)
}

/// 流式:逐增量经 Channel 推送,同时返回累积的完整文本(供写入执行日志)。
/// run_id 由前端生成,用于支持中途取消 —— 外部调用 cancel_run(run_id) 即发取消信号。
pub async fn understand_video_stream(
    provider: &Provider,
    model: &Model,
    prompt: &str,
    video_path: &str,
    run_id: &str,
    channel: &Channel<StreamEvent>,
) -> AppResult<String> {
    log::info!(
        "视频理解(流式 run={run_id}): {} / {} ← {}",
        provider.name,
        model.model_id,
        video_path
    );
    let mut cancel_rx = register_cancel(run_id);

    let data_url = match video_data_url(video_path) {
        Ok(u) => u,
        Err(e) => {
            unregister_cancel(run_id);
            return Err(e);
        }
    };
    let part_type = video_part_type(&provider.kind);
    let body = build_body(&model.model_id, prompt, &data_url, part_type, true);

    let resp = match http_client()?
        .post(chat_endpoint(&provider.base_url))
        .with_provider_auth(&provider.kind, &provider.api_key)
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            unregister_cancel(run_id);
            return Err(AppError::Msg(format!("请求失败: {}", error_chain(&e))));
        }
    };

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        unregister_cancel(run_id);
        return Err(AppError::Msg(format!("API 返回 {status}: {text}")));
    }

    // 逐 chunk 累积字节,按 \n 切完整 SSE 行(\n 是 ASCII,不会切断多字节字符)。
    // select! 同时监听 cancel,任一就绪先返回。
    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    let mut accum = String::new();
    loop {
        tokio::select! {
            _ = &mut cancel_rx => {
                // cancel_rx fire 时 sender 已由 cancel_run 移除,无需再 unregister。
                log::info!("视频理解被取消: {run_id}");
                return Err(AppError::Cancelled);
            }
            chunk = stream.next() => {
                let Some(chunk) = chunk else { break };
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        unregister_cancel(run_id);
                        return Err(AppError::Msg(format!("流读取失败: {}", error_chain(&e))));
                    }
                };
                buf.extend_from_slice(&chunk);
                while let Some(nl) = buf.iter().position(|&b| b == b'\n') {
                    let line: Vec<u8> = buf.drain(..=nl).collect();
                    let line = String::from_utf8_lossy(&line);
                    if let Some(delta) = extract_delta(line.trim()) {
                        if !delta.is_empty() {
                            accum.push_str(&delta);
                            let _ = channel.send(StreamEvent::Delta { text: delta });
                        }
                    }
                }
            }
        }
    }
    unregister_cancel(run_id);
    let _ = channel.send(StreamEvent::Done);
    Ok(accum)
}

/// 从一行 SSE 中提取 delta 文本(非 data 行 / [DONE] / 无 content 返回 None)。
fn extract_delta(line: &str) -> Option<String> {
    let data = line.strip_prefix("data:")?.trim();
    if data.is_empty() || data == "[DONE]" {
        return None;
    }
    let v: Value = serde_json::from_str(data).ok()?;
    v["choices"][0]["delta"]["content"]
        .as_str()
        .map(|s| s.to_string())
}

/// 从 OpenAI 兼容响应中提取 choices[0].message.content。
/// finish_reason=length 视为失败 —— 截断产出宁可重试也不落盘。
fn parse_content(raw: &str) -> AppResult<String> {
    let v: Value = serde_json::from_str(raw)
        .map_err(|e| AppError::Msg(format!("响应解析失败: {e} — 原文: {raw}")))?;
    if v["choices"][0]["finish_reason"].as_str() == Some("length") {
        return Err(AppError::Msg(
            "输出被 max_tokens 截断(finish_reason=length),请提高模型参数里的输出预算".into(),
        ));
    }
    match v["choices"][0]["message"]["content"].as_str() {
        Some(s) if !s.is_empty() => Ok(s.to_string()),
        _ => Err(AppError::Msg(format!("响应缺少 content 字段: {raw}"))),
    }
}

// ────────────────────────────── tests ──────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_openai_response() {
        let raw = r#"{"choices":[{"message":{"content":"你好世界"}}]}"#;
        assert_eq!(parse_content(raw).unwrap(), "你好世界");
    }

    #[test]
    fn parse_missing_content_errors() {
        let raw = r#"{"choices":[{"message":{}}]}"#;
        assert!(parse_content(raw).is_err());
    }

    #[test]
    fn part_type_by_kind() {
        assert_eq!(video_part_type("gemini"), "image_url");
        assert_eq!(video_part_type("volcengine"), "video_url");
        assert_eq!(video_part_type("dashscope"), "video_url");
    }

    #[test]
    fn constraint_defaults_by_kind() {
        // 2026-07 实测:云雾网关请求体 128MB → gemini 原始视频取 50MB;
        // MiMo base64 硬顶 50MB → 原始取 36MB;其余维持 13MB 通用默认。
        assert_eq!(
            video_constraint_defaults("gemini").max_bytes,
            50 * 1024 * 1024
        );
        assert_eq!(
            video_constraint_defaults("xiaomi").max_bytes,
            36 * 1024 * 1024
        );
        assert_eq!(
            video_constraint_defaults("dashscope").max_bytes,
            13 * 1024 * 1024
        );
        assert_eq!(
            video_constraint_defaults("volcengine").max_bytes,
            13 * 1024 * 1024
        );
        // 其余维度不随 kind 变化。
        assert_eq!(video_constraint_defaults("gemini").max_fps, 5.0);
    }

    #[test]
    fn sse_delta_extraction() {
        let line = r#"data: {"choices":[{"delta":{"content":"片段"}}]}"#;
        assert_eq!(extract_delta(line), Some("片段".to_string()));
        assert_eq!(extract_delta("data: [DONE]"), None);
        assert_eq!(extract_delta(": keep-alive"), None);
        assert_eq!(extract_delta(""), None);
        let no_content = r#"data: {"choices":[{"delta":{}}]}"#;
        assert_eq!(extract_delta(no_content), None);
    }

    // ── 真实 API spike(需要 key 与网络,手动跑) ──
    // 运行: VOLC_KEY=.. ALI_KEY=.. YUN_KEY=.. cargo test -- --include-ignored --nocapture

    fn mk_provider(kind: &str, base_url: &str, key: &str) -> Provider {
        Provider {
            id: "spike".into(),
            name: "spike".into(),
            kind: kind.into(),
            base_url: base_url.into(),
            api_key: key.into(),
            extra_config: "{}".into(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn mk_model(model_id: &str) -> Model {
        Model {
            id: "spike".into(),
            provider_id: "spike".into(),
            model_id: model_id.into(),
            display_name: model_id.into(),
            video_input_method: "base64".into(),
            constraints: "{}".into(),
            params: "{}".into(),
            enabled: true,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    async fn run_spike(kind: &str, base_url: &str, key: &str, model_id: &str) {
        let video = std::env::var("DRAMADNA_SPIKE_VIDEO")
            .unwrap_or_else(|_| "/tmp/dramadna-test.mp4".to_string());
        let provider = mk_provider(kind, base_url, key);
        let model = mk_model(model_id);
        match understand_video(&provider, &model, "用一句话描述这段视频画面", &video).await
        {
            Ok(out) => {
                println!("[{kind}] ✓ 接受 — {out}");
                assert!(!out.is_empty());
            }
            Err(e) => {
                println!("[{kind}] ✗ 拒绝 — {e}");
                panic!("[{kind}] understand_video 失败");
            }
        }
    }

    #[tokio::test]
    #[ignore = "需要真实 API key 与网络"]
    async fn spike_volcengine() {
        let key = std::env::var("VOLC_KEY").expect("VOLC_KEY");
        run_spike(
            "volcengine",
            "https://ark.cn-beijing.volces.com/api/v3",
            &key,
            "doubao-seed-2-0-lite-260428",
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "需要真实 API key 与网络"]
    async fn spike_dashscope() {
        let key = std::env::var("ALI_KEY").expect("ALI_KEY");
        run_spike(
            "dashscope",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            &key,
            "qwen3.6-plus",
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "需要真实 API key 与网络"]
    async fn spike_gemini() {
        let key = std::env::var("YUN_KEY").expect("YUN_KEY");
        run_spike(
            "gemini",
            "https://yunwu.ai/v1",
            &key,
            "gemini-3.1-flash-lite",
        )
        .await;
    }

    /// 端到端实测:真实视频走完整链路(ffmpeg 预处理 → 提交三家 API)。
    /// 运行:DRAMADNA_TEST_VIDEO=/path VOLC_KEY=.. ALI_KEY=.. YUN_KEY=.. \
    ///   cargo test --manifest-path src-tauri/Cargo.toml e2e_real_video -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "需要真实 API key 与网络"]
    async fn e2e_real_video() {
        let video = std::env::var("DRAMADNA_TEST_VIDEO").expect("DRAMADNA_TEST_VIDEO");
        let c = crate::ffmpeg::VideoConstraints::default();
        let cache = std::env::temp_dir().join("dramadna-e2e-cache");
        let _ = std::fs::remove_dir_all(&cache);

        let t0 = std::time::Instant::now();
        let ready = crate::ffmpeg::ensure_compliant(&video, &c, &cache)
            .await
            .expect("预处理失败");
        let transcoded = ready != video;
        println!(
            "\n预处理{} 耗时 {:?}",
            if transcoded {
                "(已转码)"
            } else {
                "(直接提交)"
            },
            t0.elapsed()
        );

        let targets = [
            (
                "volcengine",
                "https://ark.cn-beijing.volces.com/api/v3",
                "VOLC_KEY",
                "doubao-seed-2-0-lite-260428",
            ),
            (
                "dashscope",
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
                "ALI_KEY",
                "qwen3.6-plus",
            ),
            (
                "gemini",
                "https://yunwu.ai/v1",
                "YUN_KEY",
                "gemini-3.1-flash-lite",
            ),
        ];
        for (kind, base_url, key_env, model_id) in targets {
            let Ok(key) = std::env::var(key_env) else {
                println!("[{kind}] 跳过(未设 {key_env})");
                continue;
            };
            let provider = mk_provider(kind, base_url, &key);
            let model = mk_model(model_id);
            let t = std::time::Instant::now();
            match understand_video(
                &provider,
                &model,
                "这段视频讲了什么?用三到五句话概括剧情与人物冲突。",
                &ready,
            )
            .await
            {
                Ok(out) => println!("[{kind}] ✓ ({:?})\n{out}\n", t.elapsed()),
                Err(e) => println!("[{kind}] ✗ ({:?}) {e}\n", t.elapsed()),
            }
        }
    }
}

// ────────────────────────────── DramaDNA:管线调用 ──────────────────────────────

/// 管线任务的响应超时 —— 长上下文 + 32k 级输出 + reasoning 思考,单次生成
/// 可达十几分钟(节拍表实测 6 分钟+),给足余量;正确与完整优先于快速失败。
const PIPELINE_TIMEOUT: Duration = Duration::from_secs(1800);

/// 管线统一调用:可带视频(A/B 阶段)或纯文本(C/D 阶段),
/// 并把 extra_params(如 max_tokens)合并进请求体顶层。
pub async fn complete(
    provider: &Provider,
    model: &Model,
    prompt: &str,
    video_path: Option<&str>,
    extra_params: &Value,
) -> AppResult<String> {
    let content = match video_path {
        Some(path) => {
            let data_url = video_data_url(path)?;
            let part_type = video_part_type(&provider.kind);
            json!([
                { "type": "text", "text": prompt },
                { "type": part_type, part_type: { "url": data_url } }
            ])
        }
        None => json!(prompt),
    };
    let mut body = json!({
        "model": model.model_id,
        "stream": false,
        "messages": [{ "role": "user", "content": content }]
    });
    // 合并参数:model.params 为底,extra_params(资产级)覆盖。
    for source in [
        &model.params.parse::<Value>().unwrap_or(json!({})),
        extra_params,
    ] {
        if let Some(map) = source.as_object() {
            for (k, v) in map {
                body[k] = v.clone();
            }
        }
    }

    let client = reqwest::Client::builder()
        .timeout(PIPELINE_TIMEOUT)
        .build()
        .map_err(|e| AppError::Msg(format!("HTTP 客户端创建失败: {e}")))?;
    let resp = client
        .post(chat_endpoint(&provider.base_url))
        .with_provider_auth(&provider.kind, &provider.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Msg(format!("请求失败: {}", error_chain(&e))))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| AppError::Msg(format!("读取响应失败: {e}")))?;
    if !status.is_success() {
        return Err(AppError::Msg(format!("API 返回 {status}: {text}")));
    }
    parse_content(&text)
}

// ──────────────────────── DramaDNA:豆包 Files API 路径 ────────────────────────
//
// 全剧拼接走此路径:Files API 默认存储支持 512MB,上传后经 Responses API 以
// file_id 引用。平台按 fps 抽帧且上限 1280 帧(超长视频自动均匀抽取),
// 没有 base64 路径的 2h30m 时长限制。model.video_input_method == "file_api" 时启用。

/// 上传超时:512MB 上限文件在慢速上行下可能耗时很久。
const UPLOAD_TIMEOUT: Duration = Duration::from_secs(3600);
/// 上传后平台预处理(抽帧)的轮询间隔与最长等待。
const FILE_POLL_INTERVAL: Duration = Duration::from_secs(3);
const FILE_POLL_MAX: Duration = Duration::from_secs(900);

/// 上传视频到方舟 Files API,等预处理完成,返回 file_id。
/// fps:平台抽帧率 —— 单集任务用高值(5,保台词密度);全剧/段任务用 1
/// (超长视频反正被 1280 帧上限钳制为约 9 秒一帧)。
/// max_video_tokens:全剧段传 200_000 —— 把单帧 token 预算从 64 提到 ~156,
/// 字幕可读性显著提升(实测人名/时间点定位准确);0 = 用平台默认(81920)。
pub async fn ark_upload_file(
    provider: &Provider,
    video_path: &str,
    fps: f64,
    max_video_tokens: u64,
) -> AppResult<String> {
    let base = provider.base_url.trim_end_matches('/');
    let bytes = std::fs::read(video_path)
        .map_err(|e| AppError::Msg(format!("读取视频失败 ({video_path}): {e}")))?;
    let size_mb = bytes.len() as f64 / 1048576.0;
    let name = std::path::Path::new(video_path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "video.mp4".into());
    log::info!("Files API 上传开始: {name} ({size_mb:.1}MB)");
    crate::activity::set(format!("上传 {name}({size_mb:.0}MB)…"));

    let mut form = reqwest::multipart::Form::new()
        .text("purpose", "user_data")
        .text("preprocess_configs[video][fps]", format!("{fps}"));
    if max_video_tokens > 0 {
        form = form.text(
            "preprocess_configs[video][max_video_tokens]",
            max_video_tokens.to_string(),
        );
    }
    let form = form.part(
        "file",
        reqwest::multipart::Part::bytes(bytes)
            .file_name(name)
            .mime_str(guess_mime(video_path))
            .map_err(|e| AppError::Msg(format!("MIME 错误: {e}")))?,
    );

    let client = reqwest::Client::builder()
        .timeout(UPLOAD_TIMEOUT)
        .build()
        .map_err(|e| AppError::Msg(format!("HTTP 客户端创建失败: {e}")))?;
    let resp = client
        .post(format!("{base}/files"))
        .bearer_auth(&provider.api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| AppError::Msg(format!("文件上传失败: {}", error_chain(&e))))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AppError::Msg(format!("Files API 返回 {status}: {text}")));
    }
    let v: Value = serde_json::from_str(&text)
        .map_err(|e| AppError::Msg(format!("Files API 响应解析失败: {e} — {text}")))?;
    let file_id = v["id"]
        .as_str()
        .ok_or_else(|| AppError::Msg(format!("Files API 响应缺少 id: {text}")))?
        .to_string();
    log::info!("Files API 上传完成: {file_id}");
    Ok(file_id)
}

/// 等待平台预处理完成。与上传解耦:任务超时重试时凭缓存的 file_id 继续等,
/// 不会重复上传排队(大量文件并发时平台预处理队列可达十几分钟)。
async fn ark_wait_processed(provider: &Provider, file_id: &str) -> AppResult<()> {
    // 已确认就绪的文件直接放行,省一次查询。
    {
        let done = processed_files().lock().unwrap();
        if done.contains(file_id) {
            return Ok(());
        }
    }
    crate::activity::set("平台预处理视频中(抽帧)…");
    let base = provider.base_url.trim_end_matches('/');
    let client = http_client()?;
    let deadline = std::time::Instant::now() + FILE_POLL_MAX;
    loop {
        let resp = client
            .get(format!("{base}/files/{file_id}"))
            .bearer_auth(&provider.api_key)
            .send()
            .await
            .map_err(|e| AppError::Msg(format!("查询文件状态失败: {}", error_chain(&e))))?;
        let v: Value = resp
            .json()
            .await
            .map_err(|e| AppError::Msg(format!("文件状态解析失败: {e}")))?;
        match v["status"].as_str().unwrap_or("") {
            "processed" | "succeeded" | "success" | "active" => {
                log::info!("文件预处理完成: {file_id}");
                processed_files()
                    .lock()
                    .unwrap()
                    .insert(file_id.to_string());
                return Ok(());
            }
            "error" | "failed" => {
                return Err(AppError::Msg(format!("平台文件预处理失败: {v}")));
            }
            _ => {
                if std::time::Instant::now() > deadline {
                    return Err(AppError::Msg(format!(
                        "文件预处理等待超时(file_id={file_id},重试将继续等待,不会重新上传)"
                    )));
                }
                tokio::time::sleep(FILE_POLL_INTERVAL).await;
            }
        }
    }
}

fn processed_files() -> &'static Mutex<std::collections::HashSet<String>> {
    static DONE: OnceLock<Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    DONE.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

/// 经 Responses API 以 file_id 引用视频完成一次调用。
/// prev_id:前缀缓存的 response_id —— 有值时以 previous_response_id 引用视频前缀
/// (确定性命中缓存价、可并发、各任务上下文互不可见),input 只发本任务 prompt;
/// 引用失败(缓存过期等)自动降级为普通全量调用。
pub async fn complete_file_api(
    provider: &Provider,
    model: &Model,
    prompt: &str,
    file_id: &str,
    extra_params: &Value,
    prev_id: Option<&str>,
) -> AppResult<String> {
    let base = provider.base_url.trim_end_matches('/');
    let mut body = match prev_id {
        Some(pid) => json!({
            "model": model.model_id,
            "previous_response_id": pid,
            "input": [{
                "role": "user",
                "content": [{ "type": "input_text", "text": prompt }]
            }]
        }),
        None => json!({
            "model": model.model_id,
            "input": [{
                "role": "user",
                "content": [
                    { "type": "input_video", "file_id": file_id },
                    { "type": "input_text", "text": prompt }
                ]
            }]
        }),
    };
    for source in [
        &model.params.parse::<Value>().unwrap_or(json!({})),
        extra_params,
    ] {
        merge_responses_params(&mut body, source);
    }

    let client = reqwest::Client::builder()
        .timeout(PIPELINE_TIMEOUT)
        .build()
        .map_err(|e| AppError::Msg(format!("HTTP 客户端创建失败: {e}")))?;
    let resp = client
        .post(format!("{base}/responses"))
        .bearer_auth(&provider.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Msg(format!("请求失败: {}", error_chain(&e))))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        // 前缀缓存引用失效(过期/被删):降级为普通全量调用。
        if prev_id.is_some() {
            log::warn!("前缀缓存引用失败({status}),降级为普通调用: {text}");
            return Box::pin(complete_file_api(
                provider,
                model,
                prompt,
                file_id,
                extra_params,
                None,
            ))
            .await;
        }
        return Err(AppError::Msg(format!(
            "Responses API 返回 {status}: {text}"
        )));
    }
    parse_responses_output(&text)
}

/// 前缀缓存预热:视频 file_id + 固定短指令建立前缀缓存(实测 3 秒、零输出、近乎免费),
/// 返回 response_id 供全局资产任务以 previous_response_id 并发引用。
/// 注意 caching.prefix 与 max_output_tokens 互斥(实测 400)。
/// 失败(缓存服务未开通等)返回 None,调用方降级为普通调用。
pub async fn ark_prefix_warm(
    db: &crate::db::Db,
    provider: &Provider,
    model: &Model,
    file_id: &str,
) -> Option<String> {
    let skey = format!("arkprefix:{file_id}@{}", model.model_id);
    // 45 分钟内复用(缓存默认有效期未知,过期引用会在调用侧自动降级重预热)。
    if let Ok(Some(v)) = crate::repo::get_setting(db, &skey).await {
        if let Ok(j) = v.parse::<Value>() {
            let fresh = j["at"]
                .as_str()
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                .map(|t| chrono::Utc::now().signed_duration_since(t).num_minutes() < 45)
                .unwrap_or(false);
            if fresh {
                if let Some(id) = j["id"].as_str() {
                    return Some(id.to_string());
                }
            }
        }
    }
    let body = json!({
        "model": model.model_id,
        "input": [{
            "role": "user",
            "content": [
                { "type": "input_video", "file_id": file_id },
                { "type": "input_text", "text": "收到视频后仅回复:OK" }
            ]
        }],
        "caching": { "type": "enabled", "prefix": true },
        "store": true
    });
    let base = provider.base_url.trim_end_matches('/');
    let client = reqwest::Client::builder()
        .timeout(PIPELINE_TIMEOUT)
        .build()
        .ok()?;
    let resp = client
        .post(format!("{base}/responses"))
        .bearer_auth(&provider.api_key)
        .json(&body)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        log::warn!(
            "前缀缓存预热失败(将用普通调用): {}",
            resp.text()
                .await
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect::<String>()
        );
        return None;
    }
    let v: Value = resp.json().await.ok()?;
    let id = v["id"].as_str()?.to_string();
    log::info!("前缀缓存预热完成: {id}");
    let _ = crate::repo::set_setting(
        db,
        &skey,
        &serde_json::json!({"id": &id, "at": chrono::Utc::now().to_rfc3339()}).to_string(),
    )
    .await;
    Some(id)
}

/// 从 Responses API 输出提取正文(output[].content[] 中 type=output_text 的 text)。
fn parse_responses_output(raw: &str) -> AppResult<String> {
    let v: Value = serde_json::from_str(raw)
        .map_err(|e| AppError::Msg(format!("Responses 解析失败: {e} — {raw}")))?;
    if v["status"].as_str() == Some("incomplete") {
        return Err(AppError::Msg(
            "输出未完成(status=incomplete,多为输出预算不足或思考耗尽预算),请提高模型输出预算"
                .into(),
        ));
    }
    let mut out = String::new();
    if let Some(items) = v["output"].as_array() {
        for item in items {
            if let Some(parts) = item["content"].as_array() {
                for p in parts {
                    if p["type"].as_str() == Some("output_text") {
                        out.push_str(p["text"].as_str().unwrap_or(""));
                    }
                }
            }
        }
    }
    if out.is_empty() {
        return Err(AppError::Msg(format!(
            "Responses 输出缺少 output_text: {raw}"
        )));
    }
    Ok(out)
}

/// 把 Chat API 风格参数合并进 Responses API 请求体 —— spec.params 用统一写法两条 API 通吃:
/// max_tokens → max_output_tokens,reasoning_effort → reasoning.effort,其余原样透传。
fn merge_responses_params(body: &mut Value, source: &Value) {
    if let Some(map) = source.as_object() {
        for (k, v) in map {
            match k.as_str() {
                "max_tokens" => body["max_output_tokens"] = v.clone(),
                "reasoning_effort" => body["reasoning"] = json!({ "effort": v }),
                _ => body[k.as_str()] = v.clone(),
            }
        }
    }
}

#[cfg(test)]
mod file_api_tests {
    use super::*;

    #[test]
    fn maps_chat_params_to_responses_fields() {
        let mut body = json!({});
        merge_responses_params(
            &mut body,
            &json!({"max_tokens": 16000, "reasoning_effort": "low", "temperature": 0.7}),
        );
        assert_eq!(body["max_output_tokens"], 16000);
        assert_eq!(body["reasoning"]["effort"], "low");
        assert_eq!(body["temperature"], 0.7);
        assert!(body.get("max_tokens").is_none());
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn parses_responses_output_text() {
        let raw = r#"{"output":[{"type":"reasoning","summary":[]},{"type":"message","content":[{"type":"output_text","text":"人物档案正文"}]}]}"#;
        assert_eq!(parse_responses_output(raw).unwrap(), "人物档案正文");
    }

    #[test]
    fn responses_output_missing_text_errors() {
        assert!(parse_responses_output(r#"{"output":[]}"#).is_err());
    }
}

/// file_id 进程内缓存(视频路径+参数 → 单飞 OnceCell):同一文件只上传一次,
/// 不同文件完全并行(全局锁只保护 map 的短暂读写,不覆盖上传过程)。
/// 方舟文件默认存 7 天,应用重启后重新上传,避免引用过期文件。
type FileCell = Arc<tokio::sync::OnceCell<String>>;
fn ark_file_cache() -> &'static Mutex<HashMap<String, FileCell>> {
    static CACHE: OnceLock<Mutex<HashMap<String, FileCell>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 取缓存的 file_id,没有则上传。同一(文件,参数)并发首传单飞;
/// file_id 持久化到 settings 表,6 天内跨重启复用(方舟文件默认存 7 天,留 1 天余量),
/// 上传失败不缓存,下次调用自动重试。
pub async fn ark_file_for(
    db: &crate::db::Db,
    provider: &Provider,
    video_path: &str,
    fps: f64,
    max_video_tokens: u64,
) -> AppResult<String> {
    let meta_len = std::fs::metadata(video_path).map(|m| m.len()).unwrap_or(0);
    let key = format!("{video_path}@{meta_len}@{fps}@{max_video_tokens}");
    let cell: FileCell = {
        let mut cache = ark_file_cache().lock().unwrap();
        cache.entry(key.clone()).or_default().clone()
    };
    let skey_outer = format!("arkfile:{key}");
    let id = cell
        .get_or_try_init(|| async {
            // 先查持久缓存(跨重启复用,免重复上传大文件排队)。
            let skey = format!("arkfile:{key}");
            if let Ok(Some(v)) = crate::repo::get_setting(db, &skey).await {
                if let Ok(j) = v.parse::<Value>() {
                    let fresh = j["at"]
                        .as_str()
                        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                        .map(|t| chrono::Utc::now().signed_duration_since(t).num_days() < 6)
                        .unwrap_or(false);
                    if fresh {
                        if let Some(id) = j["id"].as_str() {
                            log::info!("file_id 持久缓存命中: {id}");
                            return Ok(id.to_string());
                        }
                    }
                }
            }
            let id = ark_upload_file(provider, video_path, fps, max_video_tokens).await?;
            let _ = crate::repo::set_setting(
                db,
                &skey,
                &serde_json::json!({"id": &id, "at": chrono::Utc::now().to_rfc3339()}).to_string(),
            )
            .await;
            Ok::<String, AppError>(id)
        })
        .await?
        .clone();
    if let Err(e) = ark_wait_processed(provider, &id).await {
        // 文件在平台侧失效(过期/预处理失败):清掉两级缓存,任务重试时重新上传,
        // 避免坏 file_id 造成的永久失败循环。
        ark_file_cache().lock().unwrap().remove(&key);
        let _ = crate::repo::set_setting(db, &skey_outer, "").await;
        return Err(e);
    }
    Ok(id)
}
