#!/usr/bin/env python3
"""
doubao_summary.py —— DramaDNA 的单文件视频摘要示例。

把本地视频按 DramaDNA 的规则压缩到豆包 base64 上限(43 MiB),提交豆包视觉
理解,返回约 800 字剧情。零外部依赖:仅 Python 3.9+ 标准库 + 系统 ffmpeg。

用法:
    VOLC_KEY=ark-xxxxxxxx python3 doubao_summary.py [test.mp4]

依赖:
    - Python 3.9+(标准库 urllib / base64 / subprocess / json,无 pip 包)
    - ffmpeg + ffprobe 在 PATH(macOS: brew install ffmpeg)
"""
from __future__ import annotations

import base64
import json
import os
import shutil
import subprocess
import sys
import tempfile
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path

# ─── 豆包配置 ───────────────────────────────────────────
API_URL = "https://ark.cn-beijing.volces.com/api/v3/chat/completions"
MODEL_ID = "doubao-seed-2-0-lite-260428"
# 豆包 HTTP 请求体硬上限 64MB(实测 v0.1.10) → base64 ≤ ~60MB → 原始视频
# ≤ 45MB,设 43 MiB 留余量。
MAX_BYTES = 43 * 1024 * 1024  # 45,088,768

# ─── 转码规则 ───────────────────────────────────────────
USER_MAX_EDGE = 854        # 长边上限,等同 default 480p(横屏 854×480 / 竖屏 480×854)
USER_MAX_FPS = 5.0
USER_AUDIO_BITRATE = 64_000  # bps

# 视频码率绝对下限 —— 极端长视频压不进 max_bytes 时给反馈修正留余地。
# videotoolbox / nvenc 对 maxrate 是「软目标」,实际平均码率可能比目标高 ~40%。
MIN_BITRATE = 50_000

# 码率→分辨率长边阶梯。低码率配低分辨率,每像素分到的比特更多、画面更可辨。
RESOLUTION_LADDER = [
    (700_000, 10**9),  # 充足:不额外限制,用满用户设定
    (400_000, 640),
    (200_000, 480),
    (0, 320),
]

MAX_RETRANSCODE = 3

PROMPT = (
    "请用约 800 字(700-900 字)详细描述这部短剧:"
    "剧情主线、关键人物与关系、核心冲突与转折、最终结局。"
    "用连续叙述段落,不要列点,不要做开头总结。"
)


# ─── ffmpeg / ffprobe ───────────────────────────────────────────────


def _bin(name: str) -> str:
    p = shutil.which(name)
    if not p:
        sys.exit(f"找不到 {name},请先安装 ffmpeg 并加入 PATH")
    return p


@dataclass
class Probe:
    duration_s: float
    width: int
    height: int
    fps: float
    size_bytes: int
    codec: str


def probe(path: Path) -> Probe:
    cp = subprocess.run(
        [
            _bin("ffprobe"), "-v", "error", "-select_streams", "v:0",
            "-show_entries", "stream=width,height,codec_name,avg_frame_rate",
            "-show_entries", "format=duration,size",
            "-of", "json", str(path),
        ],
        capture_output=True, text=True, check=True,
    )
    data = json.loads(cp.stdout)
    s = data["streams"][0]
    f = data["format"]
    num, _, den = (s.get("avg_frame_rate") or "0/1").partition("/")
    d = float(den or 1)
    fps = (float(num) / d) if d else 0.0
    return Probe(
        duration_s=float(f.get("duration", 0) or 0),
        width=int(s.get("width", 0)),
        height=int(s.get("height", 0)),
        fps=fps,
        size_bytes=int(f.get("size", 0)),
        codec=s.get("codec_name", ""),
    )


# ─── 自动联动规则 ────────────────────────────────────────────────────


def estimate_bitrate(duration_s: float, audio_bitrate: int) -> int:
    """video 码率 = max_bytes×8×0.88/时长 − audio,触底 MIN_BITRATE。"""
    if duration_s <= 0.5:
        return 4_000_000
    raw = int(MAX_BYTES * 8 * 0.88 / duration_s) - audio_bitrate
    return max(raw, MIN_BITRATE)


def fit_dimension(bitrate: int, user_limit: int) -> int:
    for min_br, cap in RESOLUTION_LADDER:
        if bitrate >= min_br:
            return min(user_limit, cap)
    return 320


def fit_fps(bitrate: int, user_fps: float) -> float:
    if bitrate <= MIN_BITRATE:
        return min(user_fps, 1.0)
    if bitrate < 250_000:
        return min(user_fps, 3.0)
    return user_fps


def fit_audio_bitrate(video_bitrate: int, user_audio: int) -> int:
    """触底场景下 audio 让位给 video(避免 audio×duration 吃掉 max_bytes 大头)。"""
    if video_bitrate > MIN_BITRATE:
        return user_audio
    if user_audio == 0:
        return 16_000  # copy 模式触底时强制 16k,避免继承高码率源音频
    return min(user_audio, 16_000)


def _encoder_candidates() -> list[str]:
    if sys.platform == "darwin":
        return ["h264_videotoolbox", "libx264"]
    if sys.platform.startswith("win"):
        return ["h264_nvenc", "h264_qsv", "h264_amf", "libx264"]
    return ["libx264"]


def _run_encode(
    src: Path, dst: Path, p: Probe,
    edge: int, fps: float, video_br: int, audio_br: int, encoder: str,
) -> None:
    # 等比缩放进 edge×edge 框(不放大),再保证宽高偶数(h264 要求)。
    scale = (
        f"scale='min({edge},iw)':'min({edge},ih)':force_original_aspect_ratio=decrease,"
        f"scale=trunc(iw/2)*2:trunc(ih/2)*2"
    )
    eff_fps = max(min(p.fps, fps), 1.0)
    args = [
        _bin("ffmpeg"), "-y", "-i", str(src),
        "-vf", scale,
        "-r", f"{eff_fps:.0f}",
        "-c:v", encoder,
        "-b:v", str(video_br),
        "-maxrate", str(video_br),
        "-bufsize", str(video_br * 2),
        "-pix_fmt", "yuv420p",
    ]
    if audio_br == 0:
        args += ["-c:a", "copy"]
    else:
        args += ["-c:a", "aac", "-b:a", str(audio_br)]
    args.append(str(dst))
    cp = subprocess.run(args, capture_output=True, text=True)
    if cp.returncode != 0:
        raise RuntimeError(f"ffmpeg({encoder}) 转码失败: {cp.stderr[-2000:]}")


def transcode(src: Path, dst: Path, p: Probe) -> None:
    """首轮按估算 → 实测反馈修正二压,直到达标或判定不可压缩。"""
    bitrate = estimate_bitrate(p.duration_s, USER_AUDIO_BITRATE)
    encoder: str | None = None

    for round_ in range(MAX_RETRANSCODE + 1):
        edge = fit_dimension(bitrate, USER_MAX_EDGE)
        fps = fit_fps(bitrate, USER_MAX_FPS)
        a_br = fit_audio_bitrate(bitrate, USER_AUDIO_BITRATE)

        if encoder is None:
            last_err = ""
            for enc in _encoder_candidates():
                try:
                    _run_encode(src, dst, p, edge, fps, bitrate, a_br, enc)
                    encoder = enc
                    print(f"  → 编码器: {enc}", file=sys.stderr)
                    break
                except RuntimeError as e:
                    last_err = str(e)
                    print(f"  ↩ 编码器 {enc} 不可用,回退", file=sys.stderr)
            if encoder is None:
                raise RuntimeError(f"所有编码器都失败:{last_err}")
        else:
            _run_encode(src, dst, p, edge, fps, bitrate, a_br, encoder)

        actual = dst.stat().st_size
        if actual <= MAX_BYTES:
            print(
                f"  ✓ 第 {round_+1} 次编码达标:{actual/1024/1024:.2f}MB ≤ "
                f"{MAX_BYTES/1024/1024:.0f}MB(码率 {bitrate}, 长边 {edge}, "
                f"{fps:.0f}fps, audio {a_br})",
                file=sys.stderr,
            )
            return

        if round_ == MAX_RETRANSCODE:
            raise RuntimeError(
                f"二压 {MAX_RETRANSCODE} 轮后仍超体积上限"
                f"({actual} > {MAX_BYTES} 字节);视频过长无法压到上限,"
                f"请缩短视频时长(粗算可处理时长 ≈ {MAX_BYTES * 8 // MIN_BITRATE // 60} 分钟)。"
            )

        # 反馈修正:按实测比例下调码率,留 5% 裕度。
        next_br = max(int(bitrate * MAX_BYTES / actual * 0.95), MIN_BITRATE)
        if next_br >= bitrate:
            raise RuntimeError(
                f"码率已降至下限 {bitrate} bps,产物仍超体积上限"
                f"({actual} > {MAX_BYTES} 字节);视频过长无法压到上限,"
                f"请缩短视频时长(粗算可处理时长 ≈ {MAX_BYTES * 8 // MIN_BITRATE // 60} 分钟)。"
            )
        print(
            f"  ⤵ 产物 {actual/1024/1024:.2f}MB 超上限,第 {round_+1} 轮二压:"
            f"码率 {bitrate} → {next_br}",
            file=sys.stderr,
        )
        bitrate = next_br


def ensure_compliant(src: Path) -> Path:
    """体积达标(任何 codec)→ 直接返回原路径;超标 → 转码到临时文件。"""
    p = probe(src)
    print(
        f"源视频: {p.width}×{p.height} {p.fps:.0f}fps {p.duration_s:.0f}s "
        f"{p.codec} {p.size_bytes/1024/1024:.1f}MB",
        file=sys.stderr,
    )
    if p.size_bytes <= MAX_BYTES:
        print(
            f"  ✓ 体积达标({MAX_BYTES/1024/1024:.0f}MB 内,任何 codec 都接受),"
            "直接提交",
            file=sys.stderr,
        )
        return src

    print(
        f"  ⤵ 超出 {MAX_BYTES/1024/1024:.0f}MB,开始转码", file=sys.stderr
    )
    out = Path(tempfile.gettempdir()) / f"doubao-summary-{os.getpid()}.mp4"
    transcode(src, out, p)
    return out


# ─── 豆包 API ───────────────────────────────────────────────────────


def call_doubao(video_path: Path, api_key: str) -> str:
    size_mb = video_path.stat().st_size / 1024 / 1024
    print(f"准备 base64({size_mb:.1f}MB)…", file=sys.stderr)
    b64 = base64.b64encode(video_path.read_bytes()).decode("ascii")
    data_url = f"data:video/mp4;base64,{b64}"

    body = {
        "model": MODEL_ID,
        "stream": False,
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": PROMPT},
                {"type": "video_url", "video_url": {"url": data_url}},
            ],
        }],
    }
    payload = json.dumps(body).encode("utf-8")
    print(f"提交豆包(请求体约 {len(payload)/1024/1024:.1f}MB)…", file=sys.stderr)

    req = urllib.request.Request(
        API_URL,
        data=payload,
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=300) as resp:
            data = json.loads(resp.read())
    except urllib.error.HTTPError as e:
        err = e.read().decode("utf-8", errors="replace")
        raise RuntimeError(f"API 返回 {e.code}: {err}")
    return data["choices"][0]["message"]["content"]


# ─── main ──────────────────────────────────────────────────────────


def main() -> int:
    api_key = os.environ.get("VOLC_KEY")
    if not api_key:
        sys.exit("请设置环境变量 VOLC_KEY=<火山引擎 ark key>")

    src_arg = sys.argv[1] if len(sys.argv) > 1 else "test.mp4"
    src = Path(src_arg).expanduser().resolve()
    if not src.is_file():
        sys.exit(f"找不到文件: {src}")

    ready = ensure_compliant(src)
    try:
        result = call_doubao(ready, api_key)
    finally:
        # 清理转码临时文件(若产生)。
        if ready != src and ready.exists():
            ready.unlink(missing_ok=True)

    print(result)
    return 0


if __name__ == "__main__":
    sys.exit(main())
