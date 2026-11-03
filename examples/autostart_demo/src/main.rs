//! A windowless CLI demonstrating `gpui-autostart`.
//!
//! ```sh
//! cargo run -p autostart_demo -- status
//! cargo run -p autostart_demo -- enable
//! cargo run -p autostart_demo -- disable
//! cargo run -p autostart_demo -- verify   # enable → assert → disable, exit 0/1
//! ```
//!
//! `verify` performs the full round trip with *independent* platform
//! assertions (not just the library's own `status()`): the Windows `Run`
//! registry value is read back through the raw registry API, and on Linux the
//! XDG autostart directory is scanned for a `.desktop` entry launching this
//! executable. On macOS the assertion goes through `SMAppService` status
//! (there is no file footprint to inspect independently).
#![allow(missing_docs)]

use gpui_kit::platform as gpui_platform;
use gpui_kit::App;
use gpui_autostart::Autostart;
use anyhow::{Result, bail};

const APP_NAME: &str = "GPUI Autostart Example";

fn main() {
    let command = std::env::args().nth(1).unwrap_or_else(|| "status".into());
    let mut exit_code = 0;
    gpui_platform::application().run(move |cx: &mut App| {
        if let Err(error) = run(&command, cx) {
            eprintln!("gpui-autostart example failed: {error:#}");
            exit_code = 1;
        }
        cx.quit();
        // `run` returns after the event loop stops; propagate the failure.
        if exit_code != 0 {
            std::process::exit(exit_code);
        }
    });
}

fn run(command: &str, cx: &mut App) -> Result<()> {
    let autostart = Autostart::builder()
        .with_app_name(APP_NAME)
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
        "verify" => verify(&autostart, cx)?,
        other => eprintln!("unknown command `{other}`: expected enable, disable, status or verify"),
    }
    Ok(())
}

/// Round trip: enable → assert enabled through the library *and* through the
/// platform footprint → disable → assert both are gone. Every step prints its
/// verdict; the first failure returns an error (exit code 1).
fn verify(autostart: &Autostart, cx: &mut App) -> Result<()> {
    let exe = std::env::current_exe().expect("current executable path");

    println!("[1/4] enable");
    autostart.enable(cx)?;

    println!("[2/4] assert enabled");
    let status = autostart.status(cx)?;
    if !status.is_enabled() {
        bail!("status after enable is {status:?}, expected enabled");
    }
    assert_platform_footprint(&exe, true)?;

    println!("[3/4] disable");
    autostart.disable(cx)?;

    println!("[4/4] assert disabled");
    let status = autostart.status(cx)?;
    if status.is_enabled() {
        bail!("status after disable is {status:?}, expected disabled");
    }
    assert_platform_footprint(&exe, false)?;

    println!("verify: OK (enable → footprint → disable → clean)");
    Ok(())
}

/// Independent of the library's `status()`: look at the actual platform
/// footprint. `present` selects the expected direction of the assertion.
fn assert_platform_footprint(exe: &std::path::Path, present: bool) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let value = raw_run_value(APP_NAME);
        let contains_exe = value
            .as_deref()
            .map(|v| v.contains(&exe.to_string_lossy().as_ref()))
            .unwrap_or(false);
        match (present, contains_exe) {
            (true, false) => {
                bail!(
                    "expected a `Run` value named {APP_NAME:?} pointing at {exe:?}, found {value:?}"
                );
            }
            (false, true) => {
                bail!(
                    "expected no `Run` value named {APP_NAME:?} after disable, found {value:?}"
                );
            }
            _ => println!("  windows footprint ok: {value:?}"),
        }
    }

    #[cfg(target_os = "linux")]
    {
        let Some(dir) = dirs::config_dir().map(|d| d.join("autostart")) else {
            bail!("cannot resolve the XDG config directory");
        };
        let mut found = None;
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if let Ok(contents) = std::fs::read_to_string(entry.path()) {
                    if contents.contains(&exe.to_string_lossy().as_ref()) {
                        found = Some(entry.path());
                        break;
                    }
                }
            }
        }
        match (present, found.is_some()) {
            (true, false) => {
                bail!(
                    "expected a `.desktop` entry in {} launching {exe:?}",
                    dir.display()
                );
            }
            (false, true) => {
                bail!(
                    "expected no `.desktop` entry after disable, found {}",
                    found
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "none".into())
                );
            }
            _ => println!(
                "  linux footprint ok: {}",
                found
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "none".into())
            ),
        }
    }

    // macOS: the library's status already goes through `SMAppService`, which
    // is the platform footprint; there is no file to inspect independently.
    #[cfg(target_os = "macos")]
    {
        let _ = exe;
        println!("  macos footprint: asserted through SMAppService status");
    }

    Ok(())
}

/// Read the raw `HKCU\...\Run` value named `name` through the registry API,
/// bypassing the library. `None` when the value does not exist.
#[cfg(target_os = "windows")]
fn raw_run_value(name: &str) -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::WIN32_ERROR;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};

    const RUN_SUBKEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";

    let subkey: Vec<u16> = RUN_SUBKEY
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let value_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut buffer = [0u16; 2048];
    let mut length = buffer.len() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value_name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut length),
        )
    };
    if status != WIN32_ERROR(0) {
        return None;
    }
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..end]))
}
