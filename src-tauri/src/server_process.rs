#[cfg(windows)]
pub(crate) fn is_running() -> Result<bool, String> {
    // This system-wide snapshot does not prevent a later server launch.
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
            "Could not determine whether the dedicated server is running: {}",
            std::io::Error::last_os_error()
        ));
    }
    let handle = SnapshotHandle(handle);
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut present = unsafe { Process32FirstW(handle.0, &mut entry) }.as_bool();
    let mut running = false;
    loop {
        if !present {
            let error = unsafe { GetLastError() };
            return if error == ERROR_NO_MORE_FILES {
                Ok(running)
            } else {
                Err(format!(
                    "Could not enumerate running processes (Windows error {}). Dedicated server changes require a known process status.",
                    error.0
                ))
            };
        }
        let end = entry.szExeFile.iter().position(|character| *character == 0)
            .ok_or("A running process has an unreadable executable name. Dedicated server changes require a known process status.")?;
        let name = String::from_utf16(&entry.szExeFile[..end])
            .map_err(|_| "A running process has an unreadable executable name. Dedicated server changes require a known process status.")?;
        running |= name.eq_ignore_ascii_case("SonsOfTheForestDS.exe");
        present = unsafe { Process32NextW(handle.0, &mut entry) }.as_bool();
    }
}

#[cfg(not(windows))]
pub(crate) fn is_running() -> Result<bool, String> {
    Err("Dedicated server process checks are supported only on Windows.".into())
}

pub(crate) fn require_stopped() -> Result<(), String> {
    if is_running()? {
        return Err("Stop the Sons of the Forest dedicated server before making changes.".into());
    }
    Ok(())
}
