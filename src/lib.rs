//! Native auto-start (launch at login) integration designed specifically for GPUI.
//!
//! [`Autostart`] registers and unregisters the current application with the
//! platform's native login-item mechanism: `SMAppService` (with a `LaunchAgent`
//! fallback) on macOS, the `HKCU\...\Run` registry key on Windows, and an XDG
//! autostart `.desktop` file on Linux.
//!
//! Select exactly one GPUI flavor: `gpui-kit` (GPUI Kit's `gpui-pre` types,
//! the default) or `gpui-fast` (GPUI Fast). Disable default features when
//! selecting the alternative. The selected flavor must be the same GPUI the
//! application itself builds against, because every operation takes that
//! flavor's `gpui::App`.
//!
//! `Autostart` is not `Send`: create it and call it on the GPUI thread, like
//! every other handle into application state.

#[cfg(all(feature = "gpui-kit", feature = "gpui-fast"))]
compile_error!(
    "features `gpui-kit` and `gpui-fast` are mutually exclusive; use default-features = false and select exactly one backend"
);

#[cfg(not(any(feature = "gpui-kit", feature = "gpui-fast")))]
compile_error!("enable exactly one of the `gpui-kit` or `gpui-fast` features");

#[cfg(all(feature = "gpui-kit", not(feature = "gpui-fast")))]
extern crate gpui_kit_backend as gpui;
#[cfg(all(feature = "gpui-fast", not(feature = "gpui-kit")))]
extern crate gpui_fast_backend as gpui;

#[cfg(any(feature = "gpui-kit", feature = "gpui-fast"))]
mod autostart;
#[cfg(any(feature = "gpui-kit", feature = "gpui-fast"))]
mod backend;
#[cfg(any(feature = "gpui-kit", feature = "gpui-fast"))]
mod error;

#[cfg(any(feature = "gpui-kit", feature = "gpui-fast"))]
pub use autostart::{Autostart, AutostartBuilder, AutostartStatus};
#[cfg(any(feature = "gpui-kit", feature = "gpui-fast"))]
pub use error::{Error, Result};
