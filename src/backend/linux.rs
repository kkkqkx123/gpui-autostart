use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::{
    Error, Result,
    autostart::{AutostartConfig, AutostartStatus},
};

/// Registers auto-start entries through the XDG autostart directory.
pub(crate) struct LinuxAutostart;

impl LinuxAutostart {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn enable(&self, config: &AutostartConfig) -> Result<()> {
        write_entry(&autostart_dir()?, config)
    }

    pub(crate) fn disable(&self, config: &AutostartConfig) -> Result<()> {
        remove_entry(&autostart_dir()?, config)
    }

    pub(crate) fn status(&self, config: &AutostartConfig) -> Result<AutostartStatus> {
        entry_status(&autostart_dir()?, config)
    }
}

/// The XDG autostart directory, `$XDG_CONFIG_HOME/autostart`.
fn autostart_dir() -> Result<PathBuf> {
    dirs::config_dir()
        .map(|dir| dir.join("autostart"))
        .ok_or_else(|| Error::native_message("cannot resolve the user configuration directory"))
}

fn entry_path(dir: &Path, config: &AutostartConfig) -> PathBuf {
    dir.join(format!("{}.desktop", desktop_file_id(&config.app_name)))
}

fn write_entry(dir: &Path, config: &AutostartConfig) -> Result<()> {
    fs::create_dir_all(dir).map_err(Error::native)?;
    fs::write(entry_path(dir, config), desktop_entry(config)).map_err(Error::native)
}

fn remove_entry(dir: &Path, config: &AutostartConfig) -> Result<()> {
    match fs::remove_file(entry_path(dir, config)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::native(error)),
    }
}

fn entry_status(dir: &Path, config: &AutostartConfig) -> Result<AutostartStatus> {
    let enabled = entry_path(dir, config)
        .try_exists()
        .map_err(Error::native)?;
    Ok(if enabled {
        AutostartStatus::Enabled
    } else {
        AutostartStatus::Disabled
    })
}

/// Builds the `.desktop` entry stored in the autostart directory.
fn desktop_entry(config: &AutostartConfig) -> String {
    let mut exec = quote_exec_arg(&config.exe_path.to_string_lossy());
    for arg in &config.args {
        exec.push(' ');
        exec.push_str(&quote_exec_arg(arg));
    }
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name={}\n\
         Exec={}\n\
         X-GNOME-Autostart-enabled=true\n\
         Hidden=false\n",
        escape_value(&config.app_name),
        exec,
    )
}

/// Sanitizes the application name into a Desktop Entry file identifier.
fn desktop_file_id(app_name: &str) -> String {
    let id: String = app_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();
    if id.is_empty() { "app".to_owned() } else { id }
}

/// Escapes a Desktop Entry string value.
fn escape_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\t' => escaped.push_str("\\t"),
            '\r' => escaped.push_str("\\r"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// Quotes one `Exec` argument per the Desktop Entry specification.
fn quote_exec_arg(arg: &str) -> String {
    const RESERVED: &[char] = &[
        '"', '\'', '\\', '$', '`', '>', '<', '~', '|', '&', ';', '*', '?', '(', ')', '#',
    ];
    if !arg.is_empty()
        && !arg
            .chars()
            .any(|c| c.is_whitespace() || RESERVED.contains(&c))
    {
        return arg.to_owned();
    }
    let mut quoted = String::with_capacity(arg.len() + 2);
    quoted.push('"');
    for c in arg.chars() {
        if matches!(c, '\\' | '`' | '"' | '$') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    quoted
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
    fn test_desktop_entry_contents() {
        let entry = desktop_entry(&config(
            "Mail Client",
            "/opt/mail client/mail",
            &["--minimized", "a\"b", "$HOME"],
        ));
        assert_eq!(
            entry,
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=Mail Client\n\
             Exec=\"/opt/mail client/mail\" --minimized \"a\\\"b\" \"\\$HOME\"\n\
             X-GNOME-Autostart-enabled=true\n\
             Hidden=false\n"
        );
    }

    #[test]
    fn test_desktop_entry_escapes_values() {
        let entry = desktop_entry(&config("Mail\\Client\n2", "/opt/mail/mail", &[]));
        assert!(entry.contains("Name=Mail\\\\Client\\n2\n"));
        assert!(entry.contains("Exec=/opt/mail/mail\n"));
    }

    #[test]
    fn test_desktop_file_id() {
        assert_eq!(desktop_file_id("Mail Client"), "Mail-Client");
        assert_eq!(desktop_file_id("com.example.Mail_2"), "com.example.Mail_2");
        assert_eq!(desktop_file_id(""), "app");
    }

    #[test]
    fn test_entry_lifecycle_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let entry = config("Mail", "/opt/mail/mail", &["--minimized"]);
        assert_eq!(
            entry_status(dir.path(), &entry).unwrap(),
            AutostartStatus::Disabled
        );

        write_entry(dir.path(), &entry).unwrap();
        assert_eq!(
            entry_status(dir.path(), &entry).unwrap(),
            AutostartStatus::Enabled
        );

        // Enabling again refreshes the entry instead of duplicating it.
        let refreshed = config("Mail", "/opt/mail/mail", &["--hidden"]);
        write_entry(dir.path(), &refreshed).unwrap();
        assert_eq!(
            entry_status(dir.path(), &refreshed).unwrap(),
            AutostartStatus::Enabled
        );
        assert!(
            fs::read_to_string(entry_path(dir.path(), &refreshed))
                .unwrap()
                .contains("--hidden")
        );

        remove_entry(dir.path(), &refreshed).unwrap();
        assert_eq!(
            entry_status(dir.path(), &refreshed).unwrap(),
            AutostartStatus::Disabled
        );
        // Disabling again is not an error.
        remove_entry(dir.path(), &refreshed).unwrap();
    }
}
