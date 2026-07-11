use std::io;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;

use windows::core::w;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
};

const RUN_KEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_NAME: windows::core::PCWSTR = w!("TheMissingCtrl");

pub fn is_enabled() -> io::Result<bool> {
    let mut byte_count = 0;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            VALUE_NAME,
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut byte_count),
        )
    };

    if status == ERROR_FILE_NOT_FOUND {
        return Ok(false);
    }
    check_status(status)?;

    let mut value = vec![0u16; (byte_count as usize + 1) / 2];
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            VALUE_NAME,
            RRF_RT_REG_SZ,
            None,
            Some(value.as_mut_ptr().cast()),
            Some(&mut byte_count),
        )
    };
    check_status(status)?;

    value.truncate((byte_count as usize / 2).min(value.len()));
    while value.last() == Some(&0) {
        value.pop();
    }

    Ok(value == startup_command()?)
}

pub fn set_enabled(enabled: bool) -> io::Result<()> {
    if enabled {
        let mut command = startup_command()?;
        command.push(0);
        let byte_count = command
            .len()
            .checked_mul(size_of::<u16>())
            .and_then(|count| u32::try_from(count).ok())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "startup path is too long")
            })?;

        let status = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                VALUE_NAME,
                REG_SZ.0,
                Some(command.as_ptr().cast()),
                byte_count,
            )
        };
        check_status(status)
    } else {
        let status = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE_NAME) };
        if status == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            check_status(status)
        }
    }
}

fn startup_command() -> io::Result<Vec<u16>> {
    let executable = std::env::current_exe()?;
    let mut command = Vec::new();
    command.push(u16::from(b'"'));
    command.extend(executable.as_os_str().encode_wide());
    command.push(u16::from(b'"'));
    Ok(command)
}

fn check_status(status: WIN32_ERROR) -> io::Result<()> {
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status.0 as i32))
    }
}
