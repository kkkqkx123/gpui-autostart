use std::{
    path::{Path, PathBuf},
    rc::Rc,
    thread::{self, ThreadId},
};

use crate::{Error, Result, backend::PlatformAutostart};

/// The registration settings every platform backend receives.
#[derive(Clone, Debug)]
pub(crate) struct AutostartConfig {
    pub(crate) app_name: String,
    pub(crate) exe_path: PathBuf,
    pub(crate) args: Vec<String>,
}

/// The state of the current platform's auto-start registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AutostartStatus {
    /// No auto-start registration exists.
    Disabled,
    /// The application is registered and launches at login.
    Enabled,
    /// macOS: registered, but the user has not approved the login item in
    /// System Settings yet.
    RequiresApproval,
    /// The platform reported a state this crate cannot map. Carries a message
    /// explaining what the platform said.
    Error(String),
}

impl AutostartStatus {
    /// Whether the application currently launches at login.
    ///
    /// [`Self::RequiresApproval`] is not enabled: the system will not launch
    /// the application until the user approves the login item.
    pub fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled)
    }
}

/// A cloneable handle to the platform's auto-start registration.
///
/// `Autostart` is intentionally not `Send`: the registration and the GPUI
/// application state it is used from are bound to the thread on which the
/// handle was built.
#[derive(Clone)]
pub struct Autostart {
    inner: Rc<AutostartInner>,
}

struct AutostartInner {
    thread_id: ThreadId,
    config: AutostartConfig,
    backend: PlatformAutostart,
}

/// Configures and creates an [`Autostart`].
#[must_use]
#[derive(Default)]
pub struct AutostartBuilder {
    app_name: Option<String>,
    exe_path: Option<PathBuf>,
    args: Vec<String>,
}

impl AutostartBuilder {
    /// Sets the name the platform shows for the registration, such as the
    /// registry value name on Windows or the `.desktop` file name on Linux.
    /// Required before [`Self::build`].
    pub fn with_app_name(mut self, app_name: impl Into<String>) -> Self {
        self.app_name = Some(app_name.into());
        self
    }

    /// Sets the executable the platform launches at login. Must be absolute.
    /// Defaults to `std::env::current_exe()`.
    pub fn with_exe_path(mut self, exe_path: impl Into<PathBuf>) -> Self {
        self.exe_path = Some(exe_path.into());
        self
    }

    /// Replaces the arguments the platform passes to the executable at login.
    pub fn with_args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    /// The configured application name.
    pub fn app_name(&self) -> Option<&str> {
        self.app_name.as_deref()
    }

    /// The configured executable path.
    pub fn exe_path(&self) -> Option<&Path> {
        self.exe_path.as_deref()
    }

    /// The configured launch arguments.
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Validates the settings and creates the [`Autostart`] handle.
    pub fn build(self) -> Result<Autostart> {
        let Some(app_name) = self.app_name.filter(|name| !name.trim().is_empty()) else {
            return Err(Error::InvalidConfig(
                "app_name must not be empty".to_owned(),
            ));
        };
        let exe_path = match self.exe_path {
            Some(exe_path) => exe_path,
            None => std::env::current_exe().map_err(Error::native)?,
        };
        if !exe_path.is_absolute() {
            return Err(Error::InvalidConfig(format!(
                "exe_path must be absolute: {}",
                exe_path.display()
            )));
        }
        Ok(Autostart {
            inner: Rc::new(AutostartInner {
                thread_id: thread::current().id(),
                config: AutostartConfig {
                    app_name,
                    exe_path,
                    args: self.args,
                },
                backend: PlatformAutostart::new()?,
            }),
        })
    }
}

impl Autostart {
    /// Starts building an auto-start registration.
    pub fn builder() -> AutostartBuilder {
        AutostartBuilder::default()
    }

    /// Registers the application to launch at login. Idempotent: calling this
    /// again refreshes the registration with the current settings.
    pub fn enable(&self, _cx: &mut gpui::App) -> Result<()> {
        self.check_thread()?;
        self.inner.backend.enable(&self.inner.config)
    }

    /// Removes the registration so the application no longer launches at
    /// login. Idempotent: removing an absent registration succeeds.
    pub fn disable(&self, _cx: &mut gpui::App) -> Result<()> {
        self.check_thread()?;
        self.inner.backend.disable(&self.inner.config)
    }

    /// Returns the state of the registration.
    pub fn status(&self, _cx: &mut gpui::App) -> Result<AutostartStatus> {
        self.check_thread()?;
        self.inner.backend.status(&self.inner.config)
    }

    /// Whether the application currently launches at login.
    ///
    /// See [`AutostartStatus::is_enabled`] for how the platform states map.
    pub fn is_enabled(&self, cx: &mut gpui::App) -> Result<bool> {
        Ok(self.status(cx)?.is_enabled())
    }

    fn check_thread(&self) -> Result<()> {
        if thread::current().id() == self.inner.thread_id {
            Ok(())
        } else {
            Err(Error::WrongThread)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static_assertions::assert_not_impl_any!(Autostart: Send, Sync);

    #[test]
    fn test_autostart_builder() {
        let builder = Autostart::builder();
        assert!(builder.app_name().is_none());
        assert!(builder.exe_path().is_none());
        assert!(builder.args().is_empty());

        let builder = Autostart::builder()
            .with_app_name("Mail")
            .with_exe_path("/opt/mail/mail")
            .with_args(["--minimized", "--hidden"]);
        assert_eq!(builder.app_name(), Some("Mail"));
        assert_eq!(builder.exe_path(), Some(Path::new("/opt/mail/mail")));
        assert_eq!(builder.args(), ["--minimized", "--hidden"]);

        let autostart = builder.build().unwrap();
        assert_eq!(autostart.inner.config.app_name, "Mail");
        assert_eq!(autostart.inner.config.exe_path, Path::new("/opt/mail/mail"));
        assert_eq!(autostart.inner.config.args, ["--minimized", "--hidden"]);
    }

    #[test]
    fn test_autostart_builder_defaults_to_current_exe() {
        let autostart = Autostart::builder().with_app_name("Mail").build().unwrap();
        assert_eq!(
            autostart.inner.config.exe_path,
            std::env::current_exe().unwrap()
        );
        assert!(autostart.inner.config.args.is_empty());
    }

    #[test]
    fn test_autostart_builder_validation() {
        let missing_name = Autostart::builder().with_exe_path("/opt/mail/mail").build();
        assert!(matches!(missing_name, Err(Error::InvalidConfig(_))));

        let blank_name = Autostart::builder().with_app_name("   ").build();
        assert!(matches!(blank_name, Err(Error::InvalidConfig(_))));

        let relative_exe = Autostart::builder()
            .with_app_name("Mail")
            .with_exe_path("mail")
            .build();
        assert!(matches!(relative_exe, Err(Error::InvalidConfig(_))));
    }

    #[test]
    fn test_autostart_status_is_enabled() {
        assert!(AutostartStatus::Enabled.is_enabled());
        assert!(!AutostartStatus::Disabled.is_enabled());
        assert!(!AutostartStatus::RequiresApproval.is_enabled());
        assert!(!AutostartStatus::Error("unknown".into()).is_enabled());
    }
}
