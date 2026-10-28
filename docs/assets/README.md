# Visual assets

All application captures show the English release build. The images are real window captures, not generated UI or retouched values.

- `banner.png`: an abstract brand illustration localized using the built-in ImageGen tool. Final text: **SpaceFinder — Space for what matters.** It does not depict the application UI.
- `overview.png`, `explorer.png`, `largest-files.png`, `file-types.png`, `recommendations.png`: actual SpaceFinder window captures using Windows Computer Use.
- `demo.gif` and `demo.mp4`: a 24.9-second edited walkthrough of 11 real captured states: welcome, entering the demo path, scan results, explorer, expanding a folder, file properties, largest files, file types, filtering by extension, recommendations and **Review in tree**. Captures are held for readability; this is not a continuous real-time screen recording. No UI values or transitions were fabricated. Both formats are 1120 × 744. The GIF uses 128 colors per frame; the MP4 uses H.264, 10 fps and no audio.

## Demo data

The application scanned `C:\SpaceFinder-Demo`, an isolated fixture containing 15 synthetic files (178.01 MiB logical total) in Videos, Projects, Audio, Photography and Documents. The Cargo build directory contains synthetic manifest/output markers so the recommendation engine can demonstrate its evidence panel. These files are not usable media or binaries. The demonstration is illustrative, not a performance measurement. Drive capacity/free-space values come from the real Windows volume.

## Banner localization prompt

Mode: built-in ImageGen, editing the previous banner. Prompt used verbatim:

> Use case: text-localization. Edit the provided SpaceFinder GitHub banner. Replace ONLY the Portuguese subtitle 'Espaço para o que importa.' with the exact English text 'Space for what matters.' Preserve the SpaceFinder title, dark graphite background, mint/teal colors, typography quality, composition and abstract 3D treemap blocks. Keep the wide panoramic 3:1 aspect ratio. All visible text must be English. No extra text or watermark.
