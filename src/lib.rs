//! Native auto-start (launch at login) integration designed specifically for GPUI.
//!
//! [`Autostart`] registers and unregisters the current application with the
//! platform's native login-item mechanism: `SMAppService` (with a `LaunchAgent`
//! fallback) on macOS, the `HKCU\...\Run` registry key on Windows, and an XDG
//! autostart `.desktop` file on Linux.
//!
//! Select exactly one UI dependency: `gpui` (the default) for upstream GPUI,
//! `gpui-kit` for GPUI Kit's `gpui-pre` types, or `gpui-ce` for GPUI Community
//! Edition. Disable default features when selecting either alternative. The
//! selected flavor must be the same GPUI the application itself builds against,
//! because every operation takes that flavor's `gpui::App`.
//!
//! `Autostart` is not `Send`: create it and call it on the GPUI thread, like
//! every other handle into application state.

#[cfg(any(
    all(feature = "gpui", feature = "gpui-kit"),
    all(feature = "gpui", feature = "gpui-ce"),
    all(feature = "gpui-kit", feature = "gpui-ce"),
))]
compile_error!(
    "features `gpui`, `gpui-kit`, and `gpui-ce` are mutually exclusive; use default-features = false and select exactly one backend"
);

#[cfg(not(any(feature = "gpui", feature = "gpui-kit", feature = "gpui-ce")))]
compile_error!("enable exactly one of the `gpui`, `gpui-kit`, or `gpui-ce` features");

#[cfg(all(feature = "gpui-ce", not(any(feature = "gpui", feature = "gpui-kit"))))]
extern crate gpui_ce_backend as gpui;
#[cfg(all(feature = "gpui-kit", not(feature = "gpui")))]
extern crate gpui_kit_backend as gpui;

#[cfg(any(feature = "gpui", feature = "gpui-kit", feature = "gpui-ce"))]
mod autostart;
#[cfg(any(feature = "gpui", feature = "gpui-kit", feature = "gpui-ce"))]
mod backend;
#[cfg(any(feature = "gpui", feature = "gpui-kit", feature = "gpui-ce"))]
mod error;

#[cfg(any(feature = "gpui", feature = "gpui-kit", feature = "gpui-ce"))]
pub use autostart::{Autostart, AutostartBuilder, AutostartStatus};
#[cfg(any(feature = "gpui", feature = "gpui-kit", feature = "gpui-ce"))]
pub use error::{Error, Result};
