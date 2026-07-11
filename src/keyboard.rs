use std::cell::Cell;
use std::mem::size_of;

use windows::core::Result;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_CONTROL, VK_RETURN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT,
    LLKHF_INJECTED, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

const TAP_THRESHOLD_MS: u32 = 500;

#[derive(Clone, Copy, Default)]
struct KeyboardState {
    enter_down: bool,
    enter_down_time: u32,
    combo_key: bool,
}

thread_local! {
    static STATE: Cell<KeyboardState> = const { Cell::new(KeyboardState {
        enter_down: false,
        enter_down_time: 0,
        combo_key: false,
    }) };
}

enum Action {
    ControlDown,
    ControlUp,
    EnterTap,
}

pub struct KeyboardHook {
    handle: Option<HHOOK>,
}

impl KeyboardHook {
    pub fn install() -> Result<Self> {
        let handle = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0)? };
        Ok(Self {
            handle: Some(handle),
        })
    }

    pub fn uninstall(&mut self) -> Result<()> {
        release_pressed_control();

        if let Some(handle) = self.handle.take() {
            unsafe { UnhookWindowsHookEx(handle) }
        } else {
            Ok(())
        }
    }
}

impl Drop for KeyboardHook {
    fn drop(&mut self) {
        let _ = self.uninstall();
    }
}

extern "system" fn hook_proc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if ncode != HC_ACTION as i32 {
        return unsafe { CallNextHookEx(None, ncode, wparam, lparam) };
    }

    let hook = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    if hook.flags.contains(LLKHF_INJECTED) {
        return unsafe { CallNextHookEx(None, ncode, wparam, lparam) };
    }

    let (block, action) = STATE.with(|cell| {
        let mut state = cell.get();
        let result = match wparam.0 as u32 {
            WM_SYSKEYDOWN | WM_KEYDOWN => {
                if hook.vkCode == u32::from(VK_RETURN.0) {
                    if !state.enter_down {
                        state.enter_down = true;
                        state.enter_down_time = hook.time;
                    }
                    (true, None)
                } else if state.enter_down && !state.combo_key {
                    state.combo_key = true;
                    (false, Some(Action::ControlDown))
                } else {
                    (false, None)
                }
            }
            WM_SYSKEYUP | WM_KEYUP if hook.vkCode == u32::from(VK_RETURN.0) => {
                state.enter_down = false;
                if state.combo_key {
                    state.combo_key = false;
                    (false, Some(Action::ControlUp))
                } else if hook.time.wrapping_sub(state.enter_down_time) < TAP_THRESHOLD_MS {
                    (false, Some(Action::EnterTap))
                } else {
                    (false, None)
                }
            }
            _ => (false, None),
        };

        cell.set(state);
        result
    });

    if let Some(action) = action {
        send_action(action);
    }
    if block {
        return LRESULT(1);
    }

    unsafe { CallNextHookEx(None, ncode, wparam, lparam) }
}

fn send_action(action: Action) {
    match action {
        Action::ControlDown => send_inputs(&[keyboard_input(VK_CONTROL, false)]),
        Action::ControlUp => send_inputs(&[keyboard_input(VK_CONTROL, true)]),
        Action::EnterTap => send_inputs(&[
            keyboard_input(VK_RETURN, false),
            keyboard_input(VK_RETURN, true),
        ]),
    }
}

fn keyboard_input(key: VIRTUAL_KEY, key_up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: if key_up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send_inputs(inputs: &[INPUT]) {
    unsafe {
        SendInput(inputs, size_of::<INPUT>() as i32);
    }
}

fn release_pressed_control() {
    let should_release = STATE.with(|state| {
        let previous = state.replace(KeyboardState::default());
        previous.combo_key
    });

    if should_release {
        send_action(Action::ControlUp);
    }
}
