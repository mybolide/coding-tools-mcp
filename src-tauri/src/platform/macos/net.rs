use std::mem;
use std::process::Command;

use crate::error::{AppError, AppResult};

#[repr(C)]
struct ProcBsdInfo {
    pbi_flags: u32,
    pbi_status: u32,
    pbi_xstatus: u32,
    pbi_pid: u32,
    pbi_ppid: u32,
    pbi_uid: u32,
    pbi_gid: u32,
    pbi_ruid: u32,
    pbi_rgid: u32,
    pbi_svuid: u32,
    pbi_svgid: u32,
    pbi_rfu: u32,
    pbi_comm: [i8; 17],
    pbi_name: [i8; 33],
    pbi_nfiles: u32,
    pbi_pgid: u32,
    pbi_pjobc: u32,
    e_tdev: u32,
    e_tpgid: u32,
    pbi_nice: i32,
    pbi_start_tvsec: u64,
    pbi_start_tvusec: u64,
}

const PROC_PIDT_SHORTBSDINFO: i32 = 13;

extern "C" {
    fn proc_listallpids(buffer: *mut libc::c_void, buffersize: i32) -> i32;
    fn proc_pidinfo(
        pid: libc::c_int,
        flavor: i32,
        arg: u64,
        buffer: *mut libc::c_void,
        buffersize: i32,
    ) -> i32;
}

pub fn find_pid_listening_on_port(port: u16) -> AppResult<Option<u32>> {
    let output = Command::new("lsof")
        .args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-t"])
        .output()
        .map_err(|err| AppError::Message(format!("lsof failed: {err}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return Ok(None);
    }

    let pid = first_line
        .parse::<u32>()
        .map_err(|err| AppError::Message(format!("invalid lsof pid output: {err}")))?;
    Ok(Some(pid))
}

pub fn parent_pid(pid: u32) -> AppResult<Option<u32>> {
    let mut info = mem::MaybeUninit::<ProcBsdInfo>::uninit();
    let size = unsafe {
        proc_pidinfo(
            pid as i32,
            PROC_PIDT_SHORTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            mem::size_of::<ProcBsdInfo>() as i32,
        )
    };
    if size <= 0 {
        return Ok(None);
    }
    let info = unsafe { info.assume_init() };
    Ok(Some(info.pbi_ppid))
}

pub fn all_pids() -> AppResult<Vec<i32>> {
    let mut buffer = vec![0u8; 32 * 1024];
    let count = unsafe { proc_listallpids(buffer.as_mut_ptr().cast(), buffer.len() as i32) };
    if count <= 0 {
        return Err(AppError::Message("proc_listallpids failed".into()));
    }
    let pids = unsafe {
        std::slice::from_raw_parts(
            buffer.as_ptr().cast::<i32>(),
            count as usize / mem::size_of::<i32>(),
        )
    };
    Ok(pids.iter().copied().filter(|pid| *pid > 0).collect())
}
