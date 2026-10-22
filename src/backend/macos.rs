use std::{
    fs, io,
    path::{Path, PathBuf},
};

use objc2::runtime::AnyClass;
use objc2_foundation::NSError;
use objc2_service_management::{SMAppService, SMAppServiceStatus};

use crate::{
    Error, Result,
    autostart::{AutostartConfig, AutostartStatus},
};

/// Registers auto-start entries through `SMAppService`, falling back to a
/// `LaunchAgent` plist on systems without `SMAppService`.
pub(crate) struct MacAutostart {
    launch_agent: bool,
}

impl MacAutostart {
    pub(crate) fn new() -> Self {
        Self {
            // `SMAppService` ships with macOS 13; older systems have the
            // ServiceManagement framework but not this class.
            launch_agent: AnyClass::get(c"SMAppService").is_none(),
        }
    }

    pub(crate) fn enable(&self, config: &AutostartConfig) -> Result<()> {
        if self.launch_agent {
            return write_launch_agent(&launch_agents_dir()?, config);
        }
        // SAFETY: `SMAppService` is available on this system, checked in `new`.
        let service = unsafe { SMAppService::mainAppService() };
        // SAFETY: `service` is a valid `SMAppService` instance.
        match unsafe { service.registerAndReturnError() } {
            Ok(()) => Ok(()),
            Err(error) => {
                // `enable` is idempotent: an app that is already registered or
                // waiting for the user's approval counts as enabled.
                let status = service_status(&service);
                if status == SMAppServiceStatus::Enabled
                    || status == SMAppServiceStatus::RequiresApproval
                {
                    Ok(())
                } else {
                    Err(service_error(&error))
                }
            }
        }
    }

    pub(crate) fn disable(&self, config: &AutostartConfig) -> Result<()> {
        if self.launch_agent {
            return remove_launch_agent(&launch_agents_dir()?, config);
        }
        // SAFETY: `SMAppService` is available on this system, checked in `new`.
        let service = unsafe { SMAppService::mainAppService() };
        // SAFETY: `service` is a valid `SMAppService` instance.
        match unsafe { service.unregisterAndReturnError() } {
            Ok(()) => Ok(()),
            Err(error) => {
                // `disable` is idempotent: a service the system no longer knows
                // about is already unregistered.
                let status = service_status(&service);
                if status == SMAppServiceStatus::NotRegistered
                    || status == SMAppServiceStatus::NotFound
                {
                    Ok(())
                } else {
                    Err(service_error(&error))
                }
            }
        }
    }

    pub(crate) fn status(&self, config: &AutostartConfig) -> Result<AutostartStatus> {
        if self.launch_agent {
            return launch_agent_status(&launch_agents_dir()?, config);
        }
        // SAFETY: `SMAppService` is available on this system, checked in `new`.
        let service = unsafe { SMAppService::mainAppService() };
        Ok(map_service_status(service_status(&service)))
    }
}

/// The current registration state of `service`.
fn service_status(service: &SMAppService) -> SMAppServiceStatus {
    // SAFETY: `service` is a valid `SMAppService` instance.
    unsafe { service.status() }
}

/// Maps `SMAppService`'s state onto [`AutostartStatus`].
fn map_service_status(status: SMAppServiceStatus) -> AutostartStatus {
    if status == SMAppServiceStatus::Enabled {
        AutostartStatus::Enabled
    } else if status == SMAppServiceStatus::RequiresApproval {
        AutostartStatus::RequiresApproval
    } else if status == SMAppServiceStatus::NotRegistered {
        AutostartStatus::Disabled
    } else {
        AutostartStatus::Error("SMAppService: the login item was not found".to_owned())
    }
}

/// Wraps a failed `SMAppService` call with the message the system reported.
fn service_error(error: &NSError) -> Error {
    Error::native_message(error.localizedDescription().to_string())
}

/// The user's `LaunchAgents` directory, `~/Library/LaunchAgents`.
fn launch_agents_dir() -> Result<PathBuf> {
    dirs::home_dir()
        .map(|home| home.join("Library/LaunchAgents"))
        .ok_or_else(|| Error::native_message("cannot resolve the user home directory"))
}

fn entry_path(dir: &Path, config: &AutostartConfig) -> PathBuf {
    dir.join(format!("{}.plist", launch_agent_label(&config.app_name)))
}

fn write_launch_agent(dir: &Path, config: &AutostartConfig) -> Result<()> {
    fs::create_dir_all(dir).map_err(Error::native)?;
    fs::write(entry_path(dir, config), launch_agent_plist(config)).map_err(Error::native)
}

fn remove_launch_agent(dir: &Path, config: &AutostartConfig) -> Result<()> {
    match fs::remove_file(entry_path(dir, config)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::native(error)),
    }
}

fn launch_agent_status(dir: &Path, config: &AutostartConfig) -> Result<AutostartStatus> {
    let enabled = entry_path(dir, config)
        .try_exists()
        .map_err(Error::native)?;
    Ok(if enabled {
        AutostartStatus::Enabled
    } else {
        AutostartStatus::Disabled
    })
}

/// Builds the `LaunchAgent` plist that launches the application at login.
fn launch_agent_plist(config: &AutostartConfig) -> String {
    let arguments = std::iter::once(config.exe_path.to_string_lossy().into_owned())
        .chain(config.args.iter().cloned())
        .map(|argument| format!("<string>{}</string>\n", escape_xml(&argument)))
        .collect::<String>();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
         <key>Label</key>\n\
         <string>{label}</string>\n\
         <key>ProgramArguments</key>\n\
         <array>\n\
         {arguments}\
         </array>\n\
         <key>RunAtLoad</key>\n\
         <true/>\n\
         </dict>\n\
         </plist>\n",
        label = escape_xml(&launch_agent_label(&config.app_name)),
    )
}

/// Sanitizes the application name into a `LaunchAgent` label and file name.
fn launch_agent_label(app_name: &str) -> String {
    let label: String = app_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();
    if label.is_empty() {
        "gpui-autostart".to_owned()
    } else {
        label
    }
}

/// Escapes a plist XML string value.
fn escape_xml(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(c),
        }
    }
    escaped
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
    fn test_launch_agent_plist_contents() {
        let plist = launch_agent_plist(&config(
            "Mail Client",
            "/opt/mail client/mail",
            &["--minimized", "a<b>&'c"],
        ));
        assert_eq!(
            plist,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
             \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n\
             <dict>\n\
             <key>Label</key>\n\
             <string>Mail-Client</string>\n\
             <key>ProgramArguments</key>\n\
             <array>\n\
             <string>/opt/mail client/mail</string>\n\
             <string>--minimized</string>\n\
             <string>a&lt;b&gt;&amp;&apos;c</string>\n\
             </array>\n\
             <key>RunAtLoad</key>\n\
             <true/>\n\
             </dict>\n\
             </plist>\n"
        );
    }

    #[test]
    fn test_launch_agent_label() {
        assert_eq!(launch_agent_label("Mail Client"), "Mail-Client");
        assert_eq!(launch_agent_label("com.example.Mail"), "com.example.Mail");
        assert_eq!(launch_agent_label(""), "gpui-autostart");
        assert_eq!(launch_agent_label("***"), "---");
    }

    #[test]
    fn test_map_service_status() {
        assert_eq!(
            map_service_status(SMAppServiceStatus::Enabled),
            AutostartStatus::Enabled
        );
        assert_eq!(
            map_service_status(SMAppServiceStatus::RequiresApproval),
            AutostartStatus::RequiresApproval
        );
        assert_eq!(
            map_service_status(SMAppServiceStatus::NotRegistered),
            AutostartStatus::Disabled
        );
        assert!(matches!(
            map_service_status(SMAppServiceStatus::NotFound),
            AutostartStatus::Error(_)
        ));
    }

    #[test]
    fn test_launch_agent_lifecycle_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let entry = config("Mail", "/opt/mail/mail", &["--minimized"]);
        assert_eq!(
            launch_agent_status(dir.path(), &entry).unwrap(),
            AutostartStatus::Disabled
        );

        write_launch_agent(dir.path(), &entry).unwrap();
        assert_eq!(
            launch_agent_status(dir.path(), &entry).unwrap(),
            AutostartStatus::Enabled
        );

        // Enabling again refreshes the plist instead of duplicating it.
        let refreshed = config("Mail", "/opt/mail/mail", &["--hidden"]);
        write_launch_agent(dir.path(), &refreshed).unwrap();
        assert_eq!(
            launch_agent_status(dir.path(), &refreshed).unwrap(),
            AutostartStatus::Enabled
        );
        assert!(
            fs::read_to_string(entry_path(dir.path(), &refreshed))
                .unwrap()
                .contains("--hidden")
        );

        remove_launch_agent(dir.path(), &refreshed).unwrap();
        assert_eq!(
            launch_agent_status(dir.path(), &refreshed).unwrap(),
            AutostartStatus::Disabled
        );
        // Disabling again is not an error.
        remove_launch_agent(dir.path(), &refreshed).unwrap();
    }
}
