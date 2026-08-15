# Third-party notices

DramaDNA depends on third-party libraries listed in `pnpm-lock.yaml` and `src-tauri/Cargo.lock`. Those components remain subject to their respective licenses.

## FFmpeg

The source repository does not include FFmpeg binaries. Release packages bundle
`ffmpeg` and `ffprobe` as separate Tauri sidecar executables.

The release workflow currently uses these pinned archives:

| Target | Build | Archive SHA-256 |
| --- | --- | --- |
| macOS Apple Silicon | Martin Riedl FFmpeg/FFprobe 9.0, build `1785863997_9.0` | FFmpeg `5267ef149ee0d208057a1b316aac079b661b0476574dee5da7d225769773c603`; FFprobe `7778fbb533fb60d3336cbd9a9e51eced71658f020b570c7203590c1c41d42f50` |
| macOS Intel | Martin Riedl FFmpeg/FFprobe 9.0, build `1785871427_9.0` | FFmpeg `79d14663d8b078dbbc38de18d63a30f8a5bfc860af5dfee7f8cf3e387cf1c02c`; FFprobe `a2dd3f2e7eb35a10fa6ac43b1a8c21890f27bee0dc4a86ddee16a57d72d3898d` |
| Windows x64 | BtbN FFmpeg `n8.1.2-40-g852b0552f0`, GPL static build `autobuild-2026-08-14-13-16` | `4dc80a665fd8a3481acae3f7836807334396a607581f7123b35a71f2ebaacb5d` |

These builds enable `--enable-gpl`, `--enable-version3`, libx264 and libx265.
They therefore use the GNU General Public License version 3 or later, not FFmpeg's
default LGPL terms. The selected archives do not use the non-redistributable
`--enable-nonfree` variant.

Exact download URLs and automated checksum enforcement are recorded in
`.github/workflows/release.yml`. Corresponding source and build information:

- FFmpeg 9.0 source: <https://ffmpeg.org/releases/ffmpeg-9.0.tar.xz>
- FFmpeg source at Windows build commit `852b0552f0`:
  <https://github.com/FFmpeg/FFmpeg/tree/852b0552f0>
- Martin Riedl build scripts: <https://git.martin-riedl.de/ffmpeg/build-script>
- Martin Riedl release build metadata: <https://ffmpeg.martin-riedl.de/>
- BtbN build scripts and dependency recipes: <https://github.com/BtbN/FFmpeg-Builds>
- BtbN pinned build release:
  <https://github.com/BtbN/FFmpeg-Builds/releases/tag/autobuild-2026-08-14-13-16>
- Bundled GNU GPL version 3 license text: [LICENSES/GPL-3.0.txt](LICENSES/GPL-3.0.txt)

- FFmpeg legal and compliance guidance: <https://ffmpeg.org/legal.html>
- FFmpeg source and downloads: <https://ffmpeg.org/download.html>

This notice is informational and is not legal advice.
