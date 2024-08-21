use std::mem::transmute;

use windows::core::Result;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;

extern "system" fn hookproc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let hook_struct: &KBDLLHOOKSTRUCT = transmute(lparam);
        if hook_struct.flags.contains(LLKHF_INJECTED) {
            println!("injected key");
            return CallNextHookEx(None, ncode, wparam, lparam);
        }

        println!("keyboard hook");

        LRESULT(0)
    }
}

fn main() -> Result<()> {
    unsafe {
        let hook = SetWindowsHookExA(WH_KEYBOARD_LL, Some(hookproc), None, 0)?;

        let mut message = MSG::default();
        while GetMessageA(&mut message, None, 0, 0).into() {
            _ = TranslateMessage(&message);
            DispatchMessageA(&message);
        }

        UnhookWindowsHookEx(hook)?;

        Ok(())
    }
}
