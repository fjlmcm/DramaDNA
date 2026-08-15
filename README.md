# DramaDNA

短剧结构化拆解桌面客户端。导入“一部剧一个目录”的竖屏短剧素材后，DramaDNA 会按可配置的资产管线提取人物、剧情、场景、台词、视听语言、分镜与节奏数据，并把结果输出为可继续编辑和分析的 Markdown 素材包。

项目基于 Tauri 2、Rust、React 和 SQLite，面向 macOS 与 Windows。

## 功能

- 多供应商模型接入：支持 OpenAI 兼容接口，并内置火山引擎 Ark、阿里百炼、小米 MiMo 等配置骨架
- 分阶段拆解：按全剧、分段、分集三个粒度调度资产任务和依赖
- 中断恢复：任务持久化到本地 SQLite，可在应用重启后继续
- 视频预处理：使用 FFmpeg 自动探测、转码、压缩和缓存超规格视频
- 资产配置：可启停拆解项、调整提示词、模型和请求参数
- 结果浏览：按剧目和资产查看 Markdown 产出与执行日志
- 视频理解调试：同一视频可交给多个模型并行分析和对比

## 环境要求

- Node.js 20 或更高版本
- pnpm 9 或更高版本
- Rust 1.88 或更高版本
- `ffmpeg` 与 `ffprobe` 可从系统 `PATH` 找到
- macOS 或 Windows 的 [Tauri 系统依赖](https://v2.tauri.app/start/prerequisites/)

## 本地开发

```bash
pnpm install --frozen-lockfile
pnpm tauri dev
```

首次启动会在系统应用数据目录创建 SQLite 数据库。随后在“设置 → 模型供应商”中配置供应商地址和 API key。

## 验证

```bash
# 前端类型检查与生产构建
pnpm build

# Rust 格式、静态检查与测试
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml

# 生产前端依赖审计
pnpm audit --prod
```

需要真实 API key、网络或本地视频的测试默认标记为 `ignored`，不会在普通测试命令中运行。

## 数据与密钥安全

- 视频、拆解产物、数据库、日志、API key 和 FFmpeg sidecar 均不应提交到 Git；仓库的 `.gitignore` 已覆盖这些常见文件。
- API key 当前以明文保存在本机 SQLite 数据库中，不会由应用主动上传到项目仓库。请只在受信任的本机账户中使用，并妥善保护数据库和系统备份。
- 不要在 issue、日志、截图或测试代码中粘贴真实密钥。安全问题请按 [SECURITY.md](SECURITY.md) 私下报告。

## 项目结构

```text
src/                         React 前端
src-tauri/src/               Tauri/Rust 后端
src-tauri/src/pipeline.rs    拆解任务调度
src-tauri/src/prompts.rs     内置资产规格与提示词
src-tauri/src/provider.rs    模型供应商适配
src-tauri/src/ffmpeg.rs      视频探测与预处理
src-tauri/src/repo_dna.rs    剧目与资产数据访问
src-tauri/migrations/        SQLite schema
```

构建发布包和准备 FFmpeg sidecar 的说明见 [BUILD.md](BUILD.md)，参与开发见 [CONTRIBUTING.md](CONTRIBUTING.md)。

准备好 sidecar 后，可通过发布专用配置构建安装包：

```bash
pnpm bundle
```

## 许可证

DramaDNA 源代码采用 [MIT License](LICENSE)。FFmpeg 和其他第三方组件不受 DramaDNA 的 MIT 许可证覆盖，详情见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
