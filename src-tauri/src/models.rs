// 表结构骨架先行 —— 部分 struct 在后续阶段(批量/日志)才被引用。
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

// ────────────────────────────── 表行结构 ──────────────────────────────
// FromRow 按字段名匹配 SQL 列;serde camelCase 仅影响与前端的 JSON。

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub api_key: String,
    pub extra_config: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub id: String,
    pub provider_id: String,
    pub model_id: String,
    pub display_name: String,
    pub video_input_method: String,
    pub constraints: String,
    pub params: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Scheme {
    pub id: String,
    pub name: String,
    pub model_id: String,
    pub prompt: String,
    pub params: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct BatchJob {
    pub id: String,
    pub name: String,
    pub scheme_id: String,
    pub status: String,
    pub total_items: i64,
    pub done_items: i64,
    pub created_at: String,
    pub updated_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct JobItem {
    pub id: String,
    pub job_id: String,
    pub file_path: String,
    pub file_hash: Option<String>,
    pub status: String,
    pub preprocessed_path: Option<String>,
    pub uploaded_ref: Option<String>,
    pub result_text: Option<String>,
    pub error: Option<String>,
    pub attempts: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub scheme_id: Option<String>,
    pub scheme_name: String,
    pub model_label: String,
    pub file_path: String,
    pub prompt: String,
    pub status: String,
    pub result_text: Option<String>,
    pub error: Option<String>,
    pub duration_ms: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub id: i64,
    pub level: String,
    pub source: String,
    pub message: String,
    pub context: String,
    pub created_at: String,
}

// ────────────────────────────── 输入结构 ──────────────────────────────
// 前端提交的创建/更新载荷,不含 id 与时间戳。

fn default_json_obj() -> String {
    "{}".to_string()
}
fn default_video_method() -> String {
    "file_api".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInput {
    pub name: String,
    pub kind: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_json_obj")]
    pub extra_config: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInput {
    pub provider_id: String,
    pub model_id: String,
    pub display_name: String,
    #[serde(default = "default_video_method")]
    pub video_input_method: String,
    #[serde(default = "default_json_obj")]
    pub constraints: String,
    #[serde(default = "default_json_obj")]
    pub params: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemeInput {
    pub name: String,
    pub model_id: String,
    pub prompt: String,
    #[serde(default = "default_json_obj")]
    pub params: String,
}

/// 创建 run(执行日志)记录的内部输入 —— 非前端载荷。
#[derive(Debug)]
pub struct RunInput {
    pub model_label: String,
    pub file_path: String,
    pub prompt: String,
    pub status: String,
    pub result_text: Option<String>,
    pub error: Option<String>,
    pub duration_ms: Option<i64>,
}

// ────────────────────────────── DramaDNA 领域表 ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Drama {
    pub id: String,
    pub name: String,
    pub dir_path: String,
    pub episode_count: i64,
    pub total_duration_sec: f64,
    pub meta: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    pub id: String,
    pub drama_id: String,
    pub ep_no: i64,
    pub title: String,
    pub file_path: String,
    pub duration_sec: Option<f64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AssetSpec {
    pub id: String,
    pub stage: String,
    pub sort_no: i64,
    pub name: String,
    pub scope: String,
    pub prompt: String,
    pub merge_prompt: Option<String>,
    pub model_id: Option<String>,
    pub inputs: String,
    pub output_template: String,
    pub needs_video: bool,
    pub user_input: bool,
    pub enabled: bool,
    pub builtin: bool,
    pub params: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DnaTask {
    pub id: String,
    pub drama_id: String,
    pub spec_id: String,
    pub episode_id: Option<String>,
    pub segment_no: Option<i64>,
    pub user_input: Option<String>,
    pub status: String,
    pub result_text: Option<String>,
    pub error: Option<String>,
    pub attempts: i64,
    pub duration_ms: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}
