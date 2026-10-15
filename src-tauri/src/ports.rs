use crate::model::{PortInfo, ScanDiagnostics};
use crate::procs::ProcessTable;
use libproc::libproc::{
    bsd_info::BSDInfo,
    file_info::{ListFDs, ProcFDType, pidfdinfo},
    net_info::{SocketFDInfo, SocketInfoKind, TcpSIState},
    proc_pid::{listpidinfo, pidinfo},
};
use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

fn canonical_address(addr: &str) -> Option<String> {
    let addr = addr.trim_matches(['[', ']']);
    if addr == "*" {
        return Some("*".into());
    }
    // Preserve IPv6 scope IDs, if lsof supplies one.
    let (ip, scope) = addr
        .split_once('%')
        .map_or((addr, None), |(ip, scope)| (ip, Some(scope)));
    let ip: IpAddr = ip.parse().ok()?;
    if ip.is_unspecified() {
        return Some("*".into());
    }
    let ip = match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        _ => ip,
    };
    Some(match scope {
        Some(scope) => format!("{ip}%{scope}"),
        None => ip.to_string(),
    })
}

/// Parse field-mode lsof output, never human-aligned columns. Only LISTEN output
/// is passed by the caller. Malformed PID records reset context instead of
/// accidentally attributing following sockets to the previous process.
pub fn parse_lsof(output: &str) -> Vec<PortInfo> {
    let mut pid = None;
    let mut ports = BTreeSet::new();
    for line in output.lines() {
        if let Some(value) = line.strip_prefix('p') {
            pid = value.parse::<u32>().ok().filter(|pid| *pid > 0);
        } else if let (Some(pid), Some(value)) = (pid, line.strip_prefix('n')) {
            let value = value.strip_suffix(" (LISTEN)").unwrap_or(value);
            let Some((addr, port)) = value.rsplit_once(':') else {
                continue;
            };
            let Some(addr) = canonical_address(addr) else {
                continue;
            };
            let Ok(port) = port.parse::<u16>() else {
                continue;
            };
            if port > 0 {
                ports.insert(PortInfo { pid, port, addr });
            }
        }
    }
    ports.into_iter().collect()
}

fn socket_listener(pid: u32, socket: &SocketFDInfo) -> Option<PortInfo> {
    if !matches!(
        SocketInfoKind::from(socket.psi.soi_kind),
        SocketInfoKind::Tcp
    ) || socket.psi.soi_protocol != libc::IPPROTO_TCP
    {
        return None;
    }
    // SAFETY: soi_kind identifies the active TCP union member.
    let tcp = unsafe { socket.psi.soi_proto.pri_tcp };
    if !matches!(TcpSIState::from(tcp.tcpsi_state), TcpSIState::Listen) {
        return None;
    }
    let info = tcp.tcpsi_ini;
    let port = u16::from_be(info.insi_lport as u16);
    if port == 0 {
        return None;
    }
    let addr = match socket.psi.soi_family {
        // SAFETY: the socket family identifies the address union member.
        libc::AF_INET => {
            Ipv4Addr::from(unsafe { info.insi_laddr.ina_46.i46a_addr4.s_addr }.to_ne_bytes())
                .to_string()
        }
        libc::AF_INET6 => {
            let ip = Ipv6Addr::from(unsafe { info.insi_laddr.ina_6.s6_addr });
            if ip.is_unicast_link_local() && info.insi_v6.in6_ifindex != 0 {
                format!("{ip}%{}", info.insi_v6.in6_ifindex)
            } else {
                ip.to_string()
            }
        }
        _ => return None,
    };
    Some(PortInfo {
        pid,
        port,
        addr: canonical_address(&addr)?,
    })
}

pub fn scan(table: &ProcessTable, uid: u32) -> (Vec<PortInfo>, ScanDiagnostics) {
    let mut ports = BTreeSet::new();
    let mut diagnostics = ScanDiagnostics {
        visibility_limited: true,
        ..Default::default()
    };
    for process in table.by_pid.values() {
        if process.uid != Some(uid) {
            continue;
        }
        let pid = process.pid as i32;
        let fds = pidinfo::<BSDInfo>(pid, 0).and_then(|info| {
            if info.pbi_uid != uid {
                return Err("process owner changed during scan".into());
            }
            // Allow headroom for descriptors opened between these two calls.
            listpidinfo::<ListFDs>(pid, info.pbi_nfiles as usize + 32)
        });
        let Ok(fds) = fds else {
            diagnostics.inaccessible_pids.push(process.pid);
            continue;
        };
        for fd in fds {
            if !matches!(ProcFDType::from(fd.proc_fdtype), ProcFDType::Socket) {
                continue;
            }
            match pidfdinfo::<SocketFDInfo>(pid, fd.proc_fd) {
                Ok(socket) => {
                    if let Some(port) = socket_listener(process.pid, &socket) {
                        ports.insert(port);
                    }
                }
                Err(_) => {
                    diagnostics.inaccessible_pids.push(process.pid);
                }
            }
        }
    }
    diagnostics.libproc_listener_count = ports.len();
    // One non-privileged fallback per tick, also covering processes created after
    // sysinfo's sample. This cannot reveal listeners macOS hides from this user.
    match crate::scan_command::output(
        "/usr/sbin/lsof",
        &["-nP", "-iTCP", "-sTCP:LISTEN", "-F", "pcn"],
    ) {
        Ok(output) => {
            let fallback = parse_lsof(&String::from_utf8_lossy(&output.stdout));
            for port in fallback {
                // Merge all bindings: even a successful libproc call can race
                // descriptor creation or provide incomplete visibility.
                if ports.insert(port) {
                    diagnostics.lsof_listener_count += 1;
                }
            }
            // Exit 1 with no output is lsof's ordinary "no matches" result.
            if !output.status.success() && output.status.code() != Some(1) {
                diagnostics
                    .warnings
                    .push(format!("lsof exited with {}", output.status));
            }
            if !output.stderr.is_empty() {
                diagnostics.warnings.push(format!(
                    "lsof: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
        }
        Err(error) => diagnostics
            .warnings
            .push(format!("lsof fallback unavailable: {error}")),
    }
    diagnostics.inaccessible_pids.sort_unstable();
    diagnostics.inaccessible_pids.dedup();
    (ports.into_iter().collect(), diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_lsof_deduplicates_fds_and_reads_ipv4_ipv6() {
        let ports = parse_lsof(include_str!("fixtures/listeners.lsof"));
        assert!(ports.contains(&PortInfo {
            pid: 2752,
            port: 3216,
            addr: "127.0.0.1".into()
        }));
        assert!(ports.iter().any(|p| p.port == 5432));
        assert!(ports.iter().any(|p| p.port == 9230));
        assert_eq!(
            ports
                .iter()
                .filter(|p| p.pid == 512 && p.port == 49173)
                .count(),
            1
        );
        assert!(ports.iter().any(|p| p.addr == "::1"));
    }

    #[test]
    fn malformed_records_do_not_leak_pid_context() {
        let ports = parse_lsof(
            "n*:80\np10\ncnode\nn[::]:8080\nn0.0.0.0:8080\npbad\nn*:90\np11\nn[::ffff:127.0.0.1]:5000\nn127.0.0.1:5000 (LISTEN)\nn*:0\nn*:65536\nnlocalhost:80\nn127.0.0.1:5000->127.0.0.1:80\n",
        );
        assert_eq!(
            ports,
            vec![
                PortInfo {
                    pid: 10,
                    port: 8080,
                    addr: "*".into()
                },
                PortInfo {
                    pid: 11,
                    port: 5000,
                    addr: "127.0.0.1".into()
                }
            ]
        );
        assert!(parse_lsof("").is_empty());
    }

    #[test]
    fn native_socket_decodes_network_byte_order_and_rejects_non_listeners() {
        use libproc::libproc::net_info::{In4In6Addr, InSIAddr, SocketInfoProto, TcpSockInfo};
        let mut socket = SocketFDInfo::default();
        socket.psi.soi_kind = SocketInfoKind::Tcp as i32;
        socket.psi.soi_protocol = libc::IPPROTO_TCP;
        socket.psi.soi_family = libc::AF_INET;
        let mut tcp = TcpSockInfo {
            tcpsi_state: TcpSIState::Listen as i32,
            ..Default::default()
        };
        tcp.tcpsi_ini.insi_lport = 5432u16.to_be() as i32;
        tcp.tcpsi_ini.insi_laddr = InSIAddr {
            ina_46: In4In6Addr {
                i46a_addr4: libc::in_addr {
                    s_addr: u32::from_ne_bytes([127, 0, 0, 1]),
                },
                ..Default::default()
            },
        };
        socket.psi.soi_proto = SocketInfoProto { pri_tcp: tcp };
        assert_eq!(
            socket_listener(12, &socket),
            Some(PortInfo {
                pid: 12,
                port: 5432,
                addr: "127.0.0.1".into()
            })
        );
        tcp.tcpsi_state = TcpSIState::Established as i32;
        socket.psi.soi_proto = SocketInfoProto { pri_tcp: tcp };
        assert!(socket_listener(12, &socket).is_none());
    }
}
