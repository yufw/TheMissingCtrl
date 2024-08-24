use core::str;
use std::mem::transmute;

use windows::core::Result;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

static mut ENTER_DOWN: bool = false;
static mut ENTER_DOWN_TIME: u32 = 0;
static mut COMBO_KEY: bool = false;

extern "system" fn hookproc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let hook_struct: &KBDLLHOOKSTRUCT = transmute(lparam);
        if hook_struct.flags.contains(LLKHF_INJECTED) {
            println!("injected key");
            return CallNextHookEx(None, ncode, wparam, lparam);
        }

        if ncode == HC_ACTION as i32 {
            match wparam.0 as u32 {
                WM_SYSKEYDOWN | WM_KEYDOWN => {
                    if hook_struct.vkCode == VK_RETURN.0 as u32 {
                        if !ENTER_DOWN {
                            ENTER_DOWN = true;
                            ENTER_DOWN_TIME = hook_struct.time;
                        }
                        return LRESULT(1);
                    } else if ENTER_DOWN {
                        COMBO_KEY = true;
                        send_input(vec![Input(VK_CONTROL, false)]);
                    }
                    println!("key name: {}", get_key_name(hook_struct));
                }
                WM_SYSKEYUP | WM_KEYUP => {
                    if hook_struct.vkCode == VK_RETURN.0 as u32 {
                        ENTER_DOWN = false;
                        if COMBO_KEY {
                            send_input(vec![Input(VK_CONTROL, true)]);
                            COMBO_KEY = false;
                        } else if hook_struct.time - ENTER_DOWN_TIME < 500 {
                            send_input(vec![Input(VK_RETURN, false), Input(VK_RETURN, true)]);
                        }
                    }
                }
                _ => {}
            }
        }

        return CallNextHookEx(None, ncode, wparam, lparam);
    }
}

fn get_key_name(hook_struct: &KBDLLHOOKSTRUCT) -> String {
    let mut msg = 1;
    msg += hook_struct.scanCode << 16;
    msg += hook_struct.flags.0 << 24;
    unsafe {
        let mut lpstring: [u8; 100] = [0; 100];
        GetKeyNameTextA(msg as i32, &mut lpstring);
        let res = str::from_utf8(&lpstring).unwrap();
        return res.to_owned();
    }
}

struct Input(VIRTUAL_KEY, bool);
fn send_input(inputs: Vec<Input>) {
    let mut input_vec: Vec<INPUT> = vec![];

    for k in inputs.iter() {
        let Input(vk, is_up) = *k;
        input_vec.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if is_up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });
    }

    unsafe {
        SendInput(&input_vec, size_of::<INPUT>() as i32);
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
