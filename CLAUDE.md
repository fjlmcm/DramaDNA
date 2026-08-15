# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

DramaDNA 是短剧逆向拆解桌面客户端:导入"一部剧一个目录"的竖屏短剧素材,
经三阶段管线拆解为给 AI 编剧二创消费的 md 素材包(改编方向由下游自行分析,应用内不做二创生成)。
技术栈:Tauri 2 + Rust 后端 + React 18 前端,SQLite 持久化,摩卡(Mocha)暖色主题。
五个 Tab:剧目拆解 / 资产配置 / 产出浏览 / 视频理解(调试) / 执行日志。

**核心概念 —— 每次模型调用只产出一类资产:**
- `asset_specs` 定义"一件事"(阶段 A global/B episode/C synth、作用域 per_segment/per_episode/per_drama、
  prompt、绑定模型、依赖 inputs、输出文件模板)。内置 16 个资产见 `src-tauri/src/prompts.rs`,
  种子由 `db.rs seed_asset_specs` 幂等写入(INSERT OR IGNORE,不覆盖用户修改;
  已下线的内置 spec 记录在 `RETIRED_BUILTIN_SPECS`,种子阶段连同其任务一并删除 ——
  2026-07 移除了 D 阶段二创链)。叙事层之外含视觉层(视觉设定/视听语言/分镜表/逆向剧本/语言风格)。
  「11-节奏数据.md」是管线完成时程序统计生成(writer.rs 纯函数),不占 spec。
- `dna_tasks` 是任务单元(剧 × 资产 × 集|段|合并),最小恢复单元,状态机 pending→processing→done/failed。
- 管线(`pipeline.rs`)按波次调度:每轮取出依赖已满足的 pending 任务并发执行(依赖以资产"最终产出"为准;
  per_segment 资产 = 各段拼接极限压缩提取 + segment_no=0 的合并任务)。B1 台词转录不标说话人,
  说话人由 C 阶段"台词标注"资产拿全局人物档案+拆解卡场景信息在文本层反推。
- 产出 md 写入 `<剧目录>/拆解/`(writer.rs),同时落库(可增量重跑单个资产)。
- 真素材 spike:`dna_spike.rs`(#[ignore],需 VOLC_KEY 与 DNA_DIR 环境变量)。
- **四家模型全集能力实测(2026-07,55 分钟 57 集竖屏短剧)**:
  - 豆包 seed-2-1-pro(Files API,file_id):最优。逐集重编码 480p/1fps 拼接(~80MB)直传,
    fps=1 + max_video_tokens=200000,输入 ~111k tokens,输出 23k 不劣化。
    注意:平台解析不了 stream-copy 拼接产物(错乱时间戳报 Invalid video_url),必须逐集重编码。
    Chat API base64 路径限时长 2h30m;Files API 无此限制。
  - 通义 qwen3.6-plus(base64):200p 压缩版(data-uri ≤20MB)可行,82s,理解准确,适合交叉验证。
  - 云雾 gemini(base64,part type=image_url):网关请求体硬顶 128MB(报错原文
    "request body exceeds 128 MB");**时长墙约 1 小时**(60min 过 / 90·108min 拒
    "invalid argument"),与体积/模型/音频无关,media_resolution 四种形态与原生
    generateContent 端点均无法解锁 —— 超长剧全集只能走豆包 file_api。计费按低分辨率
    (91 token/秒 = 70/帧 + 音频)。60 分钟全集压 50MB 内(视频 34k+音频 24k 单声道)
    理解无损;非流式 2 万 token 长输出可完成,偶发 RemoteDisconnected 靠任务重试消化。
  - 小米 mimo-v2.5(base64,api-key header):base64 硬顶 50MB(原始 ≤37MB,报错清晰)、
    帧数上限 2048(fps 0.1-10 设在 video_url 对象内,60min 用 0.3);29-134s 响应极快,
    音频 token≈6.25/秒;正文 ~2.6k token 自动收敛,不适合超长逐集输出;服从"不编造"指令,
    适合交叉验证与摘要类资产。
  - base64 提交的体积默认值按 provider kind 定制:`provider::video_constraint_defaults`
    (gemini 50MB / xiaomi 36MB / 其余 13MB),model.constraints JSON 显式字段优先;
    长视频音频码率钳到总预算 1/3(ffmpeg.rs),避免 64k 音频把视频挤到 16k 地板。
  - 交叉验证经验:多家对同一全集的剧情理解互相印证可揪出幻觉——时间戳错乱的输入曾让
    豆包编造出相反结局;修复后四家叙述一致。

## 常用命令

```bash
pnpm install                 # 安装前端依赖
pnpm tauri dev               # 开发模式(自动起 vite + 编译 Rust + 开窗口)
pnpm build                   # 仅前端:tsc 类型检查 + vite 构建
pnpm bundle                  # 完整打包(macOS .dmg / Windows .exe),不改版本号
pnpm release                 # 自增 patch 版本号 → 打包 → 自动 git commit 版本号变更

# Rust 测试(注意 --manifest-path,Cargo 工程在 src-tauri/)
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml <test_name>     # 跑单个测试

# 真实 API spike 测试(默认 #[ignore],需 key 与网络)
VOLC_KEY=.. ALI_KEY=.. YUN_KEY=.. \
  cargo test --manifest-path src-tauri/Cargo.toml -- --include-ignored --nocapture

# 填充示例供应商/模型骨架(不写入 api_key)
cargo run --manifest-path src-tauri/Cargo.toml --example seed
```

打包细节(ffmpeg sidecar 准备、macOS 签名、Windows 构建)见 `BUILD.md`。

## 架构

**前后端边界 —— Rust 做全部重活,前端纯 UI。** 所有 API 调用、ffmpeg、SQLite、文件 IO 都在 Rust。前端经 `invoke`(命令)与 `Channel`(流式事件)和 Rust 通信,**不直接发网络请求**。布局:前端在根 `src/`,Tauri 工程在 `src-tauri/`。

**Provider 适配器(`src-tauri/src/provider.rs`)是核心设计。** 三家供应商(火山引擎 Ark / 阿里百炼 / 云雾中转 Gemini)经实测均为 OpenAI 兼容 `/chat/completions` + base64 data URL,**唯一差异是视频 content part 的 `type` 字段**:火山/百炼用 `video_url`,Gemini(`kind=gemini`)用 `image_url`。因此是单一统一客户端,由 `ProviderKind` 决定 part type —— 新增供应商通常无需写代码,在设置页配置即可。

**视频处理链路:** 选视频 → `ffmpeg.rs` 预处理(`ensure_compliant`:不符合 `VideoConstraints` 才转码,产物按内容哈希缓存) → base64 → provider 适配器 → SSE 流式 → Tauri `Channel` → 前端多列实时渲染。`transcode` 优先用硬件编码器(macOS `videotoolbox` / Windows `nvenc`·`qsv`·`amf`),失败逐个回退到 `libx264`;硬件编码器不支持 CRF,统一用目标码率控制。`VideoConstraints` 默认压到 480p / 5fps —— 模型按抽帧分析,降分辨率/帧率几乎无损理解、大幅减小 base64 体积(阿里百炼 data-uri 上限 20MB)。

**数据层:** sqlx + SQLite,8 张表(providers / models / schemes / batch_jobs / job_items / runs / logs / settings),migration 在 `src-tauri/migrations/`(0001 建表、0002 加 `settings`)。`settings` 是 key-value 配置表(如 `batch_concurrency`)。`repo.rs` 是唯一数据访问层;`commands.rs` 是薄 Tauri 命令层,只转发到 `repo` / `provider` / `batch`。

**批量处理(`batch.rs`):** 有界并发(信号量)worker 逐单元处理 —— 并发数从 `settings.batch_concurrency` 读取(默认 10),是「完成一个即补入一个」的滑动窗口,非分批。`job_items` 是最小恢复单元,状态机 `pending → processing → done/failed`。**中断恢复**:启动时 `reset_processing_items` 把中断在 `processing` 的单元重置为 `pending`,用户点「继续」即重跑。

**执行日志:**「任务记录」来自 `runs` 表(每次视频理解写一条);「调试日志」由 `tauri-plugin-log` 写入 app 日志目录的 `dramadna.log`(Debug 级),前端经 `read_debug_log` 命令读尾部。

**前端状态(Zustand,`src/store/useAppStore.ts`):** 视频理解 Tab 是「多个模型共用一个 prompt」做对比。会话状态(视频、共用 prompt、选中的 modelIds、各列结果)提升到 store 而非组件 `useState` —— 切换 Tab 会卸载组件,放 store 才能保留。修改走 `setUnderstand`(支持函数式 patch)。

## 关键约定

- **API key 明文存 SQLite**(`providers.api_key`)—— 项目明确选择;`*.db` 已 gitignore,密钥读写封装在 repo 层便于将来换钥匙串。
- **`examples/seed.rs` 必须放 `examples/` 而非 `src/bin/`** —— 否则会被 `tauri build` 打进发布包。
- **ffmpeg 解析**:开发期 `ffmpeg_bin()` 显式探测系统路径(macOS GUI 应用的 PATH 不含 Homebrew,故查 `/opt/homebrew/bin` 等);打包后 `resolve_sidecars()` 把环境变量指向 bundle 内置 sidecar。二进制(63MB×2)不入库。
- **摩卡主题**:`src/styles/theme.css` 用 Tailwind 4 `@theme` 定义 oklch 暖色 token(`mocha-crust` → `mocha-rim` 六层 surface 等),组件用 `bg-mocha-*` / `.pane` 等工具类,勿硬编码颜色。
- 自定义标题栏(`tauri.conf.json` 的 `decorations:false` + `WindowControls` 组件)。
- macOS 签名凭据走环境变量(`APPLE_SIGNING_IDENTITY` 等),不写进 `tauri.conf.json`。

## 工作准则

降低常见 LLM 编码错误。琐碎任务可灵活判断。

### 1. 想清楚再写

显式陈述假设,不确定就问;有多种解读时摆出来,不要默默选一个;有更简单的方案就说出来,必要时反对。不清楚就停下来,指出困惑点并发问。

### 2. 简单优先

解决问题的最少代码,不做投机性设计。不加没要求的特性、抽象、"灵活性"或配置项;不为不可能的场景写错误处理。200 行能压到 50 行就重写。

### 3. 手术刀式修改

只动必须动的。不"顺手改进"相邻代码、注释、格式;不重构没坏的东西;匹配现有风格。只清理自己改动产生的孤儿(import/变量/函数);发现无关的既有死代码,提出来而非擅自删除。每一行改动都应能直接追溯到用户的需求。

### 4. 目标驱动

把任务转成可验证目标(如"加校验"→"为非法输入写测试再让其通过"),循环到验证通过。多步任务先说简短计划,每步带验证点。
