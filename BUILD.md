# DramaDNA 构建与打包

## 开发与测试

开发环境需要 Node.js 20+、pnpm 9+、Rust 1.88+，并确保 `ffmpeg` 和 `ffprobe` 可从系统 `PATH` 找到。

```bash
pnpm install --frozen-lockfile
pnpm tauri dev

pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

真实 API 测试需要使用自己的密钥和测试素材，且默认不会运行：

```bash
VOLC_KEY=... ALI_KEY=... YUN_KEY=... \
  cargo test --manifest-path src-tauri/Cargo.toml -- --include-ignored --nocapture
```

## 准备 FFmpeg sidecar

应用发布包需要包含与目标平台匹配的 `ffmpeg` 和 `ffprobe`。按 Tauri sidecar 命名约定放入 `src-tauri/binaries/`：

```text
src-tauri/binaries/ffmpeg-aarch64-apple-darwin
src-tauri/binaries/ffprobe-aarch64-apple-darwin
src-tauri/binaries/ffmpeg-x86_64-pc-windows-msvc.exe
src-tauri/binaries/ffprobe-x86_64-pc-windows-msvc.exe
```

这些二进制和相关动态库已被 `.gitignore` 排除，不属于源码仓库。构建者需要自行取得或构建适用版本，并在发布前检查其许可证和配置：

```bash
ffmpeg -version
ffmpeg -buildconf
```

FFmpeg 默认采用 LGPL 2.1+；如果构建时启用了 GPL 组件，则对应 FFmpeg 构建适用 GPL。不要分发使用 `--enable-nonfree` 构建且不可再分发的二进制。分发者还应保留精确源码、构建配置、许可证文本和必要的重新链接条件。完整要求以 [FFmpeg 官方法律与许可说明](https://ffmpeg.org/legal.html) 为准，项目侧说明见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。

发布专用的 `src-tauri/tauri.release.conf.json` 声明：

```json
{
  "bundle": {
    "externalBin": ["binaries/ffmpeg", "binaries/ffprobe"],
    "resources": {
      "../LICENSE": "LICENSE",
      "../LICENSES/GPL-3.0.txt": "FFmpeg-GPL-3.0.txt",
      "../THIRD_PARTY_NOTICES.md": "THIRD_PARTY_NOTICES.md"
    }
  }
}
```

## macOS

建议使用未启用 GPL/nonfree 组件的 FFmpeg，并在需要捆绑动态库时把对应 `libav*.dylib` 放入 `src-tauri/Frameworks/`，同时在 `tauri.conf.json` 的 `bundle.macOS.frameworks` 中列出。

签名和公证凭据只能通过本机安全环境变量或 CI Secrets 注入，不得写入仓库：

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: ..."
export APPLE_ID="..."
export APPLE_PASSWORD="..."
export APPLE_TEAM_ID="..."

pnpm bundle
```

构建产物位于 `src-tauri/target/release/bundle/`。

## Windows

在 Windows 环境安装 Tauri 前置依赖并准备相应 FFmpeg sidecar 后执行：

```powershell
pnpm install --frozen-lockfile
pnpm bundle
```

构建产物位于 `src-tauri/target/release/bundle/`。

## 自动发布

推送与项目版本一致的语义化版本标签（例如 `v0.1.14`）会触发
`.github/workflows/release.yml`。工作流会：

1. 检查标签与 `package.json`、`Cargo.toml`、`tauri.conf.json` 的版本一致。
2. 下载固定版本的 FFmpeg sidecar，并在解压前验证 SHA-256。
3. 分别构建 macOS Apple Silicon、macOS Intel 的 DMG，以及 Windows x64 的
   NSIS 和 MSI 安装包。
4. 验证四个安装包都已上传，生成 `SHA256SUMS.txt`，附带项目许可证、GPLv3
   正文和第三方声明，再把草稿发布为 GitHub prerelease。

```bash
git tag -a v0.1.14 -m "DramaDNA v0.1.14"
git push origin v0.1.14
```

当前自动构建使用 macOS ad-hoc 签名，Windows 安装包未做 Authenticode 签名，
因此系统可能显示来源或安全警告。在配置 Apple Developer ID、公证凭据和 Windows
代码签名证书并验证安装流程之前，发布保持 prerelease 状态。

## 发布检查

- 工作区干净，版本号在 `package.json`、`Cargo.toml` 和 `tauri.conf.json` 中一致
- `pnpm build`、`cargo fmt --check`、Clippy 和 Rust 测试全部通过
- `pnpm audit --prod` 和 RustSec 审计没有未评估漏洞
- 包内没有密钥、数据库、日志、测试素材或开发者路径
- FFmpeg 来源、源码、配置和许可证材料与实际发布二进制一一对应
- GitHub Release 包含两个 DMG、一个 MSI、一个 NSIS EXE 和 `SHA256SUMS.txt`
- 在干净虚拟机上验证安装、启动、模型配置和卸载流程
