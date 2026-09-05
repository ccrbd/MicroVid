# Third-party notices

MicroVid is licensed under the GNU General Public License v3.0 or later (see `LICENSE`).
It is a graphical front end for FFmpeg and does not modify FFmpeg.

## Bundled binaries (not stored in this repository)

`scripts/fetch-ffmpeg.sh` downloads prebuilt, statically linked FFmpeg and FFprobe binaries
at build time and the installers bundle them unchanged. They are GPL-licensed builds:

| Platform | Source | Licence of the build |
|---|---|---|
| macOS (arm64, x86_64) | https://ffmpeg.martin-riedl.de/ | GPL v3 (built with `--enable-gpl --enable-version3`) |
| Windows (x86_64) | https://github.com/BtbN/FFmpeg-Builds (`win64-gpl`) | GPL v3 |

FFmpeg is a trademark of Fabrice Bellard, originator of the FFmpeg project.
FFmpeg source code: https://ffmpeg.org/ — each build provider publishes the exact configure
line and library versions alongside the binaries (`versions.txt`).

Notable libraries inside those builds and their licences:

- FFmpeg (libavcodec, libavformat, libavfilter, libswscale, libswresample): LGPL 2.1+ / GPL 2+ / GPL 3
- x264 (H.264 encoder): GPL 2+
- x265 (HEVC encoder): GPL 2+
- SVT-AV1 (AV1 encoder): BSD-3-Clause-Clear / Alliance for Open Media Patent License 1.0
- dav1d, libaom, libvpx, libopus, libvorbis, libmp3lame, libass, libfreetype, fontconfig,
  harfbuzz, libwebp, libzimg, libvmaf, zlib, OpenSSL: see each project's licence
- Apple AudioToolbox AAC (macOS): system framework provided by macOS, used through FFmpeg's `aac_at` wrapper

Anyone redistributing MicroVid binaries must also comply with the GPL for the bundled FFmpeg,
which these builds satisfy by publishing their source and build configuration.

## Application dependencies

| Component | Licence |
|---|---|
| Tauri | MIT / Apache-2.0 |
| React, Vite, Zustand, Tailwind CSS | MIT |
| lucide-react (icons) | ISC |
| tokio, serde, reqwest, rusqlite, sysinfo, walkdir, regex, chrono, uuid, base64, anyhow, thiserror | MIT / Apache-2.0 (rusqlite bundles SQLite, public domain) |

Full Rust dependency licences: `cargo license` in `src-tauri/`. JavaScript: `pnpm licenses list`.

## Services

- OpenSubtitles (https://www.opensubtitles.com/): optional subtitle search using the user's own API key,
  subject to OpenSubtitles' terms of use. MicroVid identifies itself as `MicroVid v0.1`.

## Encoding presets

The default CRF and preset choices are ordinary FFmpeg encoder settings and are not derived from
any third-party code. The app icon and all artwork are original.
