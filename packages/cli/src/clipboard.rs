use std::io::Write;
use std::process::{Command, Stdio};

/// Write text to system clipboard.
///
/// Strategy:
/// 1. OSC 52 via crossterm `CopyToClipboard`（工作于 SSH）
/// 2. 平台原生工具 fallback（`wl-copy` / `xclip` / `osascript`）
pub fn write(text: &str) {
    // crossterm OSC 52
    {
        use crossterm::clipboard::CopyToClipboard;
        use crossterm::execute;
        let _ = execute!(std::io::stdout(), CopyToClipboard::to_clipboard_from(text));
    }

    // 平台原生 fallback
    write_native(text);
}

/// Write text via native OS clipboard tools.
fn write_native(text: &str) {
    let _result = if cfg!(target_os = "macos") {
        macos_write(text)
    } else if cfg!(target_os = "linux") {
        linux_write(text)
    } else if cfg!(target_os = "windows") {
        windows_write(text)
    } else {
        Ok(())
    };
}

/// 从系统剪贴板读取文本。
pub fn read() -> Option<String> {
    let result: Result<String, String> = if cfg!(target_os = "macos") {
        macos_read()
    } else if cfg!(target_os = "linux") {
        linux_read()
    } else if cfg!(target_os = "windows") {
        windows_read()
    } else {
        Err("unsupported platform".into())
    };
    match result {
        Ok(s) => Some(s),
        Err(e) => {
            tracing::debug!("clipboard read failed: {e}");
            None
        }
    }
}

fn macos_write(text: &str) -> Result<(), String> {
    let mut child = Command::new("osascript")
        .arg("-e")
        .arg(format!("set the clipboard to \"{}\"", text.replace('"', "\\\"")))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn osascript: {e}"))?;
    child.wait().map_err(|e| format!("wait osascript: {e}"))?;
    Ok(())
}

fn macos_read() -> Result<String, String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg("the clipboard as text")
        .output()
        .map_err(|e| format!("spawn osascript: {e}"))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn linux_write(text: &str) -> Result<(), String> {
    if linux_write_tool(text, "wl-copy", &["--trim-newline"]).is_ok() {
        return Ok(());
    }
    if linux_write_tool(text, "xclip", &["-selection", "clipboard"]).is_ok() {
        return Ok(());
    }
    Err("no clipboard tool found (try wl-clipboard or xclip)".into())
}

fn linux_write_tool(text: &str, bin: &str, args: &[&str]) -> Result<(), String> {
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn {bin}: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes()).map_err(|e| format!("write stdin: {e}"))?;
    }
    child.wait().map_err(|e| format!("wait {bin}: {e}"))?;
    Ok(())
}

fn linux_read() -> Result<String, String> {
    for bin in &["wl-paste", "xclip"] {
        let args: &[&str] = if *bin == "xclip" {
            &["-selection", "clipboard", "-o"]
        } else {
            &[]
        };
        if let Ok(out) = Command::new(bin)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
        {
            if out.status.success() {
                return Ok(String::from_utf8_lossy(&out.stdout).trim().to_string());
            }
        }
    }
    Err("no clipboard read tool found".into())
}

fn windows_write(text: &str) -> Result<(), String> {
    let mut child = Command::new("powershell.exe")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(format!("[System.Windows.Forms.Clipboard]::SetText('{}')", text.replace('\'', "''")))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn powershell: {e}"))?;
    child.wait().map_err(|e| format!("wait powershell: {e}"))?;
    Ok(())
}

fn windows_read() -> Result<String, String> {
    let output = Command::new("powershell.exe")
        .arg("-NoProfile")
        .arg("-Command")
        .arg("Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.Clipboard]::GetText()")
        .output()
        .map_err(|e| format!("spawn powershell: {e}"))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
