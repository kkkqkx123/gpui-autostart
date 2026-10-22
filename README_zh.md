> 本项目基于 [longbridge/gpui-autostart](https://github.com/longbridge/gpui-autostart) 修改。

[English](README.md) | 简体中文

# gpui-autostart

为 [GPUI](https://github.com/zed-industries/zed) 应用提供的原生开机自启（登录时启动）集成。

- 通过平台原生登录项机制注册应用：macOS 上使用 `SMAppService`（带 `LaunchAgent` 回退），Windows 上使用 `HKCU\...\Run` 注册表项，Linux 上使用 XDG autostart `.desktop` 文件。
- 一套 API 覆盖 macOS、Windows 和 Linux；每个操作都接受所选 GPUI flavor 的 `gpui::App`。
- `Autostart` 句柄不是 `Send`。请在 GPUI 线程上创建和使用它，与其他进入应用状态的句柄一致。

## 用法

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

`app_name` 是注册项的名称（注册表值名称、`.desktop` 文件名、`LaunchAgent` label）。可执行文件默认取 `std::env::current_exe()`，且必须是绝对路径。`enable` 和 `disable` 是幂等的：`enable` 会以当前设置刷新注册，`disable` 在没有注册项时同样成功。

## 选择 GPUI flavor

| feature | GPUI flavor |
| --- | --- |
| `gpui`（默认） | 上游 GPUI（Zed） |
| `gpui-kit` | GPUI Kit（`gpui-pre`） |
| `gpui-ce` | GPUI Community Edition |

请关闭默认 features 并恰好启用一个。flavor 必须与应用自身构建所用的 GPUI 一致，因为每个操作都接受该 flavor 的 `gpui::App`。

## 平台机制

| 平台 | 机制 |
| --- | --- |
| macOS | `SMAppService`（macOS 13+）；否则使用 `~/Library/LaunchAgents/<label>.plist` |
| Windows | `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run` 注册表值 |
| Linux | `$XDG_CONFIG_HOME/autostart/<id>.desktop` |

## 平台说明

- **macOS**：`AutostartStatus::RequiresApproval` 表示注册存在但用户尚未批准该登录项；请引导用户前往 系统设置 → 通用 → 登录项。`SMAppService` 要求应用经过代码签名，且注册要到下一次登录才生效。`LaunchAgent` 回退（在 `SMAppService` 不可用时使用）只写入 plist 并让系统在下次登录时加载，不会运行 `launchctl`。
- **Windows**：状态仅反映注册表项。组策略和安全软件可以在不触碰 `Run` 键的情况下阻止启动项。
- **Linux**：状态即 `.desktop` 文件是否存在。从桌面环境自身设置中再次禁用该条目不会被跟踪。

## 示例

一个无窗口的 CLI，用于启用、禁用和查询注册状态：

```sh
cargo run --example autostart -- status
cargo run --example autostart -- enable
cargo run --example autostart -- disable
```

## License

Apache-2.0
