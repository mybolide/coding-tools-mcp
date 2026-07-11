use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
};
use windows::Win32::Networking::WinSock::AF_INET;

use crate::error::{AppError, AppResult};

pub fn find_pid_listening_on_port(port: u16) -> AppResult<Option<u32>> {
    let mut size = 0u32;
    unsafe {
        let _ = GetExtendedTcpTable(
            None,
            &mut size,
            false,
            AF_INET.0.into(),
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        );
    }

    let mut buffer = vec![0u8; size as usize];
    let status = unsafe {
        GetExtendedTcpTable(
            Some(buffer.as_mut_ptr().cast()),
            &mut size,
            false,
            AF_INET.0.into(),
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };
    if status != 0 {
        return Err(AppError::Message(format!(
            "GetExtendedTcpTable failed: status={status}"
        )));
    }

    let table = unsafe { &*(buffer.as_ptr() as *const MIB_TCPTABLE_OWNER_PID) };
    let rows = unsafe {
        std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize)
    };

    for row in rows {
        let local_port = local_port_from_dw(row.dwLocalPort);
        if local_port == port && row.dwOwningPid != 0 {
            return Ok(Some(row.dwOwningPid));
        }
    }
    Ok(None)
}

fn local_port_from_dw(dw_local_port: u32) -> u16 {
    ((dw_local_port >> 8) & 0xFFFF) as u16
}
