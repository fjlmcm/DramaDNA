// 剧目导入 —— 扫描"一部剧一个目录",解析文件名集数与标题,probe 视频规格。

use std::path::Path;

use futures_util::stream::{self, StreamExt};

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::ffmpeg;
use crate::models::Drama;
use crate::repo_dna::{self, EpisodeMeta};

/// 同时 probe 的文件数 —— 导入是低频操作,适度并发即可。
const PROBE_CONCURRENCY: usize = 8;

const VIDEO_EXTS: &[&str] = &["mp4", "mov", "mkv", "webm", "avi"];

/// 从文件名解析 (集数, 标题钩子文案)。
/// 支持:"第01集_标题 #标签.mp4" / "01_标题.mp4" / "01.mp4" / "EP01.mp4"。
pub fn parse_episode_filename(stem: &str) -> Option<(i64, String)> {
    // 主模式:第NN集
    let ep_no = if let Some(pos) = stem.find('第') {
        let rest = &stem[pos + '第'.len_utf8()..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        let after = &rest[digits.len()..];
        if !digits.is_empty() && after.starts_with('集') {
            digits.parse::<i64>().ok()
        } else {
            None
        }
    } else {
        None
    };
    // 兜底:数字前缀(允许 EP/ep 前缀)
    let ep_no = ep_no.or_else(|| {
        let s = stem
            .strip_prefix("EP")
            .or_else(|| stem.strip_prefix("ep"))
            .unwrap_or(stem);
        let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            None
        } else {
            digits.parse::<i64>().ok()
        }
    })?;

    // 标题:第一个 _ 之后,截到第一个 # 标签,trim。
    let title = stem
        .split_once('_')
        .map(|(_, t)| t)
        .unwrap_or("")
        .split('#')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    Some((ep_no, title))
}

/// 导入(或重扫)剧目目录:扫描视频文件 → 解析集数 → 并发 probe → 入库。
pub async fn import_drama_dir(db: &Db, dir_path: &str) -> AppResult<Drama> {
    let dir = Path::new(dir_path);
    if !dir.is_dir() {
        return Err(AppError::Msg(format!("不是目录: {dir_path}")));
    }
    let name = dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| dir_path.to_string());

    let mut parsed: Vec<(i64, String, String)> = Vec::new(); // (ep_no, title, path)
    let mut skipped: Vec<String> = Vec::new();
    let entries =
        std::fs::read_dir(dir).map_err(|e| AppError::Msg(format!("读取目录失败: {e}")))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let is_video = path
            .extension()
            .map(|e| VIDEO_EXTS.contains(&e.to_string_lossy().to_lowercase().as_str()))
            .unwrap_or(false);
        if !path.is_file() || !is_video {
            continue;
        }
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        match parse_episode_filename(&stem) {
            Some((ep_no, title)) => parsed.push((ep_no, title, path.to_string_lossy().to_string())),
            None => skipped.push(stem),
        }
    }
    if parsed.is_empty() {
        return Err(AppError::Msg(format!(
            "目录中没有可解析集数的视频文件(跳过 {} 个)",
            skipped.len()
        )));
    }
    if !skipped.is_empty() {
        log::warn!(
            "导入 {name}: {} 个文件无法解析集数,已跳过: {skipped:?}",
            skipped.len()
        );
    }
    parsed.sort_by_key(|(ep_no, _, _)| *ep_no);

    // 并发 probe 视频规格;probe 失败不阻断导入(时长置空)。
    let episodes: Vec<EpisodeMeta> = stream::iter(parsed)
        .map(|(ep_no, title, file_path)| async move {
            let probe = ffmpeg::probe(&file_path).await;
            if let Err(e) = &probe {
                log::warn!("probe 失败({file_path}): {e}");
            }
            EpisodeMeta {
                ep_no,
                title,
                file_path,
                duration_sec: probe.as_ref().ok().map(|p| p.duration_s),
                width: probe.as_ref().ok().map(|p| p.width as i64),
                height: probe.as_ref().ok().map(|p| p.height as i64),
            }
        })
        .buffered(PROBE_CONCURRENCY)
        .collect()
        .await;

    let drama = repo_dna::upsert_drama(db, &name, dir_path, episodes).await?;
    log::info!(
        "剧目导入完成: {} —— {} 集,总时长 {:.1} 分钟",
        drama.name,
        drama.episode_count,
        drama.total_duration_sec / 60.0
    );
    Ok(drama)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_chinese_filename() {
        let (no, title) = parse_episode_filename("第01集_一次意外揭开隐藏身份 #示例短剧").unwrap();
        assert_eq!(no, 1);
        assert_eq!(title, "一次意外揭开隐藏身份");
    }

    #[test]
    fn parses_two_digit_episode() {
        let (no, title) = parse_episode_filename("第57集_大结局 #标签").unwrap();
        assert_eq!(no, 57);
        assert_eq!(title, "大结局");
    }

    #[test]
    fn falls_back_to_numeric_prefix() {
        assert_eq!(parse_episode_filename("01").unwrap().0, 1);
        assert_eq!(parse_episode_filename("EP12").unwrap().0, 12);
        let (no, title) = parse_episode_filename("03_复仇开始").unwrap();
        assert_eq!(no, 3);
        assert_eq!(title, "复仇开始");
    }

    #[test]
    fn rejects_unparseable_names() {
        assert!(parse_episode_filename("花絮").is_none());
        assert!(parse_episode_filename("预告片_精彩").is_none());
    }

    #[test]
    fn title_without_hashtags_kept_whole() {
        let (_, title) = parse_episode_filename("第05集_势利闺蜜百般刁难").unwrap();
        assert_eq!(title, "势利闺蜜百般刁难");
    }
}
