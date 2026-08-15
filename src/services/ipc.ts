import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  BatchJob,
  CacheStats,
  JobItem,
  Model,
  ModelInput,
  Provider,
  ProviderInput,
  Run,
  Scheme,
  SchemeInput,
  StreamEvent,
} from "@/types";

// Tauri command 的统一封装层。所有后端调用都经过这里。
export const ipc = {
  listProviders: () => invoke<Provider[]>("list_providers"),
  createProvider: (input: ProviderInput) =>
    invoke<Provider>("create_provider", { input }),
  updateProvider: (id: string, input: ProviderInput) =>
    invoke<Provider>("update_provider", { id, input }),
  deleteProvider: (id: string) => invoke<void>("delete_provider", { id }),

  listModels: () => invoke<Model[]>("list_models"),
  createModel: (input: ModelInput) => invoke<Model>("create_model", { input }),
  updateModel: (id: string, input: ModelInput) =>
    invoke<Model>("update_model", { id, input }),
  deleteModel: (id: string) => invoke<void>("delete_model", { id }),

  listSchemes: () => invoke<Scheme[]>("list_schemes"),
  createScheme: (input: SchemeInput) =>
    invoke<Scheme>("create_scheme", { input }),
  updateScheme: (id: string, input: SchemeInput) =>
    invoke<Scheme>("update_scheme", { id, input }),
  deleteScheme: (id: string) => invoke<void>("delete_scheme", { id }),

  listBatchJobs: () => invoke<BatchJob[]>("list_batch_jobs"),
  createBatchJob: (name: string, schemeId: string, filePaths: string[]) =>
    invoke<BatchJob>("create_batch_job", { name, schemeId, filePaths }),
  runBatchJob: (jobId: string) => invoke<void>("run_batch_job", { jobId }),
  cancelBatchJob: (jobId: string) =>
    invoke<void>("cancel_batch_job", { jobId }),
  deleteBatchJob: (jobId: string) =>
    invoke<void>("delete_batch_job", { jobId }),
  listJobItems: (jobId: string) =>
    invoke<JobItem[]>("list_job_items", { jobId }),
  exportJobResults: (jobId: string, outPath: string) =>
    invoke<void>("export_job_results", { jobId, outPath }),

  listRuns: () => invoke<Run[]>("list_runs"),
  clearRuns: () => invoke<void>("clear_runs"),
  readDebugLog: () => invoke<string>("read_debug_log"),
  debugLogPath: () => invoke<string>("debug_log_path"),
  clearDebugLog: () => invoke<void>("clear_debug_log"),

  getSetting: (key: string) => invoke<string | null>("get_setting", { key }),
  setSetting: (key: string, value: string) =>
    invoke<void>("set_setting", { key, value }),

  cacheStats: () => invoke<CacheStats>("cache_stats"),
  clearCache: () => invoke<number>("clear_cache"),

  /** 非流式视频理解 —— 一次性返回完整结果。 */
  understandVideo: (modelId: string, prompt: string, videoPath: string) =>
    invoke<string>("understand_video", { modelId, prompt, videoPath }),

  /** 流式视频理解 —— 增量经回调推送;runId 由调用方生成(crypto.randomUUID),用于取消。 */
  understandVideoStream: (
    modelId: string,
    prompt: string,
    videoPath: string,
    runId: string,
    onEvent: (e: StreamEvent) => void,
  ): Promise<void> => {
    const channel = new Channel<StreamEvent>();
    channel.onmessage = onEvent;
    return invoke("understand_video_stream", {
      modelId,
      prompt,
      videoPath,
      runId,
      onEvent: channel,
    });
  },

  /** 取消某个正在跑的视频理解。返回 true 表示信号已发送。 */
  cancelUnderstandVideo: (runId: string) =>
    invoke<boolean>("cancel_understand_video", { runId }),
};

// ────────────────────────────── DramaDNA ──────────────────────────────

import type {
  AssetSpec,
  DnaTask,
  DnaTaskView,
  Drama,
  Episode,
  OutputFile,
  SpecUpdate,
} from "@/types";

export const dna = {
  importDrama: (dirPath: string) => invoke<Drama>("import_drama", { dirPath }),
  listDramas: () => invoke<Drama[]>("list_dramas"),
  deleteDrama: (id: string) => invoke<void>("delete_drama", { id }),
  listEpisodes: (dramaId: string) =>
    invoke<Episode[]>("list_drama_episodes", { dramaId }),

  listSpecs: () => invoke<AssetSpec[]>("list_asset_specs"),
  updateSpec: (id: string, update: SpecUpdate) =>
    invoke<AssetSpec>("update_asset_spec", { id, update }),
  resetSpec: (id: string) => invoke<AssetSpec>("reset_asset_spec", { id }),

  runPipeline: (dramaId: string) =>
    invoke<void>("run_dna_pipeline", { dramaId }),
  cancelPipeline: (dramaId: string) =>
    invoke<boolean>("cancel_dna_pipeline", { dramaId }),
  pipelineRunning: (dramaId: string) =>
    invoke<boolean>("dna_pipeline_running", { dramaId }),
  pipelineActivity: () => invoke<string>("dna_pipeline_activity"),
  listTasks: (dramaId: string) =>
    invoke<DnaTask[]>("list_dna_tasks", { dramaId }),
  retryFailed: (dramaId: string) =>
    invoke<number>("retry_failed_tasks", { dramaId }),
  resetDramaTasks: (dramaId: string) =>
    invoke<number>("reset_drama_tasks", { dramaId }),
  rerunSpec: (dramaId: string, specId: string) =>
    invoke<number>("rerun_spec", { dramaId, specId }),

  activity: () =>
    invoke<{ text: string; ageSecs: number }>("dna_activity"),
  listRecentTasks: (limit?: number) =>
    invoke<DnaTaskView[]>("list_recent_dna_tasks", { limit }),

  listOutputs: (dramaId: string) =>
    invoke<OutputFile[]>("list_outputs", { dramaId }),
  readOutput: (dramaId: string, relPath: string) =>
    invoke<string>("read_output", { dramaId, relPath }),
  outputDir: (dramaId: string) => invoke<string>("output_dir", { dramaId }),
};
