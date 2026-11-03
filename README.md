> 本项目基于 [longbridge/gpui-autostart](https://github.com/longbridge/gpui-autostart) 修改。

[English](README.md) | 简体中文

# gpui-autostart

Native auto-start (launch at login) integration for
[GPUI](https://github.com/zed-industries/zed) applications.

- Registers the application with the platform's native login-item mechanism:
  `SMAppService` (with a `LaunchAgent` fallback) on macOS, the
  `HKCU\...\Run` registry key on Windows, and an XDG autostart `.desktop` file
  on Linux.
- One API for macOS, Windows and Linux; every operation takes the selected
  GPUI flavor's `gpui::App`.
- The `Autostart` handle is not `Send`. Create and use it on the GPUI thread,
  like every other handle into application state.

## Usage

```toml
[dependencies]
gpui-kit = "0.8"
gpui-autostart = { version = "0.1", default-features = false, features = ["gpui-kit"] }
```

```rust
use gpui_autostart::Autostart;

let autostart = Autostart::builder()
    .with_app_name("Mail")
    .with_args(["--minimized"])
    .build()?;

if !autostart.is_enabled(cx)? {
    autostart.enable(cx)?;
}
```

`app_name` names the registration (the registry value name, the `.desktop` file
name, the `LaunchAgent` label). The executable defaults to
`std::env::current_exe()` and must be absolute. `enable` and `disable` are
idempotent: `enable` refreshes the registration with the current settings, and
`disable` succeeds when nothing is registered.

## Selecting a GPUI flavor

| feature | GPUI flavor |
| --- | --- |
| `gpui` (default) | upstream GPUI (Zed) |
| `gpui-kit` | GPUI Kit (`gpui-pre`) |
| `gpui-ce` | GPUI Community Edition |

Disable default features and enable exactly one. The flavor must match the GPUI
the application itself builds against, because every operation takes that
flavor's `gpui::App`.

## Platform mechanisms

| Platform | Mechanism |
| --- | --- |
| macOS | `SMAppService` (macOS 13+); `~/Library/LaunchAgents/<label>.plist` otherwise |
| Windows | `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run` registry value |
| Linux | `$XDG_CONFIG_HOME/autostart/<id>.desktop` |

## Platform notes

- **macOS**: `AutostartStatus::RequiresApproval` means the registration exists
  but the user has not approved the login item yet; guide them to
  System Settings → General → Login Items. `SMAppService` requires a
  code-signed application, and the registration only takes effect at the next
  login. The `LaunchAgent` fallback (used when `SMAppService` is unavailable)
  writes the plist and lets the system load it at the next login; it does not
  run `launchctl`.
- **Windows**: the status reflects the registry entry only. Group policies and
  security software can block startup entries without touching the `Run` key.
- **Linux**: the status is the existence of the `.desktop` file. Disabling the
  entry again from the desktop environment's own settings is not tracked.

## Example

A windowless CLI that enables, disables and reports the registration:

```sh
cargo run -p autostart_demo -- status
cargo run -p autostart_demo -- enable
cargo run -p autostart_demo -- disable
```

## License

Apache-2.0
