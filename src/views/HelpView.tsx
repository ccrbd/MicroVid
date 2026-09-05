function H({ children }: { children: React.ReactNode }) {
  return <h2 className="mt-5 mb-2 text-[14px] font-medium">{children}</h2>;
}
function P({ children }: { children: React.ReactNode }) {
  return <p className="mb-2 text-[12.5px] leading-relaxed" style={{ color: "var(--mv-muted)" }}>{children}</p>;
}

export default function HelpView() {
  return (
    <div className="h-full overflow-y-auto p-4">
      <div className="mx-auto max-w-3xl select-text">
        <h1 className="text-[18px] font-medium">How to use MicroVid</h1>
        <P>MicroVid re-encodes your videos into small files that still look good on a TV, following the well-known “360p that looks like DVD” recipe, and extends it to 480p and beyond and to modern codecs.</P>

        <H>1. Add files</H>
        <P>Drag a file, a season folder, or a whole series folder onto the window, or use Add files / Add folder. Sub-folders are included, and the folder structure is mirrored in the output folder. Each file is analysed: resolution, frame rate, audio tracks, embedded subtitles and black bars.</P>

        <H>2. Check the settings</H>
        <P>Simple mode shows the five choices that matter: codec, resolution, content type, audio and subtitles. The estimate cards update as you change them. Use “Apply to all pending” to set a whole season at once. Advanced mode adds CRF, encoder preset, 10-bit, hardware fast mode, audio tracks, crop, subtitle delay and burn-in, container and extra ffmpeg arguments.</P>

        <H>3. Test encode</H>
        <P>Encodes 30 seconds from the middle of the file with the current settings and shows the real size extrapolated to the full file, the real speed, and a before/after frame comparison with a wipe slider. The measured speed also calibrates the time estimates for your machine.</P>

        <H>4. Start the queue</H>
        <P>Press Start. New files you add while encoding join the queue. The status bar at the bottom shows progress for everything; click it for analytics. Pause freezes running encodes; the app keeps your computer awake while it works.</P>

        <H>The recipe, explained</H>
        <P><b>Slow preset.</b> x264 “veryslow” and x265 “slower” spend a lot of CPU searching for the best way to represent each frame. At 360p–480p this is affordable and it is where most of the quality per megabyte comes from.</P>
        <P><b>Constant quality (CRF).</b> Instead of a fixed bitrate, CRF keeps visual quality steady and lets the size vary with the content. Lower numbers mean better quality and bigger files. The content type sets a sensible CRF: dramas need a little more, cartoons and news get away with less, action and sports need the most.</P>
        <P><b>Audio matters.</b> At these sizes audio can be a third of the file. MicroVid uses Apple's AAC encoder on macOS, which stays clean at 64–80 kb/s stereo, and downmixes 5.1 so dialogue stays clear.</P>
        <P><b>Source quality matters.</b> Encode from the best source you have. A 1080p Blu-ray rip downscaled to 480p looks far better than a 480p source re-encoded.</P>
        <P><b>Aspect ratio and cropping.</b> Black bars are detected and cropped so no bits are wasted on them, the picture is never stretched, and nothing is ever upscaled: selecting 1080p for a 720p source keeps 720p.</P>

        <H>Which codec?</H>
        <P><b>HEVC (default)</b> is roughly 40% smaller than x264 at the same quality and plays natively on Samsung TVs since 2016, Android TV, Apple TV 4K, iPhone/iPad, and Emby's web player in Safari, Edge and recent Chrome. Files are tagged so Apple devices direct-play them.</P>
        <P><b>x264</b> plays on literally everything, encodes 3–4× faster, and is the right choice for very old devices or when you want zero transcoding risk.</P>
        <P><b>AV1</b> is another 20–30% smaller than HEVC but only newer hardware decodes it (Samsung 2020+ TVs, some Android TV boxes, iPhone 15 Pro / M3 Macs and later). Emby will transcode for devices that cannot play it, which costs server CPU.</P>
        <P><b>Fast mode</b> uses your GPU's encoder (VideoToolbox, NVENC, QSV). It is many times faster but files come out 30–50% larger at the same quality. Handy for a quick draft.</P>

        <H>Subtitles</H>
        <P>Files named like the video are found automatically, including language suffixes (Movie.en.srt) and Subs/ folders. Subtitles already inside the source are kept. Use the subtitle dropdown to pick a different file or search OpenSubtitles (needs an API key in Settings). Set a delay in milliseconds if they are out of sync; negative moves them earlier. Burn in only if a device cannot show soft subtitles.</P>

        <H>Crashes, sleep and shutdown</H>
        <P>The queue is saved to disk continuously. A finished file is always complete: MicroVid writes to a temporary name and renames it at the end. If the app or computer stops mid-encode, that job is marked Interrupted on the next launch, the partial file is removed, and you can resume with one click. Encodes cannot continue from where they stopped, so an interrupted file starts over.</P>

        <H>Keyboard</H>
        <P>⌘O add files · ⌘⇧O add folder · ⌘, settings · Esc back to the queue.</P>
      </div>
    </div>
  );
}
