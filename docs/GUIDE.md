# SpaceFinder — detailed guide

A native Rust disk space explorer for Windows. English interface, local processing, and no server, browser or JavaScript runtime.

## Interface

The sidebar provides **Overview**, **Explorer**, **Largest files**, **File types** and **Recommendations**. The summary distinguishes logical scanned size from physical drive usage and shows the largest folders. Tables, headers, selection properties and the treemap support detailed inspection; recommendations have a filterable list and an evidence panel with cautions and review actions.

## Recommendations

The recommendation engine uses metadata already collected by the scan and runs in the background without reading file contents or deleting data. Each report is tied to its source scan and discarded when another scan begins.

- **Manage with Windows:** temporary files in standard locations, the Recycle Bin and `Windows.old`, when found. The action opens the Windows tool for review; it does not run cleanup.
- **Can be recreated:** Cargo and Next.js build output with matching manifests and output markers. Confirm that there are no manual files, the project can be rebuilt and no build/server is using those files. Build folders may contain final binaries. A folder named `target` or `.next` alone is not enough.
- **Personal review:** files of at least 512 MiB in personal locations and downloads of at least 128 MiB that have not been modified for at least 90 days. Size and age do not prove that a file can be deleted. Last-access time is not used as evidence of disuse.

System folders, managed installations, `.git` and `node_modules` are excluded from direct-deletion suggestions. Cloud locations and incomplete branches require additional review. Suggestions do not identify duplicates without comparing contents. Sizes are logical and avoid double-counting a recommended folder and its descendants. Up to 200 priority suggestions are shown; totals represent the suggestions actually listed.

**Recommendations never clean up automatically.** **Review in tree** selects the item for inspection without running it. Deletion remains an explicit explorer-menu action with confirmation. Windows tools open for the user to choose what to clean up; permissions and system settings are not changed automatically.

```powershell
.\dist\SpaceFinder-CLI.exe --scan 'C:\' --json --recommendations
```

With `--recommendations`, CLI JSON includes paths, categories, observed sizes, reasons and cautions. The option also works with text output.

Criteria references: [Microsoft — free up space](https://support.microsoft.com/en-us/windows/experience/storage-filemanagement/free-up-drive-space-in-windows), [Storage Sense](https://support.microsoft.com/en-us/windows/experience/storage-filemanagement/manage-drive-space-with-storage-sense), [Cargo clean](https://doc.rust-lang.org/cargo/commands/cargo-clean.html), [Next.js — build output](https://nextjs.org/docs/app/api-reference/config/next-config-js/distDir).

## Permissions and actions

**Administrator is optional.** The app works in standard mode. **Options → Run as administrator** asks Windows for authorization and opens a new window with the same path. Once the new window confirms administrator privileges and has rendered its interface, the previous window closes automatically. Canceling the request, failure to start or failure to confirm readiness keeps the current window open. Elevation may allow more folders to be scanned. Some locations, such as parts of `System Volume Information` and `WindowsApps`, may remain protected. File permissions and ownership are not changed.

The status bar distinguishes access-denied folders, other errors and skipped links. A banner warns when the scan is incomplete. Affected branches have an indicator and `*` beside the size: this is only what could be measured, not a confirmed zero. **Coverage and permissions** filters the first 100 issues, copies a report and explains sizes and dates.

The table and treemap context menus include:

- Open an item, reveal/select it in File Explorer, open its parent folder and view native Windows properties.
- Focus, expand/collapse or rescan a folder.
- Copy name, path, parent folder path and exact byte count.
- **Move to Recycle Bin**, with in-app confirmation; folders require typing their name.

File actions run in the background. The scan root and drives cannot be deleted through this menu. The app checks the absolute path, scan boundaries, links and item changes before the operation. Recycling is limited to fixed local drives and uses the native Windows operation, with no fallback to permanent deletion. Windows may refuse or cancel the action.

After recycling, the app rescans and returns to the parent folder. **Free space may not increase until the Recycle Bin is emptied.** CSV includes a tenth column, `incomplete`; CLI JSON includes `denied` in statistics and structured issues with a path, reason and category.

References: [Windows Shell and administrator privileges](https://learn.microsoft.com/en-us/windows/win32/shell/launch) and [native file operations](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifileoperation-setoperationflags).

## Detailed browsing

- Expandable tree with hierarchy, sizes and percentage bars relative to the parent folder. Click the arrow or double-click to expand/collapse.
- Resizable columns; sort by name, size, files, subfolders, last modified and last accessed. Click the header again to reverse the order.
- Optional created, accessed, attributes and full-path columns under **Columns**. Horizontal scrolling when the table does not fit.
- Properties with exact bytes, branch contents, direct items, average per file, percentage of parent/scan, dates, attributes and path.
- **File types**, grouped by extension, with counts, total/average size and percentages. Double-click an extension to view its largest files.
- Search across every tree level while retaining ancestors of matches. Filters change visible rows; totals still represent the original scan.
- Expansion levels 1/2/3, folders-only mode, branch focus and navigation history.
- Auto/Bytes/KiB/MiB/GiB units, compact rows and optional resizable treemap/properties panels.
- **Ctrl+F** to search, **F5** to rescan and **Esc** to stop.

Dates and attributes come from the same native enumeration, with no extra call per file. Branch modified/accessed dates are the latest value across the folder and all scanned descendants; created is the folder's own date. Windows may delay or disable access-time updates. CSV includes Unix timestamps in UTC seconds and numeric Windows attributes. CLI JSON includes these details for the largest entries.

References: [TreeSize columns](https://manuals.jam-software.com/treesize_free/EN/available_columns.html) and [WIN32_FIND_DATAW metadata](https://learn.microsoft.com/en-us/windows/win32/api/minwinbase/ns-minwinbase-win32_find_dataw).

## Launching

The portable executable is `dist/SpaceFinder.exe` after running `./build.ps1`. Double-click it, select a drive, enter a path or use **Choose a folder**.

```powershell
cargo run --release
cargo run --release -- 'C:\Users'
```

- Background folder/drive scanning with progress statistics.
- Folders and files sorted by size, a treemap and double-click navigation.
- Top 1,000 files view, name filter and context menu for copying paths/opening File Explorer.
- Cancellation with partial results (button or Esc).
- UTF-8 CSV export including every item and folder totals.
- Capacity/free-space information for fixed local drives.

## Scanning engine

On Windows, the engine uses `FindFirstFileExW` with `FindExInfoBasic` and `FIND_FIRST_EX_LARGE_FETCH`, followed by `FindNextFileW`. Sizes come directly from enumeration: it does not open every file or read its contents. Multiple threads enumerate folders in parallel. Result queues are bounded, batches contain at most 1,024 entries and totals are aggregated in an O(n) pass. The UI list is virtualized.

API reference: [Microsoft — FindFirstFileExW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-findfirstfileexw). Local measurements and checks are in [VALIDATION.md](../VALIDATION.md).

Worker count is adjustable. The default depends on CPU count, capped at 16. SSDs often benefit from parallelism; HDDs may benefit from 1–2 workers. Results depend on disk, cache, antivirus, folder count and permissions. There is no direct MFT reading or requirement to run as administrator.

## Accuracy and coverage

Sizes are **logical**, in bytes (binary KiB/MiB/GiB units), not physical allocated space. Compressed/sparse files may occupy less space. Hard links are counted per path. Drive capacity/free space comes from Windows and may therefore differ from enumerated totals.

Hidden and system entries are included when access is permitted. Reparse points, including junctions, symbolic links and some cloud placeholders, are skipped to avoid cycles and crossing into other volumes. The app shows skipped-item and error counts, with the first 100 issues available through the status bar. Permission-denied folders and interrupted scans produce partial results. The disk may change during reading; this is not a transactional snapshot.

Scanning reads metadata only. Explicit menu actions can open items or move them to the Recycle Bin after confirmation.

## Command line and measurement

```powershell
.\dist\SpaceFinder-CLI.exe --scan 'C:\Users' --threads 8
.\dist\SpaceFinder-CLI.exe --scan 'C:\Users' --threads 8 --json
```

JSON includes the path, workers, bytes, files, folders, seconds, errors, skipped entries, extensions and the 30 largest root entries. Compare worker counts on the same path and account for caching; a folder scan does not predict whole-drive scanning time.

## Building and testing

Requires stable Rust and MSVC build tools for Windows.

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
./build.ps1
```

The scanner also has a portable `read_dir` implementation for other systems. The delivered application and File Explorer integration target Windows.
