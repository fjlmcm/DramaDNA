// 视频本地预处理 —— 探测 / 转码 / 缓存。
//
// 不符合模型约束的视频先用 ffmpeg 缩放、降帧、转 h264/mp4,并把产物
// 按缓存键落盘复用(批量场景下避免重复转码)。
// 开发期直接用系统 PATH 中的 ffmpeg/ffprobe;阶段 9 改为随包 sidecar。

use std::path::Path;

use serde::Deserialize;
use tokio::process::Command;

use crate::error::{AppError, AppResult};

/// 视频约束 —— 任一维超出则需预处理。
#[derive(Debug, Clone)]
pub struct VideoConstraints {
    pub max_bytes: u64,
    pub max_width: u32,
    pub max_height: u32,
    pub max_fps: f64,
    /// 音频转码码率(bps);0 表示保持原音频不转码(copy)。
    pub audio_bitrate: u64,
}

impl Default for VideoConstraints {
    fn default() -> Self {
        Self {
            // base64 膨胀 4/3 后约 17.3MB,低于各家 data-uri 上限(阿里百炼最严,20MB)。
            max_bytes: 13 * 1024 * 1024,
            // 视频理解优先降分辨率/帧率 —— 模型按抽帧分析,高分辨率高帧率是浪费。
            // 854 作长边上限 ≈ 480p(横屏 854x480 / 竖屏 480x854);5fps 足够保留动作。
            max_width: 854,
            max_height: 854,
            max_fps: 5.0,
            audio_bitrate: 64_000,
        }
    }
}

impl VideoConstraints {
    /// 从 model.constraints JSON 解析,缺失字段回落到指定默认值
    /// (默认值按供应商 kind 定制,见 provider::video_constraint_defaults)。
    pub fn from_json_with(s: &str, d: Self) -> Self {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Raw {
            max_bytes: Option<u64>,
            max_width: Option<u32>,
            max_height: Option<u32>,
            max_fps: Option<f64>,
            audio_bitrate: Option<u64>,
        }
        match serde_json::from_str::<Raw>(s) {
            Ok(r) => Self {
                max_bytes: r.max_bytes.unwrap_or(d.max_bytes),
                max_width: r.max_width.unwrap_or(d.max_width),
                max_height: r.max_height.unwrap_or(d.max_height),
                max_fps: r.max_fps.unwrap_or(d.max_fps),
                audio_bitrate: r.audio_bitrate.unwrap_or(d.audio_bitrate),
            },
            Err(_) => d,
        }
    }
}

/// ffprobe 探测结果。
#[derive(Debug, Clone)]
pub struct VideoProbe {
    pub duration_s: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub size_bytes: u64,
    pub video_codec: String,
}

/// 构造一个调用 ffmpeg / ffprobe 的 Command。
/// Windows 上加 CREATE_NO_WINDOW 防止子进程弹出控制台窗口
/// —— 批量处理时每个视频会跑 probe + 可能 transcode,不加每次都闪一个 cmd 窗口。
fn ffmpeg_command(name: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(ffmpeg_bin(name));
    #[cfg(windows)]
    {
        // 0x08000000 = CREATE_NO_WINDOW(winapi)。tokio::process::Command 在 Windows
        // 上直接暴露该方法(通过 std::os::windows::process::CommandExt 内部转发)。
        cmd.creation_flags(0x08000000);
    }
    cmd
}

/// ffmpeg/ffprobe 可执行路径。按优先级解析:
/// 1) 打包后 lib.rs 设置的环境变量(随包 sidecar);
/// 2) 常见安装路径 —— macOS GUI 应用的 PATH 通常不含 Homebrew,必须显式探测;
/// 3) 兜底用裸名,交给系统 PATH。
fn ffmpeg_bin(name: &str) -> String {
    let key = if name == "ffprobe" {
        "DRAMADNA_FFPROBE"
    } else {
        "DRAMADNA_FFMPEG"
    };
    if let Ok(path) = std::env::var(key) {
        return path;
    }
    for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
        let candidate = format!("{dir}/{name}");
        if std::path::Path::new(&candidate).exists() {
            return candidate;
        }
    }
    name.to_string()
}

/// 用 ffprobe 探测视频元数据。
pub async fn probe(path: &str) -> AppResult<VideoProbe> {
    let output = ffmpeg_command("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,codec_name,avg_frame_rate",
            "-show_entries",
            "format=duration,size",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .await
        .map_err(|e| AppError::Msg(format!("ffprobe 启动失败: {e}")))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Msg(format!("ffprobe 探测失败: {err}")));
    }
    parse_probe(&String::from_utf8_lossy(&output.stdout))
}

fn parse_probe(json: &str) -> AppResult<VideoProbe> {
    let v: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| AppError::Msg(format!("ffprobe 输出解析失败: {e}")))?;
    let stream = &v["streams"][0];
    let format = &v["format"];

    Ok(VideoProbe {
        duration_s: format["duration"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0),
        width: stream["width"].as_u64().unwrap_or(0) as u32,
        height: stream["height"].as_u64().unwrap_or(0) as u32,
        fps: parse_fraction(stream["avg_frame_rate"].as_str().unwrap_or("0/1")),
        size_bytes: format["size"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        video_codec: stream["codec_name"].as_str().unwrap_or("").to_string(),
    })
}

/// 解析 "30000/1001" 形式的帧率分数。
fn parse_fraction(s: &str) -> f64 {
    let parts: Vec<&str> = s.split('/').collect();
    match parts.as_slice() {
        [num, den] => {
            let n: f64 = num.parse().unwrap_or(0.0);
            let d: f64 = den.parse().unwrap_or(1.0);
            if d == 0.0 {
                0.0
            } else {
                n / d
            }
        }
        [num] => num.parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// 视频能否直接提交给模型 —— 唯一硬约束是体积(API data-uri 上限)。
/// codec / 分辨率 / 帧率都不参与判断:三家供应商均可直接解码 hevc 等编码
/// (2026-05 实测豆包 / 通义 / Gemini 全部接受),体积达标就无需任何预处理。
pub fn is_compliant(probe: &VideoProbe, c: &VideoConstraints) -> bool {
    probe.size_bytes <= c.max_bytes
}

/// 确保视频满足约束:符合则返回原路径,否则预处理并返回缓存产物路径。
pub async fn ensure_compliant(
    path: &str,
    constraints: &VideoConstraints,
    cache_dir: &Path,
) -> AppResult<String> {
    let probe = probe(path).await?;
    log::debug!(
        "视频探测: {}x{} {:.1}s {}B codec={}",
        probe.width,
        probe.height,
        probe.duration_s,
        probe.size_bytes,
        probe.video_codec
    );
    if is_compliant(&probe, constraints) {
        log::info!("视频符合约束,跳过预处理: {path}");
        return Ok(path.to_string());
    }
    log::info!("视频超出约束,开始 ffmpeg 预处理: {path}");

    let key = cache_key(path, constraints)?;
    let out_path = cache_dir.join(format!("{key}.mp4"));
    if out_path.exists() {
        return Ok(out_path.to_string_lossy().into_owned());
    }
    std::fs::create_dir_all(cache_dir)
        .map_err(|e| AppError::Msg(format!("创建缓存目录失败: {e}")))?;

    if let Err(e) = transcode(path, &probe, constraints, &out_path).await {
        // 删除可能已生成的超标半成品,避免下次被当作缓存命中。
        let _ = std::fs::remove_file(&out_path);
        return Err(e);
    }
    log::info!("预处理完成: {}", out_path.display());
    Ok(out_path.to_string_lossy().into_owned())
}

/// 缓存键:源文件名 + 大小 + mtime + 约束摘要。
fn cache_key(path: &str, c: &VideoConstraints) -> AppResult<String> {
    let meta =
        std::fs::metadata(path).map_err(|e| AppError::Msg(format!("读取文件信息失败: {e}")))?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let name = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");
    Ok(format!(
        "{}-{}-{}-{}x{}-{}fps-{}b",
        sanitize(name),
        meta.len(),
        mtime,
        c.max_width,
        c.max_height,
        c.max_fps as u32,
        c.max_bytes
    ))
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { '_' })
        .collect()
}

/// 转码产物仍超体积上限时的最大二压轮数(首轮编码 + 二压;合计最多 1 + MAX_RETRANSCODE 次)。
const MAX_RETRANSCODE: usize = 3;
/// 视频码率绝对下限(bps)—— 极端长视频压不进 max_bytes 时给反馈修正留余地。
/// 注:videotoolbox / nvenc 等硬件编码器对 maxrate 是「软目标」,实际平均码率可能
/// 比目标高 ~40%。16kbps 配 180p/1fps 用于全剧拼接的极限压缩(全局扫描):
/// 对白剧画面近乎静止,大字号硬字幕仍可辨;动作细节会丢失。
const MIN_BITRATE: u64 = 16_000;

/// (目标码率下限 bps, 该码率下的分辨率上限 px)。码率不足以支撑高分辨率时给出
/// 更低的分辨率上限 —— 低码率配低分辨率,每像素分到的比特更多、画面更可辨。
/// 顶档上限为 u32::MAX:码率充足时不额外限制,完全沿用用户设定(720p / 原始 均生效)。
/// 阈值为经验值,可按实际效果调整。
const RESOLUTION_LADDER: [(u64, u32); 4] = [
    (700_000, u32::MAX), // 码率充足:不额外限制
    (400_000, 640),      // 360p
    (200_000, 480),      // 270p
    (0, 320),            // 180p,兜底
];

/// 首轮目标视频码率估算:按 max_bytes / 时长,扣音频码率、留 12% 容器开销。
/// 音频按「不超过总预算 1/3」计(见 fit_audio_bitrate)—— 否则 64k 默认音频
/// 会把小时级长视频的视频码率直接挤到 MIN_BITRATE 地板
/// (实测 108 分钟压 50MB:视频 34k + 音频 24k 剧情理解无损;16k+16k 则白扔一半预算)。
fn estimate_bitrate(probe: &VideoProbe, c: &VideoConstraints) -> u64 {
    if probe.duration_s > 0.5 {
        let total = (c.max_bytes as f64 * 8.0 * 0.88 / probe.duration_s) as u64;
        let audio = if c.audio_bitrate == 0 {
            0
        } else {
            c.audio_bitrate.min((total / 3).max(16_000))
        };
        total.saturating_sub(audio).max(MIN_BITRATE)
    } else {
        4_000_000
    }
}

/// 根据目标码率给出分辨率上限,再与用户设定取较小值。
/// 码率充足时上限为 u32::MAX,结果即用户设定本身 —— 不会把 720p / 原始 压低。
fn fit_dimension(bitrate: u64, user_limit: u32) -> u32 {
    let cap = RESOLUTION_LADDER
        .iter()
        .find(|(min_br, _)| bitrate >= *min_br)
        .map(|&(_, cap)| cap)
        .unwrap_or(320);
    user_limit.min(cap)
}

/// 目标码率极低时下调帧率上限(视频理解可容忍,以动作连续性换清晰度)。
/// 触底 MIN_BITRATE(极端长视频)时进一步降到 1fps —— 每帧分到更多比特,
/// 静态画面/对白剧仍可辨,动作场景会丢帧。
fn fit_fps(bitrate: u64, user_fps: f64) -> f64 {
    if bitrate <= MIN_BITRATE {
        user_fps.min(1.0)
    } else if bitrate < 250_000 {
        user_fps.min(3.0)
    } else {
        user_fps
    }
}

/// 视频码率触底 MIN_BITRATE 时(极端长视频),把 audio 降到 16kbps 给 video 腾空间。
/// 此场景下 64kbps audio 在长视频里会占走 max_bytes 大头(1MB/分钟),
/// 必须让步。16kbps 仍能保留基本对白可辨。copy 模式(user=0)触底时也强制 16k ——
/// 避免继承源高码率音频(可能数百 kbps,直接打爆 max_bytes)。
/// 非触底时音频也不超过视频码率一半(下限 16k)—— 与 estimate_bitrate 的
/// 「音频 ≤ 总预算 1/3」一致(audio = total/3 ⇔ audio = video/2)。
fn fit_audio_bitrate(video_bitrate: u64, user_audio: u64) -> u64 {
    if video_bitrate > MIN_BITRATE {
        if user_audio == 0 {
            0
        } else {
            user_audio.min((video_bitrate / 2).max(16_000))
        }
    } else if user_audio == 0 {
        16_000
    } else {
        user_audio.min(16_000)
    }
}

/// 按目标码率自动匹配分辨率/帧率/音频码率,得到本轮转码的有效约束。
/// 用户在设置中选的分辨率/帧率/音频码率视为上限,自动调整只在其下浮动。
fn fit_constraints(c: &VideoConstraints, bitrate: u64) -> VideoConstraints {
    let edge = fit_dimension(bitrate, c.max_width.max(c.max_height));
    VideoConstraints {
        max_width: c.max_width.min(edge),
        max_height: c.max_height.min(edge),
        max_fps: fit_fps(bitrate, c.max_fps),
        audio_bitrate: fit_audio_bitrate(bitrate, c.audio_bitrate),
        ..c.clone()
    }
}

/// 把视频转码到约束以内。首轮按估算码率编码,之后按产物实测大小反馈修正码率、
/// 循环二压,直到体积达标;到上限仍超标则报错(不把超标产物送给模型)。
async fn transcode(
    src: &str,
    probe: &VideoProbe,
    c: &VideoConstraints,
    out: &Path,
) -> AppResult<()> {
    let mut bitrate = estimate_bitrate(probe, c);
    // 首轮挑出一个能跑通的编码器,后续二压沿用,不再重复回退。
    let mut encoder: Option<&'static str> = None;

    for round in 0..=MAX_RETRANSCODE {
        // 按当前目标码率自动匹配分辨率/帧率,再编码。
        let eff = fit_constraints(c, bitrate);
        match encoder {
            Some(enc) => run_encode(src, probe, &eff, out, enc, bitrate).await?,
            None => {
                encoder = Some(encode_with_fallback(src, probe, &eff, out, bitrate).await?);
            }
        }

        let actual = std::fs::metadata(out).map(|m| m.len()).unwrap_or(u64::MAX);
        if actual <= c.max_bytes {
            log::info!(
                "转码达标:{actual} ≤ {} 字节(第 {} 次编码,码率 {bitrate},长边 ≤ {}px,{} fps)",
                c.max_bytes,
                round + 1,
                eff.max_width.max(eff.max_height),
                eff.max_fps
            );
            return Ok(());
        }
        if round == MAX_RETRANSCODE {
            return Err(AppError::Msg(format!(
                "二压 {MAX_RETRANSCODE} 轮后产物仍超体积上限({actual} > {} 字节);\
                 视频过长,请缩短视频或按集拆分后再试。",
                c.max_bytes
            )));
        }

        // 反馈修正:码率与产物大小近似线性,按实测比例下调并留 5% 裕度。
        let next =
            ((bitrate as f64 * c.max_bytes as f64 / actual as f64 * 0.95) as u64).max(MIN_BITRATE);
        if next >= bitrate {
            return Err(AppError::Msg(format!(
                "码率已降至下限 {bitrate} bps,产物仍超体积上限({actual} > {} 字节);\
                 视频过长,请缩短视频或按集拆分后再试。",
                c.max_bytes
            )));
        }
        log::warn!(
            "转码产物 {actual} 字节超出上限 {},第 {} 轮二压:码率 {bitrate} → {next}",
            c.max_bytes,
            round + 1
        );
        bitrate = next;
    }
    unreachable!("循环必在 round == MAX_RETRANSCODE 时返回")
}

/// 逐个尝试编码器候选,返回首个能跑通的编码器名;全部失败则报错。
async fn encode_with_fallback(
    src: &str,
    probe: &VideoProbe,
    c: &VideoConstraints,
    out: &Path,
    bitrate: u64,
) -> AppResult<&'static str> {
    let mut last_err = String::new();
    for &encoder in hw_encoder_candidates() {
        match run_encode(src, probe, c, out, encoder, bitrate).await {
            Ok(()) => {
                log::info!("转码编码器: {encoder}");
                return Ok(encoder);
            }
            Err(e) => {
                log::warn!("编码器 {encoder} 不可用,回退下一个");
                last_err = e.to_string();
            }
        }
    }
    Err(AppError::Msg(format!(
        "视频转码失败(所有编码器): {last_err}"
    )))
}

/// 按平台排列的编码器候选 —— 优先硬件加速,末位回退软件编码。
fn hw_encoder_candidates() -> &'static [&'static str] {
    #[cfg(target_os = "macos")]
    return &["h264_videotoolbox", "libx264"];
    #[cfg(target_os = "windows")]
    return &["h264_nvenc", "h264_qsv", "h264_amf", "libx264"];
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return &["libx264"];
}

/// 用指定编码器、指定目标码率跑一次转码。硬件编码器不支持 CRF,统一用码率控制。
async fn run_encode(
    src: &str,
    probe: &VideoProbe,
    c: &VideoConstraints,
    out: &Path,
    encoder: &str,
    bitrate: u64,
) -> AppResult<()> {
    // 等比缩放进约束框(不放大),再保证宽高为偶数(h264 要求)。
    let scale = format!(
        "scale='min({},iw)':'min({},ih)':force_original_aspect_ratio=decrease,scale=trunc(iw/2)*2:trunc(ih/2)*2",
        c.max_width, c.max_height
    );
    let fps = probe.fps.min(c.max_fps).max(1.0);

    let mut args: Vec<String> = vec![
        "-y".into(),
        "-i".into(),
        src.into(),
        "-vf".into(),
        scale,
        "-r".into(),
        format!("{fps:.0}"),
        "-c:v".into(),
        encoder.into(),
        "-b:v".into(),
        bitrate.to_string(),
        "-maxrate".into(),
        bitrate.to_string(),
        "-bufsize".into(),
        (bitrate * 2).to_string(),
        "-pix_fmt".into(),
        "yuv420p".into(),
    ];
    // 音频:码率为 0 则保持原音频(copy),否则转 AAC。
    if c.audio_bitrate == 0 {
        args.extend(["-c:a".into(), "copy".into()]);
    } else {
        args.extend([
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            c.audio_bitrate.to_string(),
        ]);
    }
    args.push(out.to_string_lossy().into_owned());

    let output = ffmpeg_command("ffmpeg")
        .args(&args)
        .output()
        .await
        .map_err(|e| AppError::Msg(format!("ffmpeg 启动失败: {e}")))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Msg(format!("ffmpeg({encoder}) 转码失败: {err}")));
    }
    Ok(())
}

// ────────────────────────────── tests ──────────────────────────────

#[cfg(test)]
// The concat pipeline implementation follows this focused helper-test module.
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    #[test]
    fn fraction() {
        assert!((parse_fraction("30/1") - 30.0).abs() < 0.01);
        assert!((parse_fraction("30000/1001") - 29.97).abs() < 0.1);
        assert_eq!(parse_fraction("0/0"), 0.0);
    }

    #[test]
    fn compliance() {
        let c = VideoConstraints::default();
        let good = VideoProbe {
            duration_s: 10.0,
            width: 640,
            height: 480,
            fps: 5.0,
            size_bytes: 1_000_000,
            video_codec: "h264".into(),
        };
        assert!(is_compliant(&good, &c));
        // 唯一不合规的情形:体积超上限。
        assert!(!is_compliant(
            &VideoProbe {
                size_bytes: 999_000_000,
                ..good.clone()
            },
            &c
        ));
        // codec / 分辨率 / 帧率都不参与判断 —— 体积达标即可直接提交。
        assert!(is_compliant(
            &VideoProbe {
                video_codec: "hevc".into(),
                ..good.clone()
            },
            &c
        ));
        assert!(is_compliant(
            &VideoProbe {
                width: 1920,
                height: 1080,
                ..good.clone()
            },
            &c
        ));
    }

    #[test]
    fn constraints_from_json() {
        let c =
            VideoConstraints::from_json_with(r#"{"maxWidth":640}"#, VideoConstraints::default());
        assert_eq!(c.max_width, 640);
        assert_eq!(c.max_height, 854); // 缺失回落默认
        assert_eq!(
            VideoConstraints::from_json_with("{}", VideoConstraints::default()).max_width,
            854
        );
        assert_eq!(
            VideoConstraints::from_json_with("bad", VideoConstraints::default()).max_width,
            854
        );
    }

    #[test]
    fn bitrate_estimation() {
        let c = VideoConstraints::default(); // max_bytes 13MiB, audio_bitrate 64k
        let base = VideoProbe {
            duration_s: 60.0,
            width: 854,
            height: 480,
            fps: 5.0,
            size_bytes: 0,
            video_codec: "h264".into(),
        };
        // 60s:13MiB×8×0.88/60 ≈ 1.6Mbps,扣 64kbps 音频后落在合理区间。
        let b = estimate_bitrate(&base, &c);
        assert!(b > 1_300_000 && b < 1_700_000, "码率估算异常: {b}");
        // 超长视频:估算码率被钳到下限。
        let long = VideoProbe {
            duration_s: 100_000.0,
            ..base.clone()
        };
        assert_eq!(estimate_bitrate(&long, &c), MIN_BITRATE);
        // 极短视频:走固定码率分支。
        let short = VideoProbe {
            duration_s: 0.3,
            ..base.clone()
        };
        assert_eq!(estimate_bitrate(&short, &c), 4_000_000);
    }

    #[test]
    fn resolution_auto_downscale() {
        // 码率充足 → 用满用户上限。
        assert_eq!(fit_dimension(1_000_000, 854), 854);
        // 码率不足 → 自动降档。
        assert_eq!(fit_dimension(500_000, 854), 640);
        assert_eq!(fit_dimension(250_000, 854), 480);
        assert_eq!(fit_dimension(50_000, 854), 320);
        // 始终不超过用户上限:用户只要 480p,码率再足也不放大。
        assert_eq!(fit_dimension(5_000_000, 480), 480);
        // 码率充足时用满用户设定:选 720p 即 720p,不再被压到 854。
        assert_eq!(fit_dimension(5_000_000, 1280), 1280);
    }

    #[test]
    fn fps_auto_downscale() {
        assert_eq!(fit_fps(1_000_000, 5.0), 5.0); // 码率足,保持
        assert_eq!(fit_fps(150_000, 5.0), 3.0); // 码率低,降到 3
        assert_eq!(fit_fps(150_000, 2.0), 2.0); // 用户本就 < 3,不抬高
        assert_eq!(fit_fps(MIN_BITRATE, 5.0), 1.0); // 触底:降到 1
        assert_eq!(fit_fps(MIN_BITRATE, 0.5), 0.5); // 用户本就 < 1,不抬高
    }

    #[test]
    fn audio_downscale_on_bitrate_floor() {
        // 视频码率充足(≥ 2×音频):保持用户原值。
        assert_eq!(fit_audio_bitrate(200_000, 64_000), 64_000);
        assert_eq!(fit_audio_bitrate(100_000, 0), 0); // copy 保持
                                                      // 视频码率偏低:音频钳到视频一半,不再挤占。
        assert_eq!(fit_audio_bitrate(100_000, 64_000), 50_000);
        assert_eq!(fit_audio_bitrate(37_000, 64_000), 18_500);
        // 钳制下限 16k:视频略高于地板时音频不会被压到 16k 以下。
        assert_eq!(fit_audio_bitrate(20_000, 64_000), 16_000);
        // 触底(video = MIN_BITRATE):audio 强制降到 16k。
        assert_eq!(fit_audio_bitrate(MIN_BITRATE, 64_000), 16_000);
        // 触底 + copy:强制 16k(避免继承源高码率)。
        assert_eq!(fit_audio_bitrate(MIN_BITRATE, 0), 16_000);
        // 触底但用户本就 ≤ 16k:保持。
        assert_eq!(fit_audio_bitrate(MIN_BITRATE, 8_000), 8_000);
    }

    #[test]
    fn long_video_audio_budget_split() {
        // 108 分钟压 50MB(2026-07 云雾 Gemini 实测参数):总预算 ≈56.5kbps,
        // 音频钳到 1/3,视频 ≈37kbps —— 旧逻辑先扣满 64k 音频会把视频挤到
        // 16k 地板,白扔一半体积预算。
        let c = VideoConstraints {
            max_bytes: 50 * 1024 * 1024,
            ..VideoConstraints::default()
        };
        let p = VideoProbe {
            duration_s: 6531.0,
            width: 480,
            height: 854,
            fps: 1.0,
            size_bytes: 135_000_000,
            video_codec: "h264".into(),
        };
        let v = estimate_bitrate(&p, &c);
        assert!(v > 30_000 && v < 45_000, "视频码率异常: {v}");
        let a = fit_audio_bitrate(v, 64_000);
        assert!((16_000..=v).contains(&a), "音频码率异常: {a}");
        // 视频+音频合计不超总预算(留了 12% 容器开销)。
        assert!((v + a) as f64 <= c.max_bytes as f64 * 8.0 * 0.9 / p.duration_s);
    }

    #[test]
    fn constraints_from_json_with_custom_defaults() {
        let d = VideoConstraints {
            max_bytes: 50 * 1024 * 1024,
            ..VideoConstraints::default()
        };
        // 缺失字段回落到定制默认。
        assert_eq!(
            VideoConstraints::from_json_with("{}", d.clone()).max_bytes,
            50 * 1024 * 1024
        );
        // 显式设置优先于定制默认。
        assert_eq!(
            VideoConstraints::from_json_with(r#"{"maxBytes":1000}"#, d).max_bytes,
            1000
        );
    }

    async fn gen_test_video(path: &str, codec: &str) {
        ffmpeg_command("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=3:size=640x480:rate=15",
                "-c:v",
                codec,
                "-b:v",
                "8M", // 强制高码率,产出体积可观,便于测试体积压缩
                "-pix_fmt",
                "yuv420p",
                path,
            ])
            .output()
            .await
            .expect("生成测试视频失败");
    }

    #[tokio::test]
    async fn probe_real_video() {
        let path = std::env::temp_dir().join("dramadna-probe-test.mp4");
        let p = path.to_str().unwrap();
        gen_test_video(p, "libx264").await;
        let probe = probe(p).await.unwrap();
        assert_eq!(probe.width, 640);
        assert_eq!(probe.height, 480);
        assert_eq!(probe.video_codec, "h264");
        assert!(probe.duration_s > 2.5);
    }

    #[tokio::test]
    async fn preprocess_shrinks_oversized_video() {
        let src = std::env::temp_dir().join("dramadna-pp-src.mp4");
        let srcs = src.to_str().unwrap();
        gen_test_video(srcs, "libx264").await;
        let cache = std::env::temp_dir().join("dramadna-pp-cache");
        let _ = std::fs::remove_dir_all(&cache);

        // max_bytes 设为源体积的一半,强制触发体积压缩。
        let src_size = std::fs::metadata(srcs).unwrap().len();
        let c = VideoConstraints {
            max_bytes: src_size / 2,
            ..VideoConstraints::default()
        };
        let out = ensure_compliant(srcs, &c, &cache).await.unwrap();
        assert_ne!(out, srcs); // 体积超标 → 应转码,返回新路径

        let out_probe = probe(&out).await.unwrap();
        assert!(out_probe.size_bytes <= c.max_bytes); // 压到上限内
        assert_eq!(out_probe.video_codec, "h264"); // 转码统一为 h264
    }

    /// 真实视频转码实测 —— 默认 ignore,需指定 DRAMADNA_TEST_VIDEO 环境变量。
    /// 运行:DRAMADNA_TEST_VIDEO=/path/to/video.mp4 \
    ///   cargo test --manifest-path src-tauri/Cargo.toml transcode_real -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn transcode_real_video() {
        let src = std::env::var("DRAMADNA_TEST_VIDEO").expect("需设置 DRAMADNA_TEST_VIDEO");
        let mut c = VideoConstraints::default();
        if let Ok(v) = std::env::var("DRAMADNA_TEST_MAX_BYTES") {
            c.max_bytes = v.parse().expect("DRAMADNA_TEST_MAX_BYTES 必须是数字");
        }

        let before = probe(&src).await.unwrap();
        println!(
            "\n源视频: {}x{} {:.0}fps {:.0}s {} {:.1}MB",
            before.width,
            before.height,
            before.fps,
            before.duration_s,
            before.video_codec,
            before.size_bytes as f64 / 1_048_576.0
        );
        println!(
            "约束: ≤{:.1}MB,长边 ≤{}px,≤{}fps",
            c.max_bytes as f64 / 1_048_576.0,
            c.max_width,
            c.max_fps
        );

        // 清缓存,确保实测真正走转码而非命中旧产物。
        let cache = std::env::temp_dir().join("dramadna-realtest-cache");
        let _ = std::fs::remove_dir_all(&cache);

        let t0 = std::time::Instant::now();
        let result = ensure_compliant(&src, &c, &cache).await;
        let elapsed = t0.elapsed();

        match result {
            Ok(out) if out == src => {
                println!("→ 符合约束,直接提交(未转码),耗时 {elapsed:?}");
            }
            Ok(out) => {
                let after = probe(&out).await.unwrap();
                println!(
                    "→ 转码完成,耗时 {elapsed:?}\n  产物路径: {out}\n  产物: {}x{} {:.0}fps {} {:.2}MB(上限的 {:.0}%)",
                    after.width,
                    after.height,
                    after.fps,
                    after.video_codec,
                    after.size_bytes as f64 / 1_048_576.0,
                    after.size_bytes as f64 / c.max_bytes as f64 * 100.0
                );
                assert!(after.size_bytes <= c.max_bytes, "产物仍超体积上限");
                assert_eq!(after.video_codec, "h264", "产物应为 h264");
            }
            Err(e) => println!("→ 失败(耗时 {elapsed:?}): {e}"),
        }
    }
}

// ────────────────────────────── DramaDNA:多集拼接 ──────────────────────────────

/// 把多集视频按给定顺序无损拼接为一个文件(concat demuxer + stream copy)。
/// 返回缓存产物路径;后续极限压缩交给 ensure_compliant(自适应阶梯会压到约束内)。
///
/// 前提:同一部剧的分集编码规格一致(实际短剧素材均满足)。规格不一致时
/// stream copy 产物会花屏/报错 —— 此时报错提示用户,不做静默重编码。
pub async fn concat_videos(paths: &[String], cache_dir: &Path) -> AppResult<String> {
    if paths.is_empty() {
        return Err(AppError::Msg("拼接列表为空".into()));
    }
    // 缓存键:各文件(名+大小+mtime)聚合哈希。
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for p in paths {
        p.hash(&mut hasher);
        if let Ok(meta) = std::fs::metadata(p) {
            meta.len().hash(&mut hasher);
            meta.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0)
                .hash(&mut hasher);
        }
    }
    let out_path = cache_dir.join(format!("concat-{:016x}.mp4", hasher.finish()));
    if out_path.exists() {
        return Ok(out_path.to_string_lossy().into_owned());
    }
    std::fs::create_dir_all(cache_dir)
        .map_err(|e| AppError::Msg(format!("创建缓存目录失败: {e}")))?;

    // concat demuxer 的文件列表(单引号包裹,内部单引号按 ffmpeg 规则转义)。
    let list: String = paths
        .iter()
        .map(|p| format!("file '{}'\n", p.replace('\'', r"'\''")))
        .collect();
    let list_path = cache_dir.join(format!("concat-{:016x}.txt", hasher.finish()));
    std::fs::write(&list_path, list).map_err(|e| AppError::Msg(format!("写拼接列表失败: {e}")))?;

    // tmp+rename 原子落位 —— 进程中断残留的半成品不会被下次误认为有效缓存。
    let tmp_path = out_path.with_extension("tmp.mp4");
    let output = ffmpeg_command("ffmpeg")
        .args(["-y", "-f", "concat", "-safe", "0", "-i"])
        .arg(&list_path)
        .args(["-c", "copy"])
        .arg(&tmp_path)
        .output()
        .await
        .map_err(|e| AppError::Msg(format!("ffmpeg 启动失败: {e}")))?;
    let _ = std::fs::remove_file(&list_path);
    if !output.status.success() {
        let _ = std::fs::remove_file(&tmp_path);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: String = stderr.lines().rev().take(5).collect::<Vec<_>>().join("\n");
        return Err(AppError::Msg(format!(
            "分集拼接失败(各集编码规格可能不一致): {tail}"
        )));
    }
    std::fs::rename(&tmp_path, &out_path)
        .map_err(|e| AppError::Msg(format!("缓存落位失败: {e}")))?;
    log::info!("分集拼接完成: {} 集 -> {}", paths.len(), out_path.display());
    Ok(out_path.to_string_lossy().into_owned())
}

/// 单集标准化重编码(480p/1fps,时间戳干净,按集缓存)。
///
/// 全集视频的生产路径必须逐集重编码后再拼接:直接 stream-copy 拼接原始分集,
/// 各集时间基不一致会产生错乱时间戳 —— 容器时长虚增数倍、方舟平台预处理报
/// "Invalid video_url"、faststart 重封装也无效(2026-07 实测)。
async fn normalize_episode(src: &str, cache_dir: &Path) -> AppResult<String> {
    let meta =
        std::fs::metadata(src).map_err(|e| AppError::Msg(format!("读取文件信息失败: {e}")))?;
    let name = Path::new(src)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("ep");
    let out_path = cache_dir.join(format!("{}-{}-n480.mp4", sanitize(name), meta.len()));
    if out_path.exists() {
        return Ok(out_path.to_string_lossy().into_owned());
    }
    std::fs::create_dir_all(cache_dir)
        .map_err(|e| AppError::Msg(format!("创建缓存目录失败: {e}")))?;

    // 写临时文件再原子 rename —— 防御并发/中断留下半成品被当缓存命中。
    let tmp_path = out_path.with_extension("tmp.mp4");
    for encoder in ["h264_videotoolbox", "libx264"] {
        let output = ffmpeg_command("ffmpeg")
            .args(["-y", "-i", src, "-vf", "scale=480:-2,fps=1"])
            .args([
                "-c:v", encoder, "-b:v", "280k", "-maxrate", "340k", "-bufsize", "680k",
            ])
            .args(["-c:a", "aac", "-b:a", "16000"])
            .args(["-video_track_timescale", "90000"])
            .arg(&tmp_path)
            .output()
            .await
            .map_err(|e| AppError::Msg(format!("ffmpeg 启动失败: {e}")))?;
        if output.status.success() {
            std::fs::rename(&tmp_path, &out_path)
                .map_err(|e| AppError::Msg(format!("缓存落位失败: {e}")))?;
            return Ok(out_path.to_string_lossy().into_owned());
        }
        let _ = std::fs::remove_file(&tmp_path);
        log::warn!("单集标准化编码器 {encoder} 失败,回退下一个");
    }
    Err(AppError::Msg(format!("单集标准化转码失败: {src}")))
}

/// 全集标准视频:逐集标准化重编码 → stream-copy 拼接(此时时间戳干净、安全)
/// → faststart。逐集产物按集缓存,加集/重跑均为增量。
pub async fn concat_normalized(paths: &[String], cache_dir: &Path) -> AppResult<String> {
    // 多个全局资产任务会并发进入 —— 整体串行:第一个任务完成全部逐集转码后,
    // 后续任务全部命中缓存瞬间通过;避免同一集被并发转码写坏产物。
    static GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _gate = GATE.lock().await;
    let norm_dir = cache_dir.join("norm");
    let mut normalized = Vec::with_capacity(paths.len());
    for (i, p) in paths.iter().enumerate() {
        crate::activity::set(format!(
            "全集准备:标准化转码 第 {}/{} 集…",
            i + 1,
            paths.len()
        ));
        normalized.push(normalize_episode(p, &norm_dir).await?);
    }
    crate::activity::set("全集准备:拼接分集…");
    let merged = concat_videos(&normalized, &cache_dir.join("concat")).await?;
    // concat_videos 产物 moov 在尾部;file_api 上传走流式读取,补 faststart。
    let fast = format!("{}.fast.mp4", merged.trim_end_matches(".mp4"));
    if std::path::Path::new(&fast).exists() {
        return Ok(fast);
    }
    let fast_tmp = format!("{fast}.tmp.mp4");
    let output = ffmpeg_command("ffmpeg")
        .args(["-y", "-i", &merged, "-c", "copy", "-movflags", "+faststart"])
        .arg(&fast_tmp)
        .output()
        .await
        .map_err(|e| AppError::Msg(format!("ffmpeg 启动失败: {e}")))?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&fast_tmp);
        return Err(AppError::Msg("faststart 重封装失败".into()));
    }
    std::fs::rename(&fast_tmp, &fast).map_err(|e| AppError::Msg(format!("缓存落位失败: {e}")))?;
    Ok(fast)
}
