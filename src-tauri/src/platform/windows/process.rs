use std::mem;

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_TERMINATE,
};

use crate::error::{AppError, AppResult};

pub fn is_process_alive(pid: u32) -> bool {
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let alive = handle != INVALID_HANDLE_VALUE;
        if alive {
            let _ = CloseHandle(handle);
        }
        alive
    }
}

pub fn process_image_path(pid: u32) -> AppResult<Option<String>> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .map_err(|err| AppError::Message(format!("OpenProcess failed: {err}")))?;
        if handle == INVALID_HANDLE_VALUE {
            return Ok(None);
        }

        let mut size = 32_768u32;
        let mut buffer = vec![0u16; size as usize];
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);
        if ok.is_err() {
            return Ok(None);
        }
        buffer.truncate(size as usize);
        Ok(Some(String::from_utf16_lossy(&buffer)))
    }
}

pub fn terminate_process_tree(root_pid: u32) -> AppResult<()> {
    let children = collect_child_pids(root_pid)?;
    for pid in children.into_iter().rev() {
        terminate_pid(pid)?;
    }
    terminate_pid(root_pid)
}

fn collect_child_pids(root_pid: u32) -> AppResult<Vec<u32>> {
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut pending = vec![root_pid];
    let mut seen = std::collections::HashSet::from([root_pid]);
    let mut ordered = Vec::new();

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .map_err(|err| AppError::Message(format!("CreateToolhelp32Snapshot failed: {err}")))?;
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(AppError::Message("invalid process snapshot".into()));
        }

        let mut entry = PROCESSENTRY32W {
            dwSize: mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let parent = entry.th32ParentProcessID;
                let pid = entry.th32ProcessID;
                if pending.contains(&parent) && seen.insert(pid) {
                    ordered.push(pid);
                    pending.push(pid);
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }

    Ok(ordered)
}

fn terminate_pid(pid: u32) -> AppResult<()> {
    unsafe {
        let handle: HANDLE = OpenProcess(PROCESS_TERMINATE, false, pid)
            .map_err(|err| AppError::Message(format!("OpenProcess terminate failed: {err}")))?;
        if handle == INVALID_HANDLE_VALUE {
            return Ok(());
        }
        let result = windows::Win32::System::Threading::TerminateProcess(handle, 1);
        let _ = CloseHandle(handle);
        result.map_err(|err| AppError::Message(format!("TerminateProcess failed: {err}")))?;
        Ok(())
    }
}
