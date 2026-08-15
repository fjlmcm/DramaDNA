// 与 Rust 侧 models.rs 对应的前端类型(camelCase)。

export type ProviderKind =
  | "openai_compatible"
  | "volcengine"
  | "dashscope"
  | "gemini"
  | "xiaomi";

export type VideoInputMethod = "file_api" | "base64" | "url";

export type TabKey =
  | "dramas"
  | "specs"
  | "outputs"
  | "understand"
  | "schemes"
  | "batch"
  | "logs";

export interface Provider {
  id: string;
  name: string;
  kind: ProviderKind;
  baseUrl: string;
  apiKey: string;
  extraConfig: string;
  createdAt: string;
  updatedAt: string;
}

export interface ProviderInput {
  name: string;
  kind: ProviderKind;
  baseUrl: string;
  apiKey: string;
  extraConfig?: string;
}

export interface Model {
  id: string;
  providerId: string;
  modelId: string;
  displayName: string;
  videoInputMethod: VideoInputMethod;
  constraints: string;
  params: string;
  enabled: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface ModelInput {
  providerId: string;
  modelId: string;
  displayName: string;
  videoInputMethod?: VideoInputMethod;
  constraints?: string;
  params?: string;
  enabled?: boolean;
}

export const PROVIDER_KINDS: {
  value: ProviderKind;
  label: string;
  hint: string;
}[] = [
  { value: "volcengine", label: "火山引擎 Ark", hint: "doubao 视觉系列" },
  { value: "dashscope", label: "阿里百炼 DashScope", hint: "qwen 多模态系列" },
  { value: "gemini", label: "Gemini", hint: "原生 File API 上传" },
  {
    value: "xiaomi",
    label: "小米 MiMo",
    hint: "mimo 全模态系列(api-key header 鉴权)",
  },
  {
    value: "openai_compatible",
    label: "OpenAI 兼容 / 中转站",
    hint: "通用 /v1/chat/completions 接口",
  },
];

export const VIDEO_INPUT_METHODS: {
  value: VideoInputMethod;
  label: string;
}[] = [
  { value: "file_api", label: "File API（上传后引用）" },
  { value: "base64", label: "Base64 内联" },
  { value: "url", label: "视频 URL" },
];

export interface Scheme {
  id: string;
  name: string;
  modelId: string;
  prompt: string;
  params: string;
  createdAt: string;
  updatedAt: string;
}

export interface SchemeInput {
  name: string;
  modelId: string;
  prompt: string;
  params?: string;
}

// 流式事件 —— 与 Rust provider::StreamEvent 对应。
export type StreamEvent =
  | { type: "delta"; text: string }
  | { type: "done" }
  | { type: "error"; message: string };

export interface BatchJob {
  id: string;
  name: string;
  schemeId: string;
  status: string; // pending|running|done|cancelled
  totalItems: number;
  doneItems: number;
  createdAt: string;
  updatedAt: string;
  finishedAt: string | null;
}

export interface JobItem {
  id: string;
  jobId: string;
  filePath: string;
  fileHash: string | null;
  status: string; // pending|processing|done|failed|cancelled
  preprocessedPath: string | null;
  uploadedRef: string | null;
  resultText: string | null;
  error: string | null;
  attempts: number;
  createdAt: string;
  updatedAt: string;
}

export interface Run {
  id: string;
  schemeId: string | null;
  schemeName: string;
  modelLabel: string;
  filePath: string;
  prompt: string;
  status: string; // done|failed
  resultText: string | null;
  error: string | null;
  durationMs: number | null;
  createdAt: string;
  updatedAt: string;
}

export interface ResultState {
  text: string;
  status: "running" | "done" | "error";
  error?: string;
  /** 后端取消用的运行 ID,前端生成(crypto.randomUUID),仅 running 期间有值。 */
  runId?: string;
}

/** 预处理缓存目录统计。 */
export interface CacheStats {
  totalBytes: number;
  fileCount: number;
  path: string;
}

// 视频理解 Tab 会话状态 —— 多个模型共用一个 prompt 对比。提升到 store,切 Tab 保留。
export interface UnderstandState {
  videoPath: string | null;
  prompt: string;
  modelIds: string[];
  results: Record<string, ResultState>; // key = modelId
  running: boolean;
}

export const DEFAULT_PROMPT = "请详细描述这段视频的画面内容、人物动作与场景。";

// 视频限制 —— 视频超出任一项时触发 ffmpeg 本地预处理。对应 Rust VideoConstraints。
// 时长不设限(体积由 maxBytes 兜底);分辨率/音频在 UI 以档位呈现。
export interface VideoConstraints {
  maxBytes: number;
  maxWidth: number;
  maxHeight: number;
  maxFps: number;
  audioBitrate: number; // 音频转码码率(bps),0 = 保持原音频不转码
}

export const DEFAULT_CONSTRAINTS: VideoConstraints = {
  maxBytes: 13 * 1024 * 1024,
  maxWidth: 854,
  maxHeight: 854,
  maxFps: 5,
  audioBitrate: 64000,
};

export function parseConstraints(json: string): VideoConstraints {
  try {
    const o = JSON.parse(json) as Partial<VideoConstraints>;
    return {
      maxBytes: o.maxBytes ?? DEFAULT_CONSTRAINTS.maxBytes,
      maxWidth: o.maxWidth ?? DEFAULT_CONSTRAINTS.maxWidth,
      maxHeight: o.maxHeight ?? DEFAULT_CONSTRAINTS.maxHeight,
      maxFps: o.maxFps ?? DEFAULT_CONSTRAINTS.maxFps,
      audioBitrate: o.audioBitrate ?? DEFAULT_CONSTRAINTS.audioBitrate,
    };
  } catch {
    return { ...DEFAULT_CONSTRAINTS };
  }
}

// 分辨率档位 —— longEdge 是长边上限(scale decrease 进框);"original" 不缩放。
export const RESOLUTION_TIERS: {
  value: string;
  label: string;
  longEdge: number;
}[] = [
  { value: "320p", label: "320p", longEdge: 576 },
  { value: "480p", label: "480p", longEdge: 854 },
  { value: "720p", label: "720p", longEdge: 1280 },
  { value: "original", label: "不变", longEdge: 100000 },
];

// 音频档位 —— value 即码率(bps),0 = 保持原音频。
export const AUDIO_TIERS: { value: number; label: string }[] = [
  { value: 32000, label: "32 kbps" },
  { value: 64000, label: "64 kbps" },
  { value: 128000, label: "128 kbps" },
  { value: 0, label: "不变" },
];

/** 由长边像素值反查分辨率档位标识。 */
export function resolutionTier(longEdge: number): string {
  const exact = RESOLUTION_TIERS.find((t) => t.longEdge === longEdge);
  return exact ? exact.value : "original";
}

// ────────────────────────────── DramaDNA ──────────────────────────────

export interface Drama {
  id: string;
  name: string;
  dirPath: string;
  episodeCount: number;
  totalDurationSec: number;
  meta: string;
  createdAt: string;
  updatedAt: string;
}

export interface Episode {
  id: string;
  dramaId: string;
  epNo: number;
  title: string;
  filePath: string;
  durationSec: number | null;
  width: number | null;
  height: number | null;
}

export type SpecStage = "global" | "episode" | "synth";
export type SpecScope = "per_segment" | "per_episode" | "per_drama";

export interface AssetSpec {
  id: string;
  stage: SpecStage;
  sortNo: number;
  name: string;
  scope: SpecScope;
  prompt: string;
  mergePrompt: string | null;
  modelId: string | null;
  inputs: string;
  outputTemplate: string;
  needsVideo: boolean;
  userInput: boolean;
  enabled: boolean;
  builtin: boolean;
  params: string;
  createdAt: string;
  updatedAt: string;
}

export interface SpecUpdate {
  prompt: string;
  mergePrompt: string | null;
  modelId: string | null;
  enabled: boolean;
  params: string;
}

export interface DnaTask {
  id: string;
  dramaId: string;
  specId: string;
  episodeId: string | null;
  segmentNo: number | null;
  userInput: string | null;
  status: string; // pending|processing|done|failed
  resultText: string | null;
  error: string | null;
  attempts: number;
  durationMs: number | null;
  createdAt: string;
  updatedAt: string;
}

export interface OutputFile {
  relPath: string;
  absPath: string;
  sizeBytes: number;
}

export const STAGE_LABELS: Record<SpecStage, string> = {
  global: "A · 全局资产",
  episode: "B · 分集资产",
  synth: "C · 聚合资产",
};

export const STAGE_ORDER: SpecStage[] = ["global", "episode", "synth"];

/// 执行日志的拆解任务视图。
export interface DnaTaskView {
  id: string;
  dramaName: string;
  specName: string;
  epNo: number | null;
  segmentNo: number | null;
  status: string;
  error: string | null;
  durationMs: number | null;
  updatedAt: string;
  resultChars: number;
}
