#![windows_subsystem = "windows"]

mod keyboard;
mod startup;
mod tray;

use windows::core::Result;

fn run() -> Result<()> {
    let tray = tray::TrayApp::new()?;
    let mut keyboard = keyboard::KeyboardHook::install()?;

    let message_result = tray.run();
    let hook_result = keyboard.uninstall();

    message_result?;
    hook_result
}

fn main() {
    if let Err(error) = run() {
        tray::show_error(&format!("The Missing Ctrl could not start:\n\n{error}"));
        std::process::exit(1);
    }
}
