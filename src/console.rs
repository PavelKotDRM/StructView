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
        ATTACH_PARENT_PROCESS, AttachConsole, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE, SetStdHandle,
    };

    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;

    // SAFETY: вызовы WinAPI без побочных эффектов для памяти процесса;
    // при отсутствии родительской консоли AttachConsole вернёт 0 и мы выходим.
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return;
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
            SetStdHandle(STD_OUTPUT_HANDLE, handle);
            SetStdHandle(STD_ERROR_HANDLE, handle);
        }
    }
}

/// Заглушка для платформ, отличных от Windows.
#[cfg(not(windows))]
pub fn attach_parent_console() {}
