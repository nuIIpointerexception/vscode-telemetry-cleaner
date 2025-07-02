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

        let is_vscode = VSCODE_PROCESSES.iter().any(|&vs| name.eq_ignore_ascii_case(vs))
            || cmd.contains("vscode")
            || exe.contains("microsoft vs code")
            || exe.contains("cursor")
            || exe.contains("code-insiders")
            || exe.contains("windsurf")
            || exe.contains("trae")
            || (exe.contains("code") && exe.contains("electron"));

        if !is_vscode { continue; }

        let _ = tx.send(ZenEvent::LogMessage(format!("gently guiding {} ({}) to peaceful rest", name, pid)));

        if let Some(parent_pid) = process.parent() {
            let _ = kill_tree(parent_pid.as_u32());
        }
        let _ = kill_tree(pid.as_u32());
    }
}
