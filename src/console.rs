//! # Подключение к консоли родительского процесса (Windows)
//!
//! В release-сборке приложение собирается с подсистемой `windows`, из-за чего
//! у процесса нет консоли и вывод CLI-команд теряется. Этот модуль подключает
//! процесс к консоли родителя и перенаправляет туда стандартные потоки.
//!
//! На не-Windows платформах функция [`attach_parent_console`] ничего не делает.

/// Подключить процесс к консоли родителя (если она есть).
///
/// Вызывать до любого вывода в stdout/stderr.
#[cfg(windows)]
pub fn attach_parent_console() {
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Console::{
        ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE,
        STD_OUTPUT_HANDLE, SetStdHandle,
    };

    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;

    // SAFETY: вызовы WinAPI без побочных эффектов для памяти процесса;
    // при отсутствии родительской консоли AttachConsole вернёт 0 и мы выходим.
    unsafe {
        let streams = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE];
        let redirected = streams.map(|stream| {
            let handle = GetStdHandle(stream);
            is_redirected_handle(handle).then_some(handle)
        });
        if redirected[1].is_some() && redirected[2].is_some() {
            return;
        }
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return;
        }
        // AttachConsole may replace standard handles, including piped stdin.
        for (stream, handle) in streams.into_iter().zip(redirected) {
            if let Some(handle) = handle {
                SetStdHandle(stream, handle);
            }
        }

        let name: Vec<u16> = std::ffi::OsStr::new("CONOUT$")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let handle = CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        );

        if handle != INVALID_HANDLE_VALUE {
            if redirected[1].is_none() {
                SetStdHandle(STD_OUTPUT_HANDLE, handle);
            }
            if redirected[2].is_none() {
                SetStdHandle(STD_ERROR_HANDLE, handle);
            }
        }
    }
}

/// Заглушка для платформ, отличных от Windows.
#[cfg(not(windows))]
pub fn attach_parent_console() {}

#[cfg(windows)]
fn is_redirected_handle(handle: windows_sys::Win32::Foundation::HANDLE) -> bool {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{FILE_TYPE_DISK, FILE_TYPE_PIPE, GetFileType};

    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return false;
    }
    // SAFETY: GetFileType only queries the handle; invalid handles return FILE_TYPE_UNKNOWN.
    matches!(
        unsafe { GetFileType(handle) },
        FILE_TYPE_DISK | FILE_TYPE_PIPE
    )
}

#[cfg(all(test, windows))]
mod tests {
    use super::is_redirected_handle;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;

    #[test]
    fn recognizes_file_redirection_without_treating_missing_handles_as_redirected() {
        let file = tempfile::tempfile().unwrap();
        assert!(is_redirected_handle(file.as_raw_handle()));
        assert!(!is_redirected_handle(std::ptr::null_mut()));
        assert!(!is_redirected_handle(INVALID_HANDLE_VALUE));
    }
}
