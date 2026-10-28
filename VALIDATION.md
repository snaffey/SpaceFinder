# Local validation — October 24, 2026

`release` build, Windows x64, on this computer. These are local measurements, not a prediction for a whole drive or a comparison with TreeSize.

## English interface and media — October 28, 2026

- Translated the application interface, recommendation evidence/cautions, file-action errors, CLI output and build messages into English.
- Preserved Portuguese folder-name aliases in location detection and Unicode test fixtures; these are input compatibility and test coverage, not untranslated UI.
- Formatting and Clippy passed without warnings. **30 tests passed; 2 integration fixtures were ignored by default.** The test assertions were updated to match English messages.
- Rebuilt the portable executables. Screenshots and demonstration media use the English application and synthetic files, not personal file listings or performance benchmarks.

## Administrator launch — October 25, 2026 fix

- The original window closes automatically only after the new elevated instance confirms readiness, sent after drawing its first UI frame. The new window receives the same path to rescan.
- Confirmation uses an event unique to each attempt and is checked against the process created by Windows. A successful launch call alone does not close the original window.
- Canceling the administrator request, launch failure, early exit or missing confirmation keeps the original window open. Waiting runs on a worker, with a timeout after process creation, without blocking the UI.
- **30 tests passed, 2 integration fixtures ignored by default.** New tests cover readiness with a live process, exit without readiness, timeout without terminating the process and rejection of readiness from an unprivileged process. Test subprocesses do not request elevation or trigger UAC. Clippy and formatting passed.
- Actual UAC approval was not automated.
- The portable executable was updated and its hash checked against the `release` build. The existing window was already elevated and was preserved; the previous running version was kept in `target/` while replacing the portable file.

## Version 0.4 — October 25, 2026

- **26 tests passed**, plus a Recycle Bin integration test ignored by default. New rules cover misleading folder names, manifests and build markers, unexpected files in generated folders, system/cloud locations, partial metadata, unknown/future dates, non-overlap and a 200-result limit with 10,000 candidates. Formatting and Clippy passed without warnings.
- Two new junction tests reject redirection within the same root and subsequent replacement of an ancestor. Neither recycles files; they use generated test fixtures only.
- The native window was checked: sidebar navigation, summary, recommendations, file types and **Review in tree**. Recommendations do not run or delete the item.
- In this project, the engine identified `target/` using Cargo markers and the manifest, with 2,505,303,074 observed bytes in the initial measurement. The suggestion explains that rebuilding must be possible, builds must be closed and manual files must be ruled out. No cleanup was performed.
- Local `C:\` scan in standard mode with 16 workers: **2,282,184 files**, **411,106 folders**, **941,239,867,991 logical bytes measured**, **25.40 s** scanner time and **25.74 s** for the complete process including recommendations and reading JSON in PowerShell. It recorded 478 access-denied issues and 1,476 skipped links. Cache and disk changes make these measurements variable; coverage is partial.
- **53 suggestions** were found: 3 recreatable Cargo artifacts requiring review (14,460,821,485 bytes), 2 Windows-managed locations and 48 personal files for inspection (38,822,803,763 bytes). These figures do not promise recoverable physical space and totals did not double-count items.
- Recommendation analysis runs separately from the UI, with a report bound to the same `Arc<Scan>`; a late report from an earlier scan is not applied.
- An independent check of the 53 recommended paths confirmed no ancestor/descendant overlaps. All card actions inspect items or open Windows tools; none performs cleanup.
- The final portable executable matches the `release` build. The final CLI confirmed 15 files and 210,265 bytes in `src/` against an independent enumeration, with no artificial recommendations for the source folder.
- In the final window check, rescanning `C:\` took 12.34 s and again showed 53 suggestions and 13.47 GiB logical size in recreatable artifacts requiring review. The difference illustrates cache/disk-state effects. The app was left open on Recommendations; no cleanup was performed.

## Version 0.3 — October 25, 2026

- **15 automated tests passed**, plus the native Recycle Bin test run separately. Clippy and formatting passed. The Recycle Bin test is ignored by default so `cargo test` does not move files without explicit intent.
- The native test moved only a disposable folder generated in `target/`, containing one test file, to the Recycle Bin. Rejection of the scan root, outside paths and files changed after preparation was also tested.
- Folder confirmation requires typing its name before the button becomes available. **Cancel** was used during UI verification, leaving the preview folder intact. The preview folder was later removed without recursion after verifying its path and contents.
- Context menu, right-click selection, sorting through header clicks and native properties were verified. The properties window was closed without changes.
- The CLI scan of `src/` returned **13 files**, **138,235 bytes**, zero errors and zero incomplete entries; totals were independently confirmed.
- The final version scanned `C:\` in standard mode: **2,281,328 files**, **410,985 folders**, **940,869,640,838 logical bytes measured**, in **10.14 s**. It recorded **477 access-denied folders**, **zero other errors** and **1,476 skipped links**. These are partial results, explicitly marked in the UI and affected branches.
- **Coverage and permissions** was checked using real `C:\` issues, including `PerfLogs`, `System Volume Information` and `ProgramData`. Its explanation states that administrator is optional, may improve coverage and does not guarantee access to every folder.
- The elevation button was not triggered automatically; Windows permissions were not changed. Argument construction with spaces and trailing backslashes has a dedicated test.
- The portable executable matched the version 0.3 `release` build and was left open on the C drive scan.

## Version 0.2 — October 25, 2026

- **12 tests passed**, including FILETIME conversion, native dates and branch-date aggregation, tree expansion/collapse, cached-sort invalidation, search with ancestors and extension filters. Formatting and Clippy also passed.
- Expandable tree, properties/treemap updates on folder selection, name sorting and extension view were checked in the native window. Double-clicking `.exe` opened the 102 matching files in the test scan.
- CSV exported using the native dialog and read independently: **4,754 nodes**, including **4,084 files**, **1,771,391,798 bytes**. The file sum matched the root. All nine columns were present, including Unix dates and attributes; every file had a modified date.
- The portable CLI scanned the **10 files** in `src/`, totaling **98,323 bytes**. An independent enumeration confirmed the count, size and modified date of one entry.
- New project-folder measurement with details enabled and 16 workers: **235,213 files**, **25,013 subfolders**, **19,074,732,179 bytes**, in **1.417 s**; **zero errors** and **4 skipped reparse points**. Cache and folder changes prevent direct comparison with the initial measurement below.
- Final version 0.2 executables in `dist/SpaceFinder.exe` and `dist/SpaceFinder-CLI.exe`.
- Fixed click interception by cell text. Clicking the name now selects the row and immediately updates properties and the treemap; double-click expansion was also verified. All 12 tests and Clippy passed again after the fix.

The following sections preserve the initial version's measurements.

## Project folder

A scan of `C:\Users\<user>\Documents\Projs` (anonymized path) with 16 workers:

| Metric | Result |
| --- | ---: |
| Files | 234,544 |
| Subfolders | 24,947 |
| Logical bytes | 18,869,397,446 |
| Scanner time, including aggregation | 3.872 s |
| Read errors | 0 |
| Skipped reparse points | 4 |

Disk, system cache and antivirus affect these results. The folder may change while scanning.

## Independent total verification

In the Cargo source cache (`C:\Users\<user>\.cargo\registry\src`, anonymized path), scans with 1, 4 and 16 workers produced the same **6,852 files**, **1,664 subfolders** and **158,349,421 bytes**. An independent enumeration using `Get-ChildItem -Recurse -File -Force` and `Measure-Object Length -Sum` confirmed the exact file count and byte total.

Times for the three sequential runs: 0.167 s, 0.089 s and 0.070 s. Run order and cache warming prevent attributing the difference solely to parallelism.

## Functional verification

- `cargo test --all-targets`: 7 tests passed, covering totals, Unicode, hidden files, empty folders, invalid paths, files as roots, batches larger than 1,024 entries, cancellation, backpressure, paths longer than 260 characters and junction cycles.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo fmt --check`: passed.
- Optimized build and portable executables generated in `dist/`.
- Interface opened and inspected on Windows; largest-files view checked.
- Export through the native dialog: CSV contained 4,022 nodes, including 3,417 files. The independent sum of file sizes (1,573,604,700 bytes) matched the exported root total.
