//! A windowless CLI demonstrating `gpui-autostart`.
//!
//! ```sh
//! cargo run --example autostart -- status
//! cargo run --example autostart -- enable
//! cargo run --example autostart -- disable
//! ```
#![allow(missing_docs)]

#[cfg(all(feature = "gpui-kit", not(feature = "gpui")))]
extern crate gpui_kit as gpui;
#[cfg(all(feature = "gpui-kit", not(feature = "gpui")))]
use gpui_kit::platform as gpui_platform;

#[cfg(all(feature = "gpui-ce", not(any(feature = "gpui", feature = "gpui-kit"))))]
extern crate gpui_ce_backend as gpui;
#[cfg(all(feature = "gpui-ce", not(any(feature = "gpui", feature = "gpui-kit"))))]
use gpui_ce_platform as gpui_platform;

use gpui::App;
use gpui_autostart::{Autostart, Result};

fn main() {
    let command = std::env::args().nth(1).unwrap_or_else(|| "status".into());
    gpui_platform::application().run(move |cx: &mut App| {
        if let Err(error) = run(&command, cx) {
            eprintln!("gpui-autostart example failed: {error}");
        }
        cx.quit();
    });
}

fn run(command: &str, cx: &mut App) -> Result<()> {
    let autostart = Autostart::builder()
        .with_app_name("GPUI Autostart Example")
        .with_args(["--minimized"])
        .build()?;
    match command {
        "enable" => {
            autostart.enable(cx)?;
            println!("auto-start enabled");
        }
        "disable" => {
            autostart.disable(cx)?;
            println!("auto-start disabled");
        }
        "status" => {
            let status = autostart.status(cx)?;
            println!(
                "auto-start status: {status:?} (enabled: {})",
                status.is_enabled()
            );
        }
        other => eprintln!("unknown command `{other}`: expected enable, disable or status"),
    }
    Ok(())
}
