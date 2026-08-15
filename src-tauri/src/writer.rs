// md 产出写盘 —— 拆解结果写入剧目录下「拆解/」子目录,供 AI 编剧直接消费。

use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

/// 拆解产出根目录:<剧目录>/拆解
pub fn output_root(drama_dir: &str) -> PathBuf {
    Path::new(drama_dir).join("拆解")
}

/// 按输出模板渲染文件路径。占位符:{ep} 集数(两位补零)。
pub fn render_output_path(drama_dir: &str, template: &str, ep_no: Option<i64>) -> PathBuf {
    let rel = template.replace("{ep}", &format!("{:02}", ep_no.unwrap_or(0)));
    output_root(drama_dir).join(rel)
}

/// 写产出文件(自动建目录,重跑覆盖)。
pub fn write_output(path: &Path, text: &str) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Msg(format!("创建产出目录失败: {e}")))?;
    }
    // 统一保证文末换行,便于下游拼接。
    let body = if text.ends_with('\n') {
        text.to_string()
    } else {
        format!("{text}\n")
    };
    std::fs::write(path, body).map_err(|e| AppError::Msg(format!("写产出文件失败: {e}")))?;
    Ok(())
}

// ────────────────────────────── 节奏数据(程序统计,不走模型) ──────────────────────────────

/// 每集硬指标 —— 二创剧本的容量红线数据。
pub struct PacingRow {
    pub ep_no: i64,
    pub duration_sec: f64,
    pub scene_count: usize,
    pub line_count: usize,
    pub shot_count: usize,
}

/// 台词原文的台词条数(形如「12. …」的编号行,含 [旁白]/[画面文字] 行)。
pub fn count_numbered_lines(text: &str) -> usize {
    text.lines()
        .filter(|l| {
            let t = l.trim_start();
            let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
            digits > 0 && t[digits..].starts_with('.')
        })
        .count()
}

/// 拆解卡「## 场景列表」小节内的场次数(跳过空行、括注行、表头与分隔线)。
pub fn count_scene_lines(text: &str) -> usize {
    let mut inside = false;
    let mut n = 0;
    for l in text.lines() {
        let t = l.trim();
        if t.starts_with("##") {
            inside = t.trim_start_matches('#').trim().starts_with("场景列表");
            continue;
        }
        if !inside || t.is_empty() || t.starts_with('(') || t.starts_with('(') {
            continue;
        }
        // markdown 表格的分隔线(---|---)与表头不算场次。
        if t.chars().all(|c| matches!(c, '-' | '|' | ':' | ' ')) {
            continue;
        }
        if t.contains("场景地点") && t.contains("出场人物") {
            continue;
        }
        n += 1;
    }
    n
}

/// 分镜表的镜头数(以整数镜号开头、后跟表格分隔的行)。
pub fn count_shot_rows(text: &str) -> usize {
    text.lines()
        .filter(|l| {
            let t = l.trim_start().trim_start_matches('|').trim_start();
            let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
            digits > 0 && t[digits..].trim_start().starts_with(['|', '|'])
        })
        .count()
}

/// 组装「节奏数据」md(表格供程序/下游直接消费,格式稳定)。
pub fn build_pacing_md(drama_name: &str, rows: &[PacingRow]) -> String {
    let mut md = format!(
        "# 《{drama_name}》节奏数据\n\n\
         程序统计的每集硬指标(非模型产出)。台词密度与场次数是二创剧本的容量红线:\
         新剧本单集超出此量级即装不进成片时长。\n\n\
         集 | 时长(秒) | 场次 | 台词句数 | 台词密度(句/分钟) | 镜头数 | 平均镜头(秒)\n\
         ---|---|---|---|---|---|---\n"
    );
    for r in rows {
        let per_min = if r.duration_sec > 0.0 {
            r.line_count as f64 * 60.0 / r.duration_sec
        } else {
            0.0
        };
        let avg_shot = if r.shot_count > 0 {
            r.duration_sec / r.shot_count as f64
        } else {
            0.0
        };
        md.push_str(&format!(
            "{} | {:.0} | {} | {} | {:.1} | {} | {:.1}\n",
            r.ep_no, r.duration_sec, r.scene_count, r.line_count, per_min, r.shot_count, avg_shot
        ));
    }
    let n = rows.len().max(1) as f64;
    let total_sec: f64 = rows.iter().map(|r| r.duration_sec).sum();
    let total_lines: usize = rows.iter().map(|r| r.line_count).sum();
    let total_scenes: usize = rows.iter().map(|r| r.scene_count).sum();
    let total_shots: usize = rows.iter().map(|r| r.shot_count).sum();
    let density = if total_sec > 0.0 {
        total_lines as f64 * 60.0 / total_sec
    } else {
        0.0
    };
    md.push_str(&format!(
        "\n## 全剧均值\n\n\
         - 平均每集:{:.0} 秒 / {:.1} 场 / {:.1} 句台词 / {:.1} 个镜头\n\
         - 全剧台词密度:{density:.1} 句/分钟\n",
        total_sec / n,
        total_scenes as f64 / n,
        total_lines as f64 / n,
        total_shots as f64 / n,
    ));
    md
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_episode_template() {
        let p = render_output_path("/tmp/剧", "分集/第{ep}集-台词原文.md", Some(7));
        assert_eq!(p, Path::new("/tmp/剧/拆解/分集/第07集-台词原文.md"));
    }

    #[test]
    fn renders_plain_template() {
        let p = render_output_path("/tmp/剧", "01-人物档案.md", None);
        assert_eq!(p, Path::new("/tmp/剧/拆解/01-人物档案.md"));
    }

    #[test]
    fn counts_numbered_transcript_lines() {
        let text = "1. 你以为这样就赢了?\n2. [旁白] 三年前的雨夜。\n注:以上为第1场\n10. 台词十\n";
        assert_eq!(count_numbered_lines(text), 3);
    }

    #[test]
    fn counts_scene_lines_inside_section_only() {
        let text = "## 本集剧情\n她回家了\n\n## 场景列表\n(每场一行)\n场景地点 | 出场人物 | 事件\n---|---|---\n客厅 | 刘茜 | 摊牌\n门口 | 张三 | 偷听\n\n## 本集功能\n推进主线\n";
        assert_eq!(count_scene_lines(text), 2);
    }

    #[test]
    fn counts_shot_rows_from_table() {
        let text = "镜号 | 起-止(秒) | 景别\n---|---|---\n1 | 0-3.5 | 近景\n| 2 | 3.5-6 | 特写\n\n## 本集镜头统计\n- 镜头总数 2\n";
        assert_eq!(count_shot_rows(text), 2);
    }

    #[test]
    fn builds_pacing_md_with_density() {
        let rows = vec![PacingRow {
            ep_no: 1,
            duration_sec: 60.0,
            scene_count: 4,
            line_count: 30,
            shot_count: 20,
        }];
        let md = build_pacing_md("测试剧", &rows);
        assert!(md.contains("1 | 60 | 4 | 30 | 30.0 | 20 | 3.0"));
        assert!(md.contains("全剧台词密度:30.0 句/分钟"));
    }
}
