# MicroVid

Tiny, high-quality video re-encodes for your media server. MicroVid wraps ffmpeg in a
simple desktop app (macOS + Windows, built with Tauri 2) that follows the well-known
"360p that looks like DVD" recipe and extends it to 480p–1080p, HEVC and AV1.

- Drop a file, a season folder or a whole series; folder structure is mirrored in the output
- HEVC by default (≈40% smaller than x264), x264 for maximum compatibility, AV1 for the newest devices
- Content-type presets (general, drama, sitcom, animation, action, news) choose the CRF for you
- Apple AAC audio on macOS with dialogue-preserving 5.1 → stereo downmix
- Automatic black-bar cropping, aspect ratio preserved, never upscales
- Subtitles picked from the same folder or `Subs/`, embedded tracks kept, delay adjustment, burn-in,
  OpenSubtitles search
- Job queue with 1–6 parallel encodes, live size/time estimates calibrated from your own machine,
  30-second test encode with before/after frame comparison
- Crash-safe: the queue is persisted, finished files are always complete, interrupted jobs resume
- Analytics page: space saved, speed, history, CSV export

## Development

Requirements: Node 20+, pnpm, Rust stable, and the platform prerequisites for
[Tauri 2](https://tauri.app/start/prerequisites/).

```bash
pnpm install
./scripts/fetch-ffmpeg.sh          # downloads static ffmpeg/ffprobe sidecars for this machine
pnpm tauri dev
```

Tests:

```bash
cd src-tauri
cargo test                                             # unit tests (argument builder, estimator, naming, subtitles, db)
MICROVID_SAMPLE=/path/to/file.mkv cargo test --test e2e_encode -- --ignored   # real ffmpeg end-to-end
```

Dev hook: `MICROVID_AUTO_ADD=/folder MICROVID_AUTO_OUT=/out pnpm tauri dev` adds a folder and starts the
queue automatically after launch.

## Building

```bash
./scripts/fetch-ffmpeg.sh aarch64-apple-darwin      # or x86_64-apple-darwin / x86_64-pc-windows-msvc
pnpm tauri build
```

The bundle includes ffmpeg and ffprobe, so users need nothing else. macOS builds come from
[ffmpeg.martin-riedl.de](https://ffmpeg.martin-riedl.de/) (includes Apple AudioToolbox AAC),
Windows builds from [BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds). Both are GPL.

## The recipe

| Content | x264 CRF | x265 CRF | SVT-AV1 CRF |
|---|---|---|---|
| General (default) | 24 | 26 | 34 |
| Drama / film | 24 | 26 | 34 |
| Sitcom / TV | 25 | 27 | 36 |
| Animation | 27 | 29 | 38 |
| Action / sports | 22 | 24 | 32 |
| News / talk | 28 | 30 | 40 |

x264 uses `veryslow`, main profile; x265 uses `slower`, main profile, `hvc1` tag; SVT-AV1 uses preset 4,
10-bit. Audio defaults to 80 kb/s stereo AAC. Everything is adjustable in Advanced mode.

## Layout

```
src/            React + TypeScript + Tailwind frontend
src-tauri/src/  Rust backend
  command.rs    EncodeSettings → ffmpeg argv (pure, unit-tested)
  estimate.rs   size/time estimates + calibration
  queue.rs      scheduling, ffmpeg runner, progress, recovery
  probe.rs      ffprobe → MediaInfo
  cropdetect.rs black-bar detection
  subtitles.rs  folder matching, SRT shifting, OpenSubtitles client
  naming.rs     output paths, tag and signature
  db.rs         SQLite: queue, history, calibration, settings
  power.rs      sleep prevention, orphan cleanup
scripts/        fetch-ffmpeg.sh
```

## Windows and macOS installers

GitHub Actions builds installers on every push to `main` (see `.github/workflows/build.yml`):
open the workflow run and download the `MicroVid-windows` (`.msi` and `.exe` installers) or
`MicroVid-macos-arm64` (`.dmg`, Apple Silicon) artifact. Tagging a commit `v*` also publishes a GitHub Release with the same files.

## Licence

GPL-3.0-or-later. See `LICENSE` and `THIRD_PARTY_NOTICES.md` for the bundled FFmpeg builds and other components.
