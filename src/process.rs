use kill_tree::blocking::kill_tree;
use sysinfo::System;
use crate::utils::VSCODE_PROCESSES;
use tokio::sync::mpsc;
use crate::zen_garden::ZenEvent;

pub fn terminate_vscode_processes(tx: &mpsc::UnboundedSender<ZenEvent>) {
    for (pid, process) in System::new_all().processes() {
        let cmd = process.cmd().join(" ".as_ref()).to_string_lossy().to_string();
        let name = process.name().to_string_lossy();
        let exe = process.exe().map(|p| p.to_string_lossy().to_lowercase()).unwrap_or_default();

        let name_lower = name.to_lowercase();
        let cmd_lower = cmd.to_lowercase();
        let exe_lower = exe.to_lowercase();

        let is_vscode = VSCODE_PROCESSES.iter().any(|&vs| name.eq_ignore_ascii_case(vs))
            || cmd_lower.contains("vscode")
            || exe_lower.contains("microsoft vs code")
            || exe_lower.contains("visual studio code")
            || name_lower.contains("cursor")
            || name_lower.contains("code-insiders")
            || name_lower.contains("windsurf")
            || name_lower.contains("trae")
            || name_lower.contains("vscodium")
            || exe_lower.contains("/code")
            || exe_lower.contains("\\code.exe")
            || exe_lower.contains("/cursor")
            || exe_lower.contains("\\cursor.exe")
            || (exe_lower.contains("code") && exe_lower.contains("electron"))
            || exe_lower.contains(".app/contents/macos/electron");

        if !is_vscode { continue; }

        let _ = tx.send(ZenEvent::LogMessage(format!("gently guiding {} ({}) to peaceful rest", name, pid)));

        if let Some(parent_pid) = process.parent() {
            let _ = kill_tree(parent_pid.as_u32());
        }
        let _ = kill_tree(pid.as_u32());
    }
}
