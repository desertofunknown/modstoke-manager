/// Whether a process named one of `names` is running. Errors name `subject` and end with
/// `purpose`, which says what has to wait for a known process status.
#[cfg(windows)]
pub(crate) fn any_running(names: &[&str], subject: &str, purpose: &str) -> Result<bool, String> {
    // This system-wide snapshot does not prevent a later launch.
    use windows::Win32::Foundation::{
        CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE, ERROR_NO_MORE_FILES,
    };
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    struct SnapshotHandle(HANDLE);
    impl Drop for SnapshotHandle {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0); }
        }
    }

    let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if handle == INVALID_HANDLE_VALUE {
        return Err(format!(
            "Could not determine whether {subject} is running: {}",
            std::io::Error::last_os_error()
        ));
    }
    let handle = SnapshotHandle(handle);
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let unreadable = || format!("A running process has an unreadable executable name. {purpose}");
    let mut present = unsafe { Process32FirstW(handle.0, &mut entry) }.as_bool();
    let mut running = false;
    loop {
        if !present {
            let error = unsafe { GetLastError() };
            return if error == ERROR_NO_MORE_FILES {
                Ok(running)
            } else {
                Err(format!(
                    "Could not enumerate running processes (Windows error {}). {purpose}",
                    error.0
                ))
            };
        }
        let end = entry.szExeFile.iter().position(|character| *character == 0)
            .ok_or_else(unreadable)?;
        let name = String::from_utf16(&entry.szExeFile[..end]).map_err(|_| unreadable())?;
        running |= names.iter().any(|candidate| name.eq_ignore_ascii_case(candidate));
        present = unsafe { Process32NextW(handle.0, &mut entry) }.as_bool();
    }
}

#[cfg(not(windows))]
pub(crate) fn any_running(_names: &[&str], _subject: &str, _purpose: &str) -> Result<bool, String> {
    Err("Process checks are supported only on Windows.".into())
}

#[cfg(windows)]
pub(crate) fn is_running() -> Result<bool, String> {
    any_running(
        &["SonsOfTheForestDS.exe"],
        "the dedicated server",
        "Dedicated server changes require a known process status.",
    )
}

#[cfg(not(windows))]
pub(crate) fn is_running() -> Result<bool, String> {
    Err("Dedicated server process checks are supported only on Windows.".into())
}

/// Sends Ctrl+C to a server started with its own console, the shutdown the native server expects.
#[cfg(windows)]
pub(crate) fn interrupt(process_id: u32) -> Result<(), String> {
    use std::sync::Once;
    use windows::Win32::Foundation::BOOL;
    use windows::Win32::System::Console::{
        AttachConsole, FreeConsole, GenerateConsoleCtrlEvent, SetConsoleCtrlHandler, CTRL_BREAK_EVENT, CTRL_C_EVENT,
    };

    // The event reaches every process on the server console, including Manager while it is
    // attached. A handler, unlike the inheritable ignore flag, keeps later children interruptible.
    unsafe extern "system" fn ignore_interrupt(event: u32) -> BOOL {
        BOOL::from(event == CTRL_C_EVENT || event == CTRL_BREAK_EVENT)
    }
    static HANDLER: Once = Once::new();
    HANDLER.call_once(|| unsafe { SetConsoleCtrlHandler(Some(ignore_interrupt), true); });

    let failure = unsafe {
        FreeConsole();
        let failure = if !AttachConsole(process_id).as_bool() {
            Some(std::io::Error::last_os_error())
        } else {
            let failure = (!GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0).as_bool()).then(std::io::Error::last_os_error);
            FreeConsole();
            failure
        };
        // Development builds have a console of their own; reattach to it.
        #[cfg(debug_assertions)]
        AttachConsole(windows::Win32::System::Console::ATTACH_PARENT_PROCESS);
        failure
    };
    match failure {
        Some(error) => Err(format!("Could not send Ctrl+C to the server console: {error}")),
        None => Ok(()),
    }
}

#[cfg(not(windows))]
pub(crate) fn interrupt(_process_id: u32) -> Result<(), String> {
    Err("Stopping the dedicated server is supported only on Windows.".into())
}

pub(crate) fn require_stopped() -> Result<(), String> {
    if is_running()? {
        return Err("Stop the Sons of the Forest dedicated server before making changes.".into());
    }
    Ok(())
}
