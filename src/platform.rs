use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Debug)]
pub struct Entry {
    pub name: OsString,
    pub size: u64,
    pub directory: bool,
    pub reparse: bool,
    pub details: FileDetails,
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct FileDetails {
    pub created: Option<i64>,
    pub modified: Option<i64>,
    pub accessed: Option<i64>,
    pub attributes: u32,
}

pub fn metadata_details(metadata: &std::fs::Metadata) -> FileDetails {
    fn seconds(time: std::io::Result<std::time::SystemTime>) -> Option<i64> {
        time.ok()
            .and_then(|t| match t.duration_since(std::time::UNIX_EPOCH) {
                Ok(d) => i64::try_from(d.as_secs()).ok(),
                Err(e) => i64::try_from(e.duration().as_secs()).ok().map(|s| -s),
            })
    }
    #[cfg(windows)]
    let attributes = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes()
    };
    #[cfg(not(windows))]
    let attributes = u32::from(metadata.permissions().readonly());
    FileDetails {
        created: seconds(metadata.created()),
        modified: seconds(metadata.modified()),
        accessed: seconds(metadata.accessed()),
        attributes,
    }
}

/// Windows FILETIME uses 100 ns ticks since 1601; zero means unavailable.
#[cfg(any(windows, test))]
fn filetime_seconds(high: u32, low: u32) -> Option<i64> {
    let ticks = ((high as u64) << 32) | low as u64;
    (ticks != 0).then(|| (ticks / 10_000_000) as i64 - 11_644_473_600)
}

pub fn attribute_names(attributes: u32) -> String {
    let labels: Vec<_> = [
        (1, "Só leitura"),
        (2, "Oculto"),
        (4, "Sistema"),
        (0x20, "Arquivo"),
        (0x100, "Temporário"),
        (0x200, "Sparse"),
        (0x800, "Comprimido"),
        (0x1000, "Offline"),
        (0x4000, "Encriptado"),
    ]
    .into_iter()
    .filter_map(|(mask, label)| (attributes & mask != 0).then_some(label))
    .collect();
    if labels.is_empty() {
        "Normal".into()
    } else {
        labels.join(", ")
    }
}

#[derive(Clone, Debug)]
pub struct Volume {
    pub path: PathBuf,
    pub total: u64,
    pub free: u64,
}

#[cfg(windows)]
mod native {
    use super::*;
    use std::{
        os::windows::ffi::{OsStrExt, OsStringExt},
        ptr,
    };
    use windows_sys::Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE},
        Storage::FileSystem::*,
        System::WindowsProgramming::DRIVE_FIXED,
    };

    struct SearchHandle(windows_sys::Win32::Foundation::HANDLE);
    impl Drop for SearchHandle {
        fn drop(&mut self) {
            // SAFETY: this guard exclusively owns a valid search handle.
            unsafe {
                FindClose(self.0);
            }
        }
    }

    pub fn read_directory(
        path: &Path,
        cancel: &AtomicBool,
        mut visit: impl FnMut(Entry),
    ) -> io::Result<()> {
        let pattern: Vec<u16> = path
            .join("*")
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        // SAFETY: WIN32_FIND_DATAW is a plain output struct. The path is NUL terminated;
        // the output buffer stays alive throughout each synchronous call.
        unsafe {
            let mut data: WIN32_FIND_DATAW = std::mem::zeroed();
            let mut raw = FindFirstFileExW(
                pattern.as_ptr(),
                FindExInfoBasic,
                (&mut data as *mut WIN32_FIND_DATAW).cast(),
                FindExSearchNameMatch,
                ptr::null(),
                FIND_FIRST_EX_LARGE_FETCH,
            );
            if raw == INVALID_HANDLE_VALUE {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(87) || error.raw_os_error() == Some(50) {
                    raw = FindFirstFileExW(
                        pattern.as_ptr(),
                        FindExInfoBasic,
                        (&mut data as *mut WIN32_FIND_DATAW).cast(),
                        FindExSearchNameMatch,
                        ptr::null(),
                        0,
                    );
                }
            }
            if raw == INVALID_HANDLE_VALUE {
                let error = io::Error::last_os_error();
                return if error.raw_os_error() == Some(ERROR_FILE_NOT_FOUND as i32) {
                    Ok(())
                } else {
                    Err(error)
                };
            }
            let handle = SearchHandle(raw);
            loop {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let length = data
                    .cFileName
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(data.cFileName.len());
                let name = &data.cFileName[..length];
                if name != [46] && name != [46, 46] {
                    visit(Entry {
                        name: OsString::from_wide(name),
                        size: ((data.nFileSizeHigh as u64) << 32) | data.nFileSizeLow as u64,
                        directory: data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0,
                        reparse: data.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0,
                        details: FileDetails {
                            created: filetime_seconds(
                                data.ftCreationTime.dwHighDateTime,
                                data.ftCreationTime.dwLowDateTime,
                            ),
                            modified: filetime_seconds(
                                data.ftLastWriteTime.dwHighDateTime,
                                data.ftLastWriteTime.dwLowDateTime,
                            ),
                            accessed: filetime_seconds(
                                data.ftLastAccessTime.dwHighDateTime,
                                data.ftLastAccessTime.dwLowDateTime,
                            ),
                            attributes: data.dwFileAttributes,
                        },
                    });
                }
                if FindNextFileW(handle.0, &mut data) == 0 {
                    let error = io::Error::last_os_error();
                    if error.raw_os_error() != Some(ERROR_NO_MORE_FILES as i32) {
                        return Err(error);
                    }
                    break;
                }
            }
        }
        Ok(())
    }

    pub fn volumes() -> Vec<Volume> {
        // SAFETY: output pointers refer to initialized, live u64 values and each
        // path buffer is NUL terminated. No handles are retained.
        unsafe {
            let mask = GetLogicalDrives();
            (0..26)
                .filter_map(|index| {
                    if mask & (1 << index) == 0 {
                        return None;
                    }
                    let path = PathBuf::from(format!("{}:\\", (b'A' + index as u8) as char));
                    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
                    // Avoid probing disconnected network volumes and removable media at startup.
                    if GetDriveTypeW(wide.as_ptr()) != DRIVE_FIXED {
                        return None;
                    }
                    let (mut available, mut total, mut free) = (0, 0, 0);
                    if GetDiskFreeSpaceExW(wide.as_ptr(), &mut available, &mut total, &mut free)
                        == 0
                    {
                        return None;
                    }
                    Some(Volume { path, total, free })
                })
                .collect()
        }
    }
}

#[cfg(windows)]
pub use native::{read_directory, volumes};

#[cfg(not(windows))]
pub fn read_directory(
    path: &Path,
    cancel: &AtomicBool,
    mut visit: impl FnMut(Entry),
) -> io::Result<()> {
    for item in std::fs::read_dir(path)? {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let item = item?;
        let metadata = item.path().symlink_metadata()?;
        visit(Entry {
            name: item.file_name(),
            size: metadata.len(),
            directory: metadata.is_dir(),
            reparse: metadata.file_type().is_symlink(),
            details: metadata_details(&metadata),
        });
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn volumes() -> Vec<Volume> {
    Vec::new()
}

pub fn display_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        value.strip_prefix(r"\\?\").unwrap_or(&value).to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filetime_epoch_and_unknown() {
        let ticks = 116_444_736_000_000_000_u64;
        assert_eq!(
            filetime_seconds((ticks >> 32) as u32, ticks as u32),
            Some(0)
        );
        assert_eq!(filetime_seconds(0, 0), None);
        let next = ticks + 10_000_000;
        assert_eq!(filetime_seconds((next >> 32) as u32, next as u32), Some(1));
    }
}
