import { create } from "zustand";
import type {
  Model,
  ModelInput,
  Provider,
  ProviderInput,
  Scheme,
  SchemeInput,
  TabKey,
  UnderstandState,
} from "@/types";
import { DEFAULT_PROMPT } from "@/types";
import { ipc } from "@/services/ipc";

interface AppState {
  // 导航
  activeTab: TabKey;
  settingsOpen: boolean;
  setTab: (t: TabKey) => void;
  openSettings: () => void;
  closeSettings: () => void;

  // 视频理解 Tab 会话状态(切 Tab 时保留)
  understand: UnderstandState;
  setUnderstand: (
    patch:
      | Partial<UnderstandState>
      | ((s: UnderstandState) => Partial<UnderstandState>),
  ) => void;

  // 数据
  providers: Provider[];
  models: Model[];
  schemes: Scheme[];
  loaded: boolean;
  loadAll: () => Promise<void>;

  // providers CRUD
  addProvider: (input: ProviderInput) => Promise<void>;
  editProvider: (id: string, input: ProviderInput) => Promise<void>;
  removeProvider: (id: string) => Promise<void>;

  // models CRUD
  addModel: (input: ModelInput) => Promise<void>;
  editModel: (id: string, input: ModelInput) => Promise<void>;
  removeModel: (id: string) => Promise<void>;

  // schemes
  addScheme: (input: SchemeInput) => Promise<Scheme>;
  editScheme: (id: string, input: SchemeInput) => Promise<void>;
  removeScheme: (id: string) => Promise<void>;
  reloadSchemes: () => Promise<void>;
}

export const useAppStore = create<AppState>((set) => ({
  activeTab: "dramas",
  settingsOpen: false,
  setTab: (t) => set({ activeTab: t }),
  openSettings: () => set({ settingsOpen: true }),
  closeSettings: () => set({ settingsOpen: false }),

  understand: {
    videoPath: null,
    prompt: DEFAULT_PROMPT,
    modelIds: [],
    results: {},
    running: false,
  },
  setUnderstand: (patch) =>
    set((s) => ({
      understand: {
        ...s.understand,
        ...(typeof patch === "function" ? patch(s.understand) : patch),
      },
    })),

  providers: [],
  models: [],
  schemes: [],
  loaded: false,
  loadAll: async () => {
    const [providers, models, schemes] = await Promise.all([
      ipc.listProviders(),
      ipc.listModels(),
      ipc.listSchemes(),
    ]);
    set({ providers, models, schemes, loaded: true });
  },

  addProvider: async (input) => {
    await ipc.createProvider(input);
    set({ providers: await ipc.listProviders() });
  },
  editProvider: async (id, input) => {
    await ipc.updateProvider(id, input);
    set({ providers: await ipc.listProviders() });
  },
  removeProvider: async (id) => {
    await ipc.deleteProvider(id);
    // 删供应商会级联删除其模型,两者都要刷新。
    const [providers, models] = await Promise.all([
      ipc.listProviders(),
      ipc.listModels(),
    ]);
    set({ providers, models });
  },

  addModel: async (input) => {
    await ipc.createModel(input);
    set({ models: await ipc.listModels() });
  },
  editModel: async (id, input) => {
    await ipc.updateModel(id, input);
    set({ models: await ipc.listModels() });
  },
  removeModel: async (id) => {
    await ipc.deleteModel(id);
    set({ models: await ipc.listModels() });
  },

  addScheme: async (input) => {
    const scheme = await ipc.createScheme(input);
    set({ schemes: await ipc.listSchemes() });
    return scheme;
  },
  editScheme: async (id, input) => {
    await ipc.updateScheme(id, input);
    set({ schemes: await ipc.listSchemes() });
  },
  removeScheme: async (id) => {
    await ipc.deleteScheme(id);
    set({ schemes: await ipc.listSchemes() });
  },
  reloadSchemes: async () => {
    set({ schemes: await ipc.listSchemes() });
  },
}));
