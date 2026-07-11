use std::error::Error;
use std::mem::size_of;
use std::sync::OnceLock;

use windows::core::{w, Error as WindowsError, Result, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION,
    NIN_SELECT, NOTIFYICONDATAW, NOTIFYICONDATAW_0, NOTIFYICON_VERSION_4,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIcon, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyMenu, DestroyWindow, DispatchMessageW, GetClassLongPtrW, GetCursorPos, GetMessageW,
    MessageBoxW, PostMessageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW,
    SetForegroundWindow, TrackPopupMenu, TranslateMessage, GCLP_HICON, HICON, HMENU, MB_ICONERROR,
    MB_OK, MF_CHECKED, MF_GRAYED, MF_SEPARATOR, MF_STRING, MSG, TPM_NONOTIFY, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, WINDOW_EX_STYLE, WM_APP, WM_CONTEXTMENU, WM_DESTROY, WM_NULL, WNDCLASSW,
    WS_OVERLAPPED,
};

use crate::startup;

const WINDOW_CLASS: PCWSTR = w!("TheMissingCtrlWindow");
const WINDOW_TITLE: PCWSTR = w!("The Missing Ctrl");
const TRAY_CALLBACK_MESSAGE: u32 = WM_APP + 1;
const TRAY_ICON_ID: u32 = 1;
const NIN_KEYSELECT: u32 = NIN_SELECT + 1;
const MENU_STARTUP: usize = 1001;
const MENU_EXIT: usize = 1002;

const ICON_BACKGROUND: [u16; 16] = [
    0x0ff0, 0x3ffc, 0x7ffe, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff,
    0xffff, 0x7ffe, 0x3ffc, 0x0ff0,
];
const ICON_GLYPH: [u16; 16] = [
    0x0000, 0x0000, 0x0000, 0x0000, 0x07e0, 0x0c00, 0x0c00, 0x0c00, 0x0c00, 0x0c00, 0x07e0, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000,
];

type AppResult<T> = std::result::Result<T, Box<dyn Error>>;

pub struct TrayApp {
    hwnd: HWND,
    icon: HICON,
}

impl TrayApp {
    pub fn new() -> Result<Self> {
        let icon = create_tray_icon()?;
        let hwnd = match create_hidden_window(icon) {
            Ok(hwnd) => hwnd,
            Err(error) => {
                unsafe {
                    let _ = DestroyIcon(icon);
                }
                return Err(error);
            }
        };

        if let Err(error) = add_tray_icon(hwnd, icon) {
            unsafe {
                let _ = DestroyWindow(hwnd);
                let _ = DestroyIcon(icon);
            }
            return Err(error);
        }

        Ok(Self { hwnd, icon })
    }

    pub fn run(&self) -> Result<()> {
        let mut message = MSG::default();

        loop {
            let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
            match result.0 {
                -1 => return Err(WindowsError::from_win32()),
                0 => return Ok(()),
                _ => unsafe {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                },
            }
        }
    }
}

impl Drop for TrayApp {
    fn drop(&mut self) {
        let _ = remove_tray_icon(self.hwnd);
        unsafe {
            let _ = DestroyIcon(self.icon);
        }
    }
}

pub fn show_error(message: &str) {
    let message = wide_string(message);
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(message.as_ptr()),
            WINDOW_TITLE,
            MB_OK | MB_ICONERROR,
        );
    }
}

fn create_hidden_window(icon: HICON) -> Result<HWND> {
    let module = unsafe { GetModuleHandleW(None)? };
    let instance = HINSTANCE::from(module);
    let window_class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        hIcon: icon,
        lpszClassName: WINDOW_CLASS,
        ..Default::default()
    };

    if unsafe { RegisterClassW(&window_class) } == 0 {
        return Err(WindowsError::from_win32());
    }

    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            WINDOW_CLASS,
            WINDOW_TITLE,
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            instance,
            None,
        )
    }
}

fn add_tray_icon(hwnd: HWND, icon: HICON) -> Result<()> {
    let mut data = tray_icon_data(hwnd, icon);
    unsafe {
        Shell_NotifyIconW(NIM_ADD, &data).ok()?;
    }

    data.Anonymous = NOTIFYICONDATAW_0 {
        uVersion: NOTIFYICON_VERSION_4,
    };
    if let Err(error) = unsafe { Shell_NotifyIconW(NIM_SETVERSION, &data).ok() } {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &data);
        }
        return Err(error);
    }

    Ok(())
}

fn remove_tray_icon(hwnd: HWND) -> Result<()> {
    let data = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ICON_ID,
        ..Default::default()
    };
    unsafe { Shell_NotifyIconW(NIM_DELETE, &data).ok() }
}

fn tray_icon_data(hwnd: HWND, icon: HICON) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ICON_ID,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: TRAY_CALLBACK_MESSAGE,
        hIcon: icon,
        ..Default::default()
    };
    copy_wide_string(&mut data.szTip, "The Missing Ctrl");
    data
}

fn create_tray_icon() -> Result<HICON> {
    let mut and_mask = [0u8; 32];
    let mut color_bitmap = [0u8; 16 * 16 * 4];

    for (row, background) in ICON_BACKGROUND.iter().enumerate() {
        let [high, low] = (!background).to_be_bytes();
        and_mask[row * 2] = high;
        and_mask[row * 2 + 1] = low;
    }

    for source_row in (0..16).rev() {
        let bitmap_row = 15 - source_row;
        for column in 0..16 {
            let bit = 1 << (15 - column);
            let pixel = (bitmap_row * 16 + column) * 4;

            if ICON_GLYPH[source_row] & bit != 0 {
                color_bitmap[pixel..pixel + 4].copy_from_slice(&[0xff, 0xff, 0xff, 0x00]);
            } else if ICON_BACKGROUND[source_row] & bit != 0 {
                color_bitmap[pixel..pixel + 4].copy_from_slice(&[0xeb, 0x63, 0x25, 0x00]);
            }
        }
    }

    unsafe {
        CreateIcon(
            None,
            16,
            16,
            1,
            32,
            and_mask.as_ptr(),
            color_bitmap.as_ptr(),
        )
    }
}

extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == taskbar_created_message() {
        let icon = HICON(unsafe { GetClassLongPtrW(hwnd, GCLP_HICON) } as *mut _);
        if let Err(error) = add_tray_icon(hwnd, icon) {
            show_error(&format!("Could not restore the tray icon:\n\n{error}"));
        }
        return LRESULT(0);
    }

    match message {
        TRAY_CALLBACK_MESSAGE => {
            let event = lparam.0 as u32 & 0xffff;
            if event == WM_CONTEXTMENU || event == NIN_SELECT || event == NIN_KEYSELECT {
                if let Err(error) = show_context_menu(hwnd) {
                    show_error(&format!("The tray menu failed:\n\n{error}"));
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            let _ = remove_tray_icon(hwnd);
            unsafe {
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn show_context_menu(hwnd: HWND) -> AppResult<()> {
    let startup_enabled = startup::is_enabled().ok();
    let menu = PopupMenu(unsafe { CreatePopupMenu()? });

    let mut startup_flags = MF_STRING;
    let startup_label = match startup_enabled {
        Some(true) => {
            startup_flags |= MF_CHECKED;
            w!("Start with Windows")
        }
        Some(false) => w!("Start with Windows"),
        None => {
            startup_flags |= MF_GRAYED;
            w!("Start with Windows (unavailable)")
        }
    };

    unsafe {
        AppendMenuW(menu.0, startup_flags, MENU_STARTUP, startup_label)?;
        AppendMenuW(menu.0, MF_SEPARATOR, 0, PCWSTR::null())?;
        AppendMenuW(menu.0, MF_STRING, MENU_EXIT, w!("Exit"))?;
    }

    let mut cursor = POINT::default();
    unsafe {
        GetCursorPos(&mut cursor)?;
        let _ = SetForegroundWindow(hwnd);
    }

    let command = unsafe {
        TrackPopupMenu(
            menu.0,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
            cursor.x,
            cursor.y,
            0,
            hwnd,
            None,
        )
        .0 as usize
    };

    unsafe {
        PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0))?;
    }

    match command {
        MENU_STARTUP => {
            if let Some(startup_enabled) = startup_enabled {
                startup::set_enabled(!startup_enabled)?;
            }
        }
        MENU_EXIT => unsafe {
            DestroyWindow(hwnd)?;
        },
        _ => {}
    }

    Ok(())
}

fn taskbar_created_message() -> u32 {
    static MESSAGE: OnceLock<u32> = OnceLock::new();
    *MESSAGE.get_or_init(|| unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) })
}

fn copy_wide_string(destination: &mut [u16], value: &str) {
    let character_limit = destination.len().saturating_sub(1);
    for (slot, character) in destination
        .iter_mut()
        .take(character_limit)
        .zip(value.encode_utf16())
    {
        *slot = character;
    }
}

fn wide_string(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

struct PopupMenu(HMENU);

impl Drop for PopupMenu {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyMenu(self.0);
        }
    }
}
