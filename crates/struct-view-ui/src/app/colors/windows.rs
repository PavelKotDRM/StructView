use std::sync::{Arc, Mutex, OnceLock, mpsc};

use egui::{Color32, Context, Id};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
    Graphics::Gdi::{
        BeginPaint, BitBlt, CAPTUREBLT, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC,
        DeleteObject, EndPaint, GetDC, GetPixel, HBITMAP, HDC, HGDIOBJ, PAINTSTRUCT, ReleaseDC,
        SRCCOPY, SelectObject,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext},
    UI::WindowsAndMessaging::{
        CREATESTRUCTW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
        GWLP_USERDATA, GetCursorPos, GetMessageW, GetSystemMetrics, GetWindowLongPtrW, IDC_CROSS,
        LoadCursorW, MSG, RegisterClassW, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
        SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_SHOW, SetForegroundWindow, SetWindowLongPtrW,
        ShowWindow, TranslateMessage, WA_INACTIVE, WM_ACTIVATE, WM_CLOSE, WM_ERASEBKGND,
        WM_KEYDOWN, WM_LBUTTONUP, WM_NCCREATE, WM_PAINT, WM_RBUTTONUP, WNDCLASSW, WS_EX_TOOLWINDOW,
        WS_EX_TOPMOST, WS_POPUP,
    },
};

type PickerResult = Result<Option<Color32>, String>;

#[derive(Clone, Default)]
pub(super) struct ScreenPickerState {
    receiver: Option<Arc<Mutex<mpsc::Receiver<PickerResult>>>>,
    pub(super) error: Option<String>,
}

pub(super) fn start_screen_capture(ctx: &Context, _id: Id) -> ScreenPickerState {
    let (sender, receiver) = mpsc::channel();
    let ctx = ctx.clone();
    match std::thread::Builder::new()
        .name("screen-color-picker".to_string())
        .spawn(move || {
            // The receiver can disappear when the edited field or application closes.
            let _ = sender.send(pick_color());
            ctx.request_repaint();
        }) {
        Ok(_) => ScreenPickerState {
            receiver: Some(Arc::new(Mutex::new(receiver))),
            error: None,
        },
        Err(error) => ScreenPickerState {
            receiver: None,
            error: Some(error.to_string()),
        },
    }
}

pub(super) fn show_eyedropper(ctx: &Context, id: Id) -> Option<Option<Color32>> {
    let mut state = ctx.data(|data| data.get_temp::<ScreenPickerState>(id))?;
    let receiver = state.receiver.as_ref()?;
    let result = match receiver.lock() {
        Ok(receiver) => match receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
                return None;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Screen color picker stopped unexpectedly".to_string())
            }
        },
        Err(_) => Err("Screen color picker worker failed".to_string()),
    };
    match result {
        Ok(selection) => Some(selection),
        Err(error) => {
            state.receiver = None;
            state.error = Some(error);
            ctx.data_mut(|data| data.insert_temp(id, state));
            ctx.request_repaint();
            None
        }
    }
}

const CLASS_NAME: windows_sys::core::PCWSTR = windows_sys::core::w!("StructViewColorPicker");

fn api_error(operation: &str) -> String {
    format!("{operation}: {}", std::io::Error::last_os_error())
}

struct Picker {
    desktop: HDC,
    snapshot: HDC,
    bitmap: HBITMAP,
    previous_bitmap: HGDIOBJ,
    window: HWND,
    origin: POINT,
    width: i32,
    height: i32,
    outcome: Option<PickerResult>,
}

impl Drop for Picker {
    fn drop(&mut self) {
        // All handles belong to the picker thread and are released on that thread.
        unsafe {
            if !self.window.is_null() {
                DestroyWindow(self.window);
            }
            if !self.previous_bitmap.is_null() {
                SelectObject(self.snapshot, self.previous_bitmap);
            }
            if !self.bitmap.is_null() {
                DeleteObject(self.bitmap);
            }
            if !self.snapshot.is_null() {
                DeleteDC(self.snapshot);
            }
            if !self.desktop.is_null() {
                ReleaseDC(std::ptr::null_mut(), self.desktop);
            }
        }
    }
}

fn pick_color() -> PickerResult {
    // This worker uses physical virtual-desktop coordinates, including mixed-DPI monitors.
    if unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) }.is_null()
    {
        return Err(api_error("SetThreadDpiAwarenessContext"));
    }
    let mut picker = capture_snapshot()?;
    picker.create_window()?;
    unsafe {
        ShowWindow(picker.window, SW_SHOW);
        if SetForegroundWindow(picker.window) == 0 {
            return Err("Could not focus the screen color picker".to_string());
        }
        let mut message = MSG::default();
        while picker.outcome.is_none() {
            match GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) {
                -1 => return Err(api_error("GetMessageW")),
                0 => return Ok(None),
                _ => {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        }
        picker.outcome.take().expect("picker outcome is available")
    }
}

fn capture_snapshot() -> Result<Box<Picker>, String> {
    unsafe {
        let mut picker = Box::new(Picker {
            desktop: GetDC(std::ptr::null_mut()),
            snapshot: std::ptr::null_mut(),
            bitmap: std::ptr::null_mut(),
            previous_bitmap: std::ptr::null_mut(),
            window: std::ptr::null_mut(),
            origin: POINT {
                x: GetSystemMetrics(SM_XVIRTUALSCREEN),
                y: GetSystemMetrics(SM_YVIRTUALSCREEN),
            },
            width: GetSystemMetrics(SM_CXVIRTUALSCREEN),
            height: GetSystemMetrics(SM_CYVIRTUALSCREEN),
            outcome: None,
        });
        if picker.desktop.is_null() {
            return Err(api_error("GetDC"));
        }
        if picker.width <= 0 || picker.height <= 0 {
            return Err("No screens are available".to_string());
        }
        picker.snapshot = CreateCompatibleDC(picker.desktop);
        if picker.snapshot.is_null() {
            return Err(api_error("CreateCompatibleDC"));
        }
        picker.bitmap = CreateCompatibleBitmap(picker.desktop, picker.width, picker.height);
        if picker.bitmap.is_null() {
            return Err(api_error("CreateCompatibleBitmap"));
        }
        picker.previous_bitmap = SelectObject(picker.snapshot, picker.bitmap);
        if picker.previous_bitmap.is_null() {
            return Err(api_error("SelectObject"));
        }
        if BitBlt(
            picker.snapshot,
            0,
            0,
            picker.width,
            picker.height,
            picker.desktop,
            picker.origin.x,
            picker.origin.y,
            SRCCOPY | CAPTUREBLT,
        ) == 0
        {
            return Err(api_error("Capture screen"));
        }
        Ok(picker)
    }
}

impl Picker {
    fn create_window(&mut self) -> Result<(), String> {
        static REGISTERED: OnceLock<Result<(), String>> = OnceLock::new();
        REGISTERED
            .get_or_init(|| unsafe {
                let cursor = LoadCursorW(std::ptr::null_mut(), IDC_CROSS);
                if cursor.is_null() {
                    return Err(api_error("LoadCursorW"));
                }
                let class = WNDCLASSW {
                    lpfnWndProc: Some(window_proc),
                    hInstance: GetModuleHandleW(std::ptr::null()),
                    hCursor: cursor,
                    lpszClassName: CLASS_NAME,
                    ..Default::default()
                };
                if RegisterClassW(&class) == 0 {
                    return Err(api_error("RegisterClassW"));
                }
                Ok(())
            })
            .clone()?;
        self.window = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                CLASS_NAME,
                CLASS_NAME,
                WS_POPUP,
                self.origin.x,
                self.origin.y,
                self.width,
                self.height,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()),
                (self as *mut Self).cast(),
            )
        };
        if self.window.is_null() {
            return Err(api_error("CreateWindowExW"));
        }
        Ok(())
    }

    fn color_at(&self, point: POINT) -> Result<Color32, String> {
        let x = i64::from(point.x) - i64::from(self.origin.x);
        let y = i64::from(point.y) - i64::from(self.origin.y);
        if x < 0 || y < 0 || x >= i64::from(self.width) || y >= i64::from(self.height) {
            return Err("Selected point is outside the screen snapshot".to_string());
        }
        let pixel = unsafe { GetPixel(self.snapshot, x as i32, y as i32) };
        if pixel == u32::MAX {
            Err(api_error("Read selected screen pixel"))
        } else {
            Ok(color_from_colorref(pixel))
        }
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(lparam as *const CREATESTRUCTW);
            SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let pointer = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut Picker;
        if pointer.is_null() {
            return DefWindowProcW(window, message, wparam, lparam);
        }
        let picker = &mut *pointer;
        match message {
            WM_PAINT => {
                let mut paint = PAINTSTRUCT::default();
                let dc = BeginPaint(window, &mut paint);
                if BitBlt(
                    dc,
                    0,
                    0,
                    picker.width,
                    picker.height,
                    picker.snapshot,
                    0,
                    0,
                    SRCCOPY,
                ) == 0
                {
                    picker.outcome = Some(Err(api_error("Paint screen snapshot")));
                }
                EndPaint(window, &paint);
                0
            }
            WM_ERASEBKGND => 1,
            WM_LBUTTONUP => {
                let mut point = POINT::default();
                picker.outcome = Some(if GetCursorPos(&mut point) == 0 {
                    Err(api_error("GetCursorPos"))
                } else {
                    picker.color_at(point).map(Some)
                });
                0
            }
            WM_CLOSE | WM_RBUTTONUP => {
                picker.outcome = Some(Ok(None));
                0
            }
            WM_KEYDOWN if wparam == 0x1B => {
                picker.outcome = Some(Ok(None));
                0
            }
            WM_ACTIVATE if wparam & 0xffff == WA_INACTIVE as usize => {
                picker.outcome = Some(Ok(None));
                0
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

fn color_from_colorref(pixel: u32) -> Color32 {
    Color32::from_rgb(pixel as u8, (pixel >> 8) as u8, (pixel >> 16) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gdi_pixel_channels_are_converted_to_opaque_rgb() {
        assert_eq!(
            color_from_colorref(0x00332211),
            Color32::from_rgb(17, 34, 51)
        );
        assert_eq!(color_from_colorref(0), Color32::BLACK);
        assert_eq!(color_from_colorref(0x00ffffff), Color32::WHITE);
    }

    #[test]
    fn native_snapshot_reads_pixels_with_negative_monitor_coordinates() {
        let mut picker = capture_snapshot().expect("capture the Windows desktop");
        picker.origin = POINT { x: -1920, y: -1080 };
        let pixel = unsafe {
            windows_sys::Win32::Graphics::Gdi::SetPixel(picker.snapshot, 12, 34, 0x00332211)
        };
        assert_ne!(pixel, u32::MAX);
        assert_eq!(
            picker.color_at(POINT { x: -1908, y: -1046 }),
            Ok(Color32::from_rgb(17, 34, 51))
        );
        assert!(picker.color_at(POINT { x: -1921, y: -1080 }).is_err());
        assert!(
            picker
                .color_at(POINT {
                    x: picker.origin.x + picker.width,
                    y: picker.origin.y,
                })
                .is_err()
        );
    }

    #[test]
    fn native_window_handles_selection_and_cancellation_without_becoming_visible() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{IsWindowVisible, SendMessageW};

        let mut picker = capture_snapshot().unwrap();
        picker.create_window().unwrap();
        assert_eq!(unsafe { IsWindowVisible(picker.window) }, 0);
        let mut point = POINT::default();
        assert_ne!(unsafe { GetCursorPos(&mut point) }, 0);
        let expected = picker.color_at(point).unwrap();
        unsafe { SendMessageW(picker.window, WM_LBUTTONUP, 0, 0) };
        assert_eq!(picker.outcome.take(), Some(Ok(Some(expected))));
        for (message, key) in [(WM_KEYDOWN, 0x1B), (WM_RBUTTONUP, 0), (WM_CLOSE, 0)] {
            unsafe { SendMessageW(picker.window, message, key, 0) };
            assert_eq!(picker.outcome.take(), Some(Ok(None)));
        }
        assert_eq!(unsafe { IsWindowVisible(picker.window) }, 0);
    }

    #[test]
    fn worker_returns_selection_and_cancellation_without_egui_viewports() {
        for selection in [Some(Color32::from_rgb(17, 34, 51)), None] {
            let ctx = Context::default();
            let id = Id::unique("native-picker-result");
            let (sender, receiver) = mpsc::channel();
            sender.send(Ok(selection)).unwrap();
            ctx.data_mut(|data| {
                data.insert_temp(
                    id,
                    ScreenPickerState {
                        receiver: Some(Arc::new(Mutex::new(receiver))),
                        error: None,
                    },
                );
            });
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                assert_eq!(show_eyedropper(ui.ctx(), id), Some(selection));
            });
            output.textures_delta.clear();
            assert_eq!(output.viewport_output.len(), 1);
            assert!(output.viewport_output.contains_key(&egui::ViewportId::ROOT));
        }
    }

    #[test]
    fn worker_errors_remain_visible_to_the_color_editor() {
        let ctx = Context::default();
        let id = Id::unique("native-picker-error");
        let (sender, receiver) = mpsc::channel();
        sender.send(Err("Capture failed".to_string())).unwrap();
        ctx.data_mut(|data| {
            data.insert_temp(
                id,
                ScreenPickerState {
                    receiver: Some(Arc::new(Mutex::new(receiver))),
                    error: None,
                },
            );
        });
        assert_eq!(show_eyedropper(&ctx, id), None);
        let state = ctx
            .data(|data| data.get_temp::<ScreenPickerState>(id))
            .unwrap();
        assert_eq!(state.error.as_deref(), Some("Capture failed"));
        assert!(state.receiver.is_none());
    }
}
