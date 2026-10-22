use windows::{
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR},
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SAM_FLAGS, REG_SZ,
            RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
        },
    },
    core::PCWSTR,
};

use crate::{
    Error, Result,
    autostart::{AutostartConfig, AutostartStatus},
};

/// The current user's `Run` key. Its values are command lines that Windows
/// launches at every login.
const RUN_SUBKEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";

/// Registers auto-start entries in the `HKCU\...\Run` registry key.
pub(crate) struct WindowsAutostart;

impl WindowsAutostart {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn enable(&self, config: &AutostartConfig) -> Result<()> {
        with_run_key(KEY_SET_VALUE | KEY_QUERY_VALUE, |key| {
            set_value(key, &config.app_name, &command_line(config))
        })
    }

    pub(crate) fn disable(&self, config: &AutostartConfig) -> Result<()> {
        with_run_key(KEY_SET_VALUE | KEY_QUERY_VALUE, |key| {
            delete_value(key, &config.app_name)
        })
    }

    pub(crate) fn status(&self, config: &AutostartConfig) -> Result<AutostartStatus> {
        with_run_key(KEY_QUERY_VALUE, |key| value_status(key, &config.app_name))
    }
}

/// Opens the `Run` key with `access`, hands it to `body`, and closes it again.
fn with_run_key<T>(access: REG_SAM_FLAGS, body: impl FnOnce(HKEY) -> Result<T>) -> Result<T> {
    let subkey = wide(RUN_SUBKEY);
    let mut key = HKEY::default();
    // SAFETY: `subkey` is a NUL-terminated string and `key` receives the opened
    // handle, which is closed below.
    let opened = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            None,
            access,
            &mut key,
        )
    };
    if opened != ERROR_SUCCESS {
        return Err(registry_error("RegOpenKeyExW", opened));
    }

    let result = body(key);
    // SAFETY: `key` was opened above and is not used afterwards.
    unsafe { RegCloseKey(key) };
    result
}

/// Writes `value` under `name`, replacing the previous registration.
fn set_value(key: HKEY, name: &str, value: &str) -> Result<()> {
    let name = wide(name);
    let data = wide_bytes(value);
    // SAFETY: `name` is a NUL-terminated string and `data` holds the `REG_SZ`
    // payload for the duration of the call.
    let status = unsafe { RegSetValueExW(key, PCWSTR(name.as_ptr()), None, REG_SZ, Some(&data)) };
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(registry_error("RegSetValueExW", status))
    }
}

/// Removes the value named `name`. An absent value is not an error, so
/// `disable` stays idempotent.
fn delete_value(key: HKEY, name: &str) -> Result<()> {
    let name = wide(name);
    // SAFETY: `name` is a NUL-terminated string.
    let status = unsafe { RegDeleteValueW(key, PCWSTR(name.as_ptr())) };
    if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        Err(registry_error("RegDeleteValueW", status))
    }
}

/// Reports whether the value named `name` is registered.
fn value_status(key: HKEY, name: &str) -> Result<AutostartStatus> {
    let name = wide(name);
    let mut size = 0_u32;
    // SAFETY: `name` is a NUL-terminated string and `size` receives the size of
    // the registered value. A missing value is reported as ERROR_FILE_NOT_FOUND.
    let status = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(name.as_ptr()),
            None,
            None,
            None,
            Some(&mut size),
        )
    };
    if status == ERROR_SUCCESS {
        Ok(AutostartStatus::Enabled)
    } else if status == ERROR_FILE_NOT_FOUND {
        Ok(AutostartStatus::Disabled)
    } else {
        Err(registry_error("RegQueryValueExW", status))
    }
}

/// Builds the command line stored in the `Run` value.
fn command_line(config: &AutostartConfig) -> String {
    // The executable is always quoted: `Run` values are parsed with the
    // `CreateProcess` heuristics, which split unquoted paths at spaces.
    let mut command = quoted_arg(&config.exe_path.to_string_lossy());
    for arg in &config.args {
        command.push(' ');
        command.push_str(&quote_command_arg(arg));
    }
    command
}

/// Quotes one command-line argument so `CommandLineToArgvW` parses it back as
/// a single argument. Arguments the grammar already keeps together stay
/// unquoted.
fn quote_command_arg(arg: &str) -> String {
    if arg.is_empty() || arg.contains([' ', '\t', '"']) {
        quoted_arg(arg)
    } else {
        arg.to_owned()
    }
}

/// Quotes one command-line argument unconditionally.
fn quoted_arg(arg: &str) -> String {
    let mut quoted = String::with_capacity(arg.len() + 2);
    quoted.push('"');
    let mut backslashes = 0;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            // Backslashes before a quote are doubled and the quote is escaped,
            // so the parser reads them back literally.
            '"' => {
                quoted.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                backslashes = 0;
                quoted.push('"');
            }
            _ => {
                quoted.extend(std::iter::repeat_n('\\', backslashes));
                backslashes = 0;
                quoted.push(c);
            }
        }
    }
    // Backslashes before the closing quote are doubled the same way.
    quoted.extend(std::iter::repeat_n('\\', backslashes * 2));
    quoted.push('"');
    quoted
}

/// Encodes `value` as the NUL-terminated UTF-16 string the registry stores for
/// `REG_SZ` values.
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Encodes `value` like [`wide`], as the little-endian bytes the registry API
/// expects as value data.
fn wide_bytes(value: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    for unit in wide(value) {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

/// Wraps a failed registry call with the system's message for its code.
fn registry_error(api: &str, status: WIN32_ERROR) -> Error {
    Error::native_message(format!(
        "{api} failed: {}",
        std::io::Error::from_raw_os_error(status.0 as i32)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(app_name: &str, exe_path: &str, args: &[&str]) -> AutostartConfig {
        AutostartConfig {
            app_name: app_name.to_owned(),
            exe_path: exe_path.into(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        }
    }

    #[test]
    fn test_command_line_contents() {
        let command = command_line(&config(
            "Mail",
            r"C:\Program Files\Mail\mail.exe",
            &["--minimized", "user name", ""],
        ));
        assert_eq!(
            command,
            r#""C:\Program Files\Mail\mail.exe" --minimized "user name" """#
        );
    }

    #[test]
    fn test_quote_command_arg() {
        // Arguments the grammar keeps together are not quoted, and their
        // backslashes stay literal.
        assert_eq!(quote_command_arg("--minimized"), "--minimized");
        assert_eq!(quote_command_arg(r"C:\mail\mail.exe"), r"C:\mail\mail.exe");
        assert_eq!(quote_command_arg("a\\"), "a\\");

        // Whitespace, quotes and empty arguments are quoted.
        assert_eq!(quote_command_arg(""), "\"\"");
        assert_eq!(quote_command_arg("user name"), "\"user name\"");
        assert_eq!(quote_command_arg("a\"b"), "\"a\\\"b\"");

        // Backslashes only escape when they precede a quote: an embedded quote
        // doubles its backslash run and a trailing one doubles before the
        // closing quote, while backslashes before other characters stay
        // literal.
        assert_eq!(quote_command_arg("a\\\"b"), "\"a\\\\\\\"b\"");
        assert_eq!(quote_command_arg("a\\ b"), "\"a\\ b\"");
        assert_eq!(quote_command_arg("a b\\"), "\"a b\\\\\"");
        assert_eq!(quote_command_arg("\\\""), "\"\\\\\\\"\"");
    }

    #[test]
    fn test_wide_encoding() {
        assert_eq!(wide("a\u{e9}"), vec![b'a' as u16, 0xe9, 0]);
        assert_eq!(wide_bytes("ab"), vec![b'a', 0, b'b', 0, 0, 0]);
    }
}
