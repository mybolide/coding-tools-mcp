use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, SetTcpEntry, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID,
    MIB_TCP_STATE_DELETE_TCB, MIB_TCP_STATE_LISTEN, TCP_TABLE_OWNER_PID_LISTENER,
};
use windows::Win32::Networking::WinSock::AF_INET;

use crate::error::{AppError, AppResult};

/// Layout-compatible with `MIB_TCPROW` for `SetTcpEntry`.
#[repr(C)]
struct TcpRow {
    dw_state: u32,
    dw_local_addr: u32,
    dw_local_port: u32,
    dw_remote_addr: u32,
    dw_remote_port: u32,
}

/// Windows stores TCP ports in network byte order inside `dwLocalPort`.
fn local_port_from_dw(dw_local_port: u32) -> u16 {
    ((dw_local_port >> 8) & 0xFF) as u16 | ((dw_local_port & 0xFF) as u16) << 8
}

fn port_to_dw(port: u16) -> u32 {
    ((port as u32) << 8) | ((port as u32) >> 8)
}

fn listener_table() -> AppResult<Vec<MIB_TCPROW_OWNER_PID>> {
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
    Ok(rows.to_vec())
}

fn find_listener_row(port: u16) -> AppResult<Option<MIB_TCPROW_OWNER_PID>> {
    for row in listener_table()? {
        if row.dwState == MIB_TCP_STATE_LISTEN.0 as u32
            && local_port_from_dw(row.dwLocalPort) == port
        {
            return Ok(Some(row));
        }
    }
    Ok(None)
}

pub fn find_pid_listening_on_port(port: u16) -> AppResult<Option<u32>> {
    Ok(find_listener_row(port)?.map(|row| row.dwOwningPid))
}

/// Force-close a TCP listener via `SetTcpEntry(MIB_TCP_STATE_DELETE_TCB)`.
/// Used to reclaim a port still held by this process after a failed graceful stop.
pub fn reclaim_listening_port(port: u16) -> AppResult<bool> {
    let Some(row) = find_listener_row(port)? else {
        return Ok(false);
    };

    let mut tcp_row = TcpRow {
        dw_state: MIB_TCP_STATE_DELETE_TCB.0 as u32,
        dw_local_addr: row.dwLocalAddr,
        dw_local_port: row.dwLocalPort,
        dw_remote_addr: row.dwRemoteAddr,
        dw_remote_port: row.dwRemotePort,
    };

    let status = unsafe { SetTcpEntry((&mut tcp_row as *mut TcpRow).cast()) };
    if status != 0 {
        let message = format!("SetTcpEntry failed for port {port}: Win32 error {status}");
        eprintln!("{message}");
        return Err(AppError::Message(message));
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_port_from_dw_decodes_windows_network_order() {
        assert_eq!(local_port_from_dw(0x901F), 8080);
        assert_eq!(local_port_from_dw(0x5F70), 28767);
        assert_eq!(local_port_from_dw(port_to_dw(8787)), 8787);
    }
}
