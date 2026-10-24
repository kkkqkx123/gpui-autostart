use crate::{
    Result,
    autostart::{AutostartConfig, AutostartStatus},
};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// The identifier the current platform stores an auto-start registration
/// under: the `.desktop` file name on Linux, the `LaunchAgent` label on
/// macOS, and the `Run` registry value name on Windows.
pub(crate) fn registration_id(app_name: &str) -> String {
    #[cfg(target_os = "linux")]
    return linux::registration_id(app_name);
    #[cfg(target_os = "macos")]
    return macos::registration_id(app_name);
    #[cfg(target_os = "windows")]
    return windows::registration_id(app_name);
    #[allow(unreachable_code)]
    app_name.to_owned()
}

/// Dispatches auto-start operations to the current platform.
pub(crate) struct PlatformAutostart {
    #[cfg(target_os = "linux")]
    inner: linux::LinuxAutostart,
    #[cfg(target_os = "macos")]
    inner: macos::MacAutostart,
    #[cfg(target_os = "windows")]
    inner: windows::WindowsAutostart,
}

impl PlatformAutostart {
    pub(crate) fn new() -> Result<Self> {
        #[cfg(target_os = "linux")]
        return Ok(Self {
            inner: linux::LinuxAutostart::new(),
        });
        #[cfg(target_os = "macos")]
        return Ok(Self {
            inner: macos::MacAutostart::new(),
        });
        #[cfg(target_os = "windows")]
        return Ok(Self {
            inner: windows::WindowsAutostart::new(),
        });
        #[allow(unreachable_code)]
        Err(crate::Error::UnsupportedPlatform)
    }

    pub(crate) fn enable(&self, config: &AutostartConfig) -> Result<()> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        return self.inner.enable(config);
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = config;
            Err(crate::Error::UnsupportedPlatform)
        }
    }

    pub(crate) fn disable(&self, config: &AutostartConfig) -> Result<()> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        return self.inner.disable(config);
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = config;
            Err(crate::Error::UnsupportedPlatform)
        }
    }

    pub(crate) fn status(&self, config: &AutostartConfig) -> Result<AutostartStatus> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        return self.inner.status(config);
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = config;
            Err(crate::Error::UnsupportedPlatform)
        }
    }
}
