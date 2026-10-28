//! User-invoked file actions. Scanning never calls these functions.
use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

/// Internal startup argument for the optional elevated-window handover.
pub const ELEVATION_READY_ARG: &str = "--elevation-ready";

#[derive(Debug, Clone, Copy)]
pub enum ShellAction {
    Open,
    Reveal,
    Properties,
}

#[derive(Debug, Clone, Copy)]
pub enum CleanupTool {
    WindowsRecommendations,
    RecycleBin,
}

#[derive(Debug, PartialEq, Eq)]
struct Stamp {
    directory: bool,
    len: u64,
    created: Option<SystemTime>,
    modified: Option<SystemTime>,
}
impl Stamp {
    fn read(path: &Path) -> Result<Self, String> {
        let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if is_link(&metadata) {
            return Err("O item é uma ligação. Reanalisa a pasta antes de continuar.".into());
        }
        Ok(Self {
            directory: metadata.is_dir(),
            len: if metadata.is_dir() { 0 } else { metadata.len() },
            created: metadata.created().ok(),
            modified: metadata.modified().ok(),
        })
    }
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

#[derive(Debug)]
pub struct RecycleRequest {
    root: PathBuf,
    original: PathBuf,
    path: PathBuf,
    stamp: Stamp,
}
impl RecycleRequest {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn directory(&self) -> bool {
        self.stamp.directory
    }

    /// Resolve and check the complete absolute target before allowing a recursive
    /// shell operation. Never allow the scan root, a volume root or a link escape.
    pub fn prepare(root: &Path, target: &Path) -> Result<Self, String> {
        if !target.is_absolute() {
            return Err("O caminho tem de ser absoluto.".into());
        }
        Self::check_original_path(target)?;
        let stamp = Stamp::read(target)?;
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let path = target.canonicalize().map_err(|e| e.to_string())?;
        Self::check_boundary(&root, &path)?;
        native_recycle_supported(&path)?;
        Ok(Self {
            root,
            original: target.to_path_buf(),
            path,
            stamp,
        })
    }

    fn check_original_path(path: &Path) -> Result<(), String> {
        if path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err("O caminho contém navegação relativa. Reanalisa a pasta.".into());
        }
        // Check before canonicalization: resolving first would hide junctions
        // which redirect to another location inside the same scan root.
        for ancestor in path.ancestors() {
            if is_link(&fs::symlink_metadata(ancestor).map_err(|e| e.to_string())?) {
                return Err(
                    "O caminho contém uma ligação. Reanalisa a pasta antes de continuar.".into(),
                );
            }
        }
        Ok(())
    }

    fn check_boundary(root: &Path, path: &Path) -> Result<(), String> {
        if path == root || path.parent().is_none() || !path.starts_with(root) {
            return Err("Só podes eliminar itens dentro da pasta analisada. A raiz da análise e as unidades estão protegidas.".into());
        }
        for ancestor in path.ancestors() {
            let metadata = fs::symlink_metadata(ancestor).map_err(|e| e.to_string())?;
            if is_link(&metadata) {
                return Err("O caminho passou a conter uma ligação. Reanalisa a pasta.".into());
            }
            if ancestor == root {
                break;
            }
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        Self::check_original_path(&self.original)?;
        Self::check_boundary(&self.root, &self.path)?;
        if self.original.canonicalize().map_err(|e| e.to_string())? != self.path
            || Stamp::read(&self.path)? != self.stamp
        {
            return Err(
                "O item mudou desde a confirmação. Reanalisa e volta a selecioná-lo.".into(),
            );
        }
        native_recycle_supported(&self.path)
    }

    pub fn recycle(self) -> Result<(), String> {
        self.validate()?;
        native_recycle(&self.path)?;
        match fs::symlink_metadata(&self.path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            _ => Err("O Windows não concluiu a operação. O item continua na localização original; reanalisa a pasta.".into()),
        }
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt};
    use windows::{
        Win32::{System::Com::*, UI::Shell::*},
        core::PCWSTR,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation},
        Storage::FileSystem::GetDriveTypeW,
        System::{
            Threading::{
                CreateEventW, EVENT_MODIFY_STATE, GetCurrentProcess, OpenEventW, OpenProcessToken,
                SetEvent, WaitForMultipleObjects, WaitForSingleObject,
            },
            WindowsProgramming::DRIVE_FIXED,
        },
        UI::{
            Shell::{
                SEE_MASK_FLAG_NO_UI, SEE_MASK_INVOKEIDLIST, SEE_MASK_NOASYNC,
                SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
            },
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
    };

    struct Com;
    impl Com {
        fn new() -> Result<Self, String> {
            // SAFETY: every action runs on its own worker thread. This guard
            // balances initialization after all COM objects have been dropped.
            unsafe {
                CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                    .ok()
                    .map_err(|e| e.to_string())?;
            }
            Ok(Self)
        }
    }
    impl Drop for Com {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }

    fn wide(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(Some(0)).collect()
    }

    // The Windows shell expects ordinary paths; keep the original UTF-16 names.
    fn shell_path(path: &Path) -> PathBuf {
        use std::path::{Component, Prefix};
        let mut parts = path.components();
        match parts.next() {
            Some(Component::Prefix(prefix)) => match prefix.kind() {
                Prefix::VerbatimDisk(letter) => {
                    let mut result = PathBuf::from(format!("{}:\\", letter as char));
                    for part in parts {
                        if part != Component::RootDir {
                            result.push(part.as_os_str());
                        }
                    }
                    result
                }
                Prefix::VerbatimUNC(host, share) => {
                    let mut result = PathBuf::from(r"\\");
                    result.push(host);
                    result.push(share);
                    for part in parts {
                        if part != Component::RootDir {
                            result.push(part.as_os_str());
                        }
                    }
                    result
                }
                _ => path.to_path_buf(),
            },
            _ => path.to_path_buf(),
        }
    }

    pub(super) fn supported(path: &Path) -> Result<(), String> {
        let root = path.ancestors().last().ok_or("Unidade inválida")?;
        let root = wide(shell_path(root).as_os_str());
        // SAFETY: the buffer is a live NUL-terminated absolute volume path.
        if unsafe { GetDriveTypeW(root.as_ptr()) } != DRIVE_FIXED {
            return Err("Enviar para a Reciclagem só está disponível em unidades locais fixas. Não é efetuada eliminação permanente.".into());
        }
        Ok(())
    }

    pub(super) fn recycle(path: &Path) -> Result<(), String> {
        let _com = Com::new()?;
        let path = wide(shell_path(path).as_os_str());
        // SAFETY: COM is initialized for this thread, buffers live through the
        // synchronous operation, and all interface references are RAII owned.
        unsafe {
            let operation: IFileOperation =
                CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER)
                    .map_err(|e| e.to_string())?;
            operation
                .SetOperationFlags(
                    FOFX_RECYCLEONDELETE
                        | FOF_ALLOWUNDO
                        | FOF_WANTNUKEWARNING
                        | FOF_NOCONFIRMATION
                        | FOF_NORECURSEREPARSE
                        | FOF_SILENT
                        | FOF_NOERRORUI
                        | FOFX_EARLYFAILURE,
                )
                .map_err(|e| e.to_string())?;
            let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(path.as_ptr()), None)
                .map_err(|e| e.to_string())?;
            operation
                .DeleteItem(&item, None)
                .map_err(|e| e.to_string())?;
            operation.PerformOperations().map_err(|e| e.to_string())?;
            if operation
                .GetAnyOperationsAborted()
                .map_err(|e| e.to_string())?
                .as_bool()
            {
                return Err(
                    "A operação foi cancelada ou não pôde ser concluída pelo Windows.".into(),
                );
            }
        }
        Ok(())
    }

    pub fn is_elevated() -> bool {
        // SAFETY: outputs are valid and the acquired token is closed once.
        unsafe {
            let mut token: HANDLE = std::ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return false;
            }
            let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
            let mut returned = 0;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                (&mut elevation as *mut TOKEN_ELEVATION).cast(),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut returned,
            );
            CloseHandle(token);
            ok != 0 && elevation.TokenIsElevated != 0
        }
    }

    fn execute(verb: &str, path: &Path, parameters: Option<&[u16]>) -> Result<(), String> {
        let _com = Com::new()?;
        let verb = wide(OsStr::new(verb));
        let path = wide(shell_path(path).as_os_str());
        // SAFETY: the struct is initialized, all strings are NUL terminated and
        // remain live until ShellExecuteExW returns. No process handle is requested.
        unsafe {
            let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
            info.fMask = SEE_MASK_INVOKEIDLIST | SEE_MASK_FLAG_NO_UI | SEE_MASK_NOASYNC;
            info.lpVerb = verb.as_ptr();
            info.lpFile = path.as_ptr();
            info.lpParameters = parameters.map_or(std::ptr::null(), |p| p.as_ptr());
            info.nShow = SW_SHOWNORMAL;
            if ShellExecuteExW(&mut info) == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
        }
        Ok(())
    }

    pub fn shell(action: ShellAction, path: &Path) -> Result<(), String> {
        match action {
            ShellAction::Open => execute("open", path, None),
            ShellAction::Properties => execute("properties", path, None),
            ShellAction::Reveal => {
                use std::os::windows::process::CommandExt;
                let mut argument = std::ffi::OsString::from("/select,");
                argument.push(shell_path(path));
                std::process::Command::new("explorer.exe")
                    .arg(argument)
                    .creation_flags(0x08000000)
                    .spawn()
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
        }
    }

    fn quoted_argument(value: &OsStr) -> Vec<u16> {
        let mut result = vec![34];
        let mut slashes = 0;
        for c in value.encode_wide() {
            if c == 92 {
                slashes += 1;
                continue;
            }
            if c == 34 {
                result.extend(std::iter::repeat_n(92, slashes * 2 + 1));
            } else {
                result.extend(std::iter::repeat_n(92, slashes));
            }
            result.push(c);
            slashes = 0;
        }
        result.extend(std::iter::repeat_n(92, slashes * 2));
        result.extend([34, 0]);
        result
    }

    const READY_PREFIX: &str = r"Local\SpaceFinder.ElevationReady.";

    struct OwnedHandle(HANDLE);
    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            // SAFETY: each guard exclusively owns a valid kernel handle.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    struct ReadyEvent {
        name: std::ffi::OsString,
        handle: OwnedHandle,
    }
    impl ReadyEvent {
        fn create() -> Result<Self, String> {
            // SAFETY: CoCreateGuid has no input pointers or COM apartment requirement.
            let guid = unsafe { CoCreateGuid() }.map_err(|e| e.to_string())?;
            let name = std::ffi::OsString::from(format!("{READY_PREFIX}{guid:?}"));
            let encoded = wide(&name);
            // SAFETY: the name is NUL terminated; default security is used and
            // the returned handle is closed by its owner, including on failure.
            let handle = unsafe { CreateEventW(std::ptr::null(), 1, 0, encoded.as_ptr()) };
            if handle.is_null() {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(Self {
                name,
                handle: OwnedHandle(handle),
            })
        }
    }

    fn wait_for_ready(event: &ReadyEvent, process: HANDLE, timeout_ms: u32) -> Result<(), String> {
        let handles = [event.handle.0, process];
        // SAFETY: both handles remain valid until the wait and liveness check
        // return. Closing our process handle never terminates the child process.
        let result = unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, timeout_ms) };
        if result == WAIT_OBJECT_0 && unsafe { WaitForSingleObject(process, 0) } == WAIT_TIMEOUT {
            return Ok(());
        }
        let message = if result == WAIT_TIMEOUT {
            "A nova janela não confirmou que está pronta. A janela atual continua aberta."
        } else if result == WAIT_OBJECT_0 || result == WAIT_OBJECT_0 + 1 {
            "A nova instância terminou antes de ficar pronta. A janela atual continua aberta."
        } else {
            "Não foi possível confirmar a nova janela. A janela atual continua aberta."
        };
        Err(message.into())
    }

    fn signal_ready_event(name: &OsStr) -> Result<(), String> {
        if !name.to_str().is_some_and(|name| {
            name.starts_with(READY_PREFIX) && name.len() < 128 && !name.contains('\0')
        }) {
            return Err("Identificador de abertura inválido.".into());
        }
        let name = wide(name);
        // SAFETY: opens only this attempt's named event with signal-only access.
        let handle = unsafe { OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let handle = OwnedHandle(handle);
        if unsafe { SetEvent(handle.0) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }

    /// Called by the new GUI only after it has painted a frame and verified its token.
    pub fn acknowledge_elevation_ready(name: &OsStr) -> Result<(), String> {
        if !is_elevated() {
            return Err("A nova janela não tem privilégios de administrador.".into());
        }
        signal_ready_event(name)
    }

    pub fn elevate(path: &Path) -> Result<(), String> {
        let _com = Com::new()?;
        let ready = ReadyEvent::create()?;
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut parameters = quoted_argument(shell_path(path).as_os_str());
        parameters.pop();
        parameters.extend(format!(" {ELEVATION_READY_ARG} ").encode_utf16());
        parameters.extend(quoted_argument(&ready.name));
        let executable = wide(shell_path(&executable).as_os_str());
        let verb = wide(OsStr::new("runas"));
        // SAFETY: all strings live through ShellExecuteExW. Request a process
        // handle explicitly; success launching alone does not close the old UI.
        let process = unsafe {
            let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
            info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI | SEE_MASK_NOASYNC;
            info.lpVerb = verb.as_ptr();
            info.lpFile = executable.as_ptr();
            info.lpParameters = parameters.as_ptr();
            info.nShow = SW_SHOWNORMAL;
            if ShellExecuteExW(&mut info) == 0 {
                let error = std::io::Error::last_os_error();
                return Err(if error.raw_os_error() == Some(1223) {
                    "Pedido de administrador cancelado. A janela atual continua aberta.".into()
                } else {
                    error.to_string()
                });
            }
            if info.hProcess.is_null() {
                return Err(
                    "O Windows não confirmou a nova instância. A janela atual continua aberta."
                        .into(),
                );
            }
            OwnedHandle(info.hProcess)
        };
        wait_for_ready(&ready, process.0, 20_000)
    }

    pub fn cleanup_tool(tool: CleanupTool) -> Result<(), String> {
        let uri = match tool {
            CleanupTool::WindowsRecommendations => "ms-settings:storagerecommendations",
            CleanupTool::RecycleBin => "shell:RecycleBinFolder",
        };
        execute("open", Path::new(uri), None)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::{
            os::windows::{io::AsRawHandle, process::CommandExt},
            process::{Child, Command, Stdio},
        };

        fn readiness_child(ready: &ReadyEvent, mode: &str) -> Child {
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "actions::native::tests::readiness_child_fixture",
                ])
                .env("SPACEFINDER_TEST_READY_EVENT", &ready.name)
                .env("SPACEFINDER_TEST_READY_MODE", mode)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(0x08000000)
                .spawn()
                .unwrap()
        }

        #[test]
        #[ignore = "Subprocess fixture, invoked only by readiness tests; never requests elevation"]
        fn readiness_child_fixture() {
            let mode = std::env::var("SPACEFINDER_TEST_READY_MODE").unwrap();
            if mode == "ready" {
                let name = std::env::var_os("SPACEFINDER_TEST_READY_EVENT").unwrap();
                signal_ready_event(&name).unwrap();
            }
            if mode != "exit" {
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }

        #[test]
        fn handover_accepts_only_ready_running_child() {
            let ready = ReadyEvent::create().unwrap();
            let mut child = readiness_child(&ready, "ready");
            assert!(wait_for_ready(&ready, child.as_raw_handle(), 5_000).is_ok());
            assert!(child.wait().unwrap().success());
            // Even a signaled event cannot replace the original UI if the child exited.
            assert!(wait_for_ready(&ready, child.as_raw_handle(), 0).is_err());
        }

        #[test]
        fn handover_rejects_exit_without_readiness() {
            let ready = ReadyEvent::create().unwrap();
            let mut child = readiness_child(&ready, "exit");
            let result = wait_for_ready(&ready, child.as_raw_handle(), 5_000);
            assert!(child.wait().unwrap().success());
            assert!(result.unwrap_err().contains("continua aberta"));
        }

        #[test]
        fn handover_timeout_keeps_original_and_leaves_child_untouched() {
            let ready = ReadyEvent::create().unwrap();
            let mut child = readiness_child(&ready, "idle");
            let result = wait_for_ready(&ready, child.as_raw_handle(), 20);
            assert!(result.unwrap_err().contains("continua aberta"));
            assert!(child.try_wait().unwrap().is_none());
            assert!(child.wait().unwrap().success());
        }

        #[test]
        fn ordinary_process_cannot_acknowledge_elevation() {
            if !is_elevated() {
                let ready = ReadyEvent::create().unwrap();
                assert!(acknowledge_elevation_ready(&ready.name).is_err());
                assert_eq!(
                    unsafe { WaitForSingleObject(ready.handle.0, 0) },
                    WAIT_TIMEOUT
                );
            }
        }

        #[test]
        fn elevation_quotes_drive_roots_and_spaces() {
            assert_eq!(
                String::from_utf16(&quoted_argument(OsStr::new("C:\\"))).unwrap(),
                "\"C:\\\\\"\0"
            );
            assert_eq!(
                String::from_utf16(&quoted_argument(OsStr::new("C:\\a b"))).unwrap(),
                "\"C:\\a b\"\0"
            );
        }
    }
}

#[cfg(windows)]
pub use native::{acknowledge_elevation_ready, cleanup_tool, elevate, is_elevated, shell};
#[cfg(windows)]
use native::{recycle as native_recycle, supported as native_recycle_supported};

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    false
}
#[cfg(not(windows))]
pub fn cleanup_tool(_tool: CleanupTool) -> Result<(), String> {
    Err("A ferramenta está disponível no Windows.".into())
}
#[cfg(not(windows))]
pub fn shell(_action: ShellAction, _path: &Path) -> Result<(), String> {
    Err("Ação disponível no Windows.".into())
}
#[cfg(not(windows))]
pub fn elevate(_path: &Path) -> Result<(), String> {
    Err("Ação disponível no Windows.".into())
}
#[cfg(not(windows))]
pub fn acknowledge_elevation_ready(_name: &std::ffi::OsStr) -> Result<(), String> {
    Err("Ação disponível no Windows.".into())
}
#[cfg(not(windows))]
fn native_recycle_supported(_path: &Path) -> Result<(), String> {
    Err("A Reciclagem está disponível nesta versão apenas no Windows.".into())
}
#[cfg(not(windows))]
fn native_recycle(_path: &Path) -> Result<(), String> {
    Err("Ação disponível no Windows.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> PathBuf {
        let path = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!(
                "recycle-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&path).unwrap();
        path.canonicalize().unwrap()
    }
    #[test]
    fn refuses_roots_escapes_and_changed_files() {
        let root = fixture();
        let file = root.join("sample.txt");
        fs::write(&file, b"sample").unwrap();
        assert!(RecycleRequest::prepare(&root, &root).is_err());
        assert!(RecycleRequest::prepare(&root, root.parent().unwrap()).is_err());
        let request = RecycleRequest::prepare(&root, &file).unwrap();
        fs::write(&file, b"changed content").unwrap();
        assert!(request.validate().is_err());
        fs::remove_file(file).unwrap();
        fs::remove_dir(root).unwrap();
    }
    #[cfg(windows)]
    fn junction(link: &Path, target: &Path) {
        use std::os::windows::process::CommandExt;
        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(crate::platform::display_path(link))
            .arg(crate::platform::display_path(target))
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(windows)]
    #[test]
    fn refuses_junction_ancestor_redirecting_inside_same_root() {
        let root = fixture();
        let real = root.join("real");
        let alias = root.join("alias");
        fs::create_dir(&real).unwrap();
        let sentinel = real.join("sentinel.txt");
        fs::write(&sentinel, b"must remain unchanged").unwrap();
        junction(&alias, &real);

        // The resolved target stays inside the scan root. Boundary checking on
        // the canonical path alone would therefore incorrectly accept it.
        let resolved = alias.join("sentinel.txt").canonicalize().unwrap();
        let result = RecycleRequest::prepare(&root, &alias.join("sentinel.txt"));
        let untouched = fs::read(&sentinel).unwrap();

        // Remove only the junction entry and these explicitly created files.
        fs::remove_dir(&alias).unwrap();
        fs::remove_file(&sentinel).unwrap();
        fs::remove_dir(&real).unwrap();
        fs::remove_dir(&root).unwrap();

        assert!(resolved.starts_with(&root));
        assert!(result.unwrap_err().contains("ligação"));
        assert_eq!(untouched, b"must remain unchanged");
    }

    #[cfg(windows)]
    #[test]
    fn refuses_junction_substitution_after_preparation() {
        let root = fixture();
        let original = root.join("original");
        let moved = root.join("moved");
        fs::create_dir(&original).unwrap();
        fs::write(original.join("sentinel.txt"), b"preserve this file").unwrap();
        let request = RecycleRequest::prepare(&root, &original.join("sentinel.txt")).unwrap();
        request.validate().unwrap();

        // Preserve the actual file and its timestamps, then make the original
        // ancestor a junction to another directory inside this same root.
        assert_eq!(
            original.canonicalize().unwrap().parent(),
            Some(root.as_path())
        );
        assert_eq!(moved.parent().unwrap().canonicalize().unwrap(), root);
        fs::rename(&original, &moved).unwrap();
        junction(&original, &moved);
        let result = request.validate();
        let untouched = fs::read(moved.join("sentinel.txt")).unwrap();

        fs::remove_dir(&original).unwrap();
        fs::remove_file(moved.join("sentinel.txt")).unwrap();
        fs::remove_dir(&moved).unwrap();
        fs::remove_dir(&root).unwrap();

        assert!(result.unwrap_err().contains("ligação"));
        assert_eq!(untouched, b"preserve this file");
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Integration test: sends only a generated fixture to the Windows Recycle Bin"]
    fn recycle_generated_fixture() {
        let root = fixture();
        let folder = root.join("disposable");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("sample.txt"), b"SpaceFinder recycling test").unwrap();
        let request = RecycleRequest::prepare(&root, &folder).unwrap();
        assert!(
            request.path().starts_with(
                std::env::current_dir()
                    .unwrap()
                    .join("target")
                    .canonicalize()
                    .unwrap()
            )
        );
        request.recycle().unwrap();
        assert!(!folder.exists());
        fs::remove_dir(root).unwrap();
    }
}
