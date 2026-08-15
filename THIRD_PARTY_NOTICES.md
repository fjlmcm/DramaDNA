# Third-party notices

DramaDNA depends on third-party libraries listed in `pnpm-lock.yaml` and `src-tauri/Cargo.lock`. Those components remain subject to their respective licenses.

## FFmpeg

The source repository does not include FFmpeg binaries. Local and release builds may bundle `ffmpeg`, `ffprobe`, and related shared libraries as Tauri sidecars.

FFmpeg is generally licensed under the GNU Lesser General Public License version 2.1 or later. A build that enables GPL components is covered by the GNU General Public License version 2 or later; a build made with `--enable-nonfree` may not be redistributable.

Anyone distributing a DramaDNA package containing FFmpeg is responsible for identifying the exact FFmpeg build and complying with its applicable license. This includes retaining the license text, corresponding source code, build/configuration information and any relinking requirements applicable to the distributed binaries.

- FFmpeg legal and compliance guidance: <https://ffmpeg.org/legal.html>
- FFmpeg source and downloads: <https://ffmpeg.org/download.html>

This notice is informational and is not legal advice.
