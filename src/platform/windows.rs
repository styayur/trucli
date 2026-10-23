use super::{Identity, ProxyState};
use anyhow::{Context, Result, ensure};
use std::{
    ffi::{OsStr, c_void},
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    ptr::{null, null_mut},
};
use windows_sys::Win32::{Foundation::*, Networking::WinInet::*, System::Threading::*};

fn wide(s: &OsStr) -> Result<Vec<u16>> {
    let mut v: Vec<u16> = s.encode_wide().collect();
    ensure!(!v.contains(&0), "Path/string contains a NUL character");
    v.push(0);
    Ok(v)
}
fn win_ok(result: i32) -> Result<()> {
    ensure!(
        result != 0,
        "Windows API failed: {}",
        std::io::Error::last_os_error()
    );
    Ok(())
}
fn options() -> [INTERNET_PER_CONN_OPTIONW; 4] {
    [
        INTERNET_PER_CONN_FLAGS_UI,
        INTERNET_PER_CONN_PROXY_SERVER,
        INTERNET_PER_CONN_PROXY_BYPASS,
        INTERNET_PER_CONN_AUTOCONFIG_URL,
    ]
    .map(|dw_option| INTERNET_PER_CONN_OPTIONW {
        dwOption: dw_option,
        ..Default::default()
    })
}
fn option_list(options: &mut [INTERNET_PER_CONN_OPTIONW; 4]) -> INTERNET_PER_CONN_OPTION_LISTW {
    INTERNET_PER_CONN_OPTION_LISTW {
        dwSize: size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
        dwOptionCount: 4,
        pOptions: options.as_mut_ptr(),
        ..Default::default()
    }
}
// SAFETY: WinINet returns owned, NUL-terminated UTF-16 strings for these options.
unsafe fn take_string(ptr: *mut u16) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let mut len = 0;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
        GlobalFree(ptr.cast());
        if s.is_empty() { None } else { Some(s) }
    }
}
pub fn proxy_read() -> Result<ProxyState> {
    let mut opts = options();
    let mut list = option_list(&mut opts);
    let mut size = size_of_val(&list) as u32;
    // SAFETY: list and its option array are writable and remain alive throughout the call.
    unsafe {
        win_ok(InternetQueryOptionW(
            null_mut(),
            INTERNET_OPTION_PER_CONNECTION_OPTION,
            (&mut list as *mut INTERNET_PER_CONN_OPTION_LISTW).cast(),
            &mut size,
        ))?;
        Ok(ProxyState {
            flags: opts[0].Value.dwValue,
            server: take_string(opts[1].Value.pszValue),
            bypass: take_string(opts[2].Value.pszValue),
            auto_config_url: take_string(opts[3].Value.pszValue),
        })
    }
}
pub fn proxy_write(state: &ProxyState) -> Result<()> {
    let mut opts = options();
    opts[0].dwOption = INTERNET_PER_CONN_FLAGS;
    opts[0].Value.dwValue = state.flags;
    let buffers: Vec<_> = [&state.server, &state.bypass, &state.auto_config_url]
        .into_iter()
        .map(|v| wide(OsStr::new(v.as_deref().unwrap_or(""))))
        .collect::<Result<_>>()?;
    for (opt, buf) in opts[1..].iter_mut().zip(&buffers) {
        opt.Value.pszValue = buf.as_ptr().cast_mut();
    }
    let mut list = option_list(&mut opts);
    // SAFETY: the option list and UTF-16 backing buffers remain alive; null handle targets current-user LAN settings.
    unsafe {
        win_ok(InternetSetOptionW(
            null_mut(),
            INTERNET_OPTION_PER_CONNECTION_OPTION,
            (&mut list as *mut INTERNET_PER_CONN_OPTION_LISTW).cast(),
            size_of_val(&list) as u32,
        ))?;
        win_ok(InternetSetOptionW(
            null_mut(),
            INTERNET_OPTION_SETTINGS_CHANGED,
            null(),
            0,
        ))?;
        win_ok(InternetSetOptionW(
            null_mut(),
            INTERNET_OPTION_REFRESH,
            null(),
            0,
        ))?;
    }
    Ok(())
}
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn identity_handle(handle: HANDLE, pid: u32) -> Result<Identity> {
    let mut created = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let mut buf = vec![0u16; 32768];
    let mut size = buf.len() as u32;
    // SAFETY: handle has query access; all output buffers are valid.
    unsafe {
        win_ok(GetProcessTimes(
            handle,
            &mut created,
            &mut exit,
            &mut kernel,
            &mut user,
        ))?;
        win_ok(QueryFullProcessImageNameW(
            handle,
            0,
            buf.as_mut_ptr(),
            &mut size,
        ))?;
    }
    Ok(Identity {
        pid,
        created: ((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64,
        executable: PathBuf::from(String::from_utf16(&buf[..size as usize])?),
    })
}
fn open(pid: u32, access: u32) -> Result<Option<Handle>> {
    // SAFETY: no inherited handle, requested access only.
    let h = unsafe { OpenProcess(access, 0, pid) };
    if h.is_null() {
        let e = std::io::Error::last_os_error();
        if e.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
            return Ok(None);
        }
        return Err(e).context("Cannot establish process identity; refusing unsafe cleanup");
    }
    let h = Handle(h);
    if unsafe { WaitForSingleObject(h.0, 0) } == WAIT_OBJECT_0 {
        return Ok(None);
    }
    Ok(Some(h))
}
pub fn identity(pid: u32) -> Result<Option<Identity>> {
    open(pid, PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE)?
        .map(|h| identity_handle(h.0, pid))
        .transpose()
}
pub fn terminate(expected: &Identity) -> Result<bool> {
    let Some(h) = open(
        expected.pid,
        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
    )?
    else {
        return Ok(false);
    };
    if identity_handle(h.0, expected.pid)? != *expected {
        return Ok(false);
    }
    // SAFETY: identity was checked using this same open handle, preventing a PID reuse race.
    unsafe {
        win_ok(TerminateProcess(h.0, 0))?;
        ensure!(
            WaitForSingleObject(h.0, 5000) == WAIT_OBJECT_0,
            "Owned process did not terminate; recovery state retained"
        );
    }
    Ok(true)
}

pub fn listener_owned(pid: u32, port: u16) -> Result<bool> {
    use windows_sys::Win32::{NetworkManagement::IpHelper::*, Networking::WinSock::AF_INET};
    let mut size = 0u32;
    // SAFETY: first call obtains the required size; the second uses a suitably aligned u32 allocation.
    unsafe {
        GetExtendedTcpTable(
            null_mut(),
            &mut size,
            0,
            AF_INET as u32,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        );
    }
    ensure!(
        (4..16 * 1024 * 1024).contains(&size),
        "Invalid TCP table size"
    );
    let mut buffer = vec![0u32; (size as usize).div_ceil(4)];
    let code = unsafe {
        GetExtendedTcpTable(
            buffer.as_mut_ptr().cast(),
            &mut size,
            0,
            AF_INET as u32,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };
    ensure!(
        code == NO_ERROR,
        "Cannot query listener process ownership ({code})"
    );
    let count = buffer[0] as usize;
    ensure!(
        4 + count * size_of::<MIB_TCPROW_OWNER_PID>() <= size as usize,
        "Invalid TCP table length"
    );
    for index in 0..count {
        // SAFETY: each row lies in the validated table; read_unaligned handles C array alignment.
        let row = unsafe {
            std::ptr::read_unaligned(
                buffer
                    .as_ptr()
                    .cast::<u8>()
                    .add(4 + index * size_of::<MIB_TCPROW_OWNER_PID>())
                    .cast::<MIB_TCPROW_OWNER_PID>(),
            )
        };
        if row.dwOwningPid == pid
            && u16::from_be(row.dwLocalPort as u16) == port
            && row.dwLocalAddr.to_le_bytes() == [127, 0, 0, 1]
        {
            return Ok(true);
        }
    }
    Ok(false)
}
pub struct Pending {
    process: Handle,
    thread: Handle,
    pid: u32,
    armed: bool,
}
impl Pending {
    pub fn launch(exe: &Path, config: &Path, data: &Path) -> Result<Self> {
        // Windows argv quoting: paths are quoted, trailing backslashes doubled before closing quotes.
        fn quote(p: &Path) -> Result<String> {
            let s = p.to_str().context("Mihomo paths must be Unicode")?;
            ensure!(
                !s.contains(['"', '\0']),
                "Invalid executable/configuration path"
            );
            let trailing = s.chars().rev().take_while(|c| *c == '\\').count();
            Ok(format!("\"{s}{}\"", "\\".repeat(trailing)))
        }
        let application = wide(exe.as_os_str())?;
        let mut cmd = wide(OsStr::new(&format!(
            "{} -d {} -f {}",
            quote(exe)?,
            quote(data)?,
            quote(config)?
        )))?;
        let directory = wide(data.as_os_str())?;
        let mut env: Vec<(std::ffi::OsString, std::ffi::OsString)> = std::env::vars_os()
            .filter(|(k, _)| {
                let k = k.to_string_lossy().to_ascii_uppercase();
                !k.starts_with("CLASH_") && !k.starts_with("MIHOMO_")
            })
            .collect();
        env.sort_by_key(|(k, _)| k.to_string_lossy().to_ascii_uppercase());
        let mut block = Vec::<u16>::new();
        for (k, v) in env {
            block.extend(wide(OsStr::new(&format!(
                "{}={}",
                k.to_string_lossy(),
                v.to_string_lossy()
            )))?);
        }
        block.push(0);
        let startup = STARTUPINFOW {
            cb: size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut info = PROCESS_INFORMATION::default();
        // SAFETY: buffers are NUL-terminated; no handles inherited; suspended child cannot listen before journaling.
        unsafe {
            win_ok(CreateProcessW(
                application.as_ptr(),
                cmd.as_mut_ptr(),
                null(),
                null(),
                0,
                CREATE_SUSPENDED | CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT,
                block.as_ptr().cast::<c_void>(),
                directory.as_ptr(),
                &startup,
                &mut info,
            ))?;
        }
        Ok(Self {
            process: Handle(info.hProcess),
            thread: Handle(info.hThread),
            pid: info.dwProcessId,
            armed: true,
        })
    }
    pub fn identity(&self) -> Result<Identity> {
        identity_handle(self.process.0, self.pid)
    }
    pub fn resume(mut self) -> Result<()> {
        // SAFETY: valid primary thread handle owned by this instance.
        ensure!(
            unsafe { ResumeThread(self.thread.0) } != u32::MAX,
            "Cannot resume journaled Mihomo process"
        );
        self.armed = false;
        Ok(())
    }
}
impl Drop for Pending {
    fn drop(&mut self) {
        if self.armed {
            unsafe {
                TerminateProcess(self.process.0, 1);
            }
        }
    }
}

pub fn evidence() -> Result<serde_json::Value> {
    // Constant script only; no profile/config values are interpolated into PowerShell.
    const SCRIPT: &str = r#"
$ErrorActionPreference='Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$installed = @(Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*','HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*','HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*' -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -match 'aTrust' } | Select-Object -ExpandProperty DisplayName)
@{
 processes = @(Get-Process | Select-Object -ExpandProperty ProcessName)
 installed = $installed
 adapters = @(Get-NetAdapter -IncludeHidden | Select-Object ifIndex,Name,InterfaceDescription,Status)
 routes = @(Get-NetRoute | Select-Object -First 4096 DestinationPrefix,InterfaceIndex,NextHop,RouteMetric)
 dns = @(Get-DnsClientServerAddress | Select-Object InterfaceIndex,AddressFamily,ServerAddresses)
 interfaces = @(Get-NetIPInterface | Select-Object InterfaceIndex,AddressFamily,InterfaceMetric,ConnectionState)
} | ConvertTo-Json -Depth 6 -Compress
"#;
    let root = std::env::var_os("SystemRoot").context("SystemRoot unavailable")?;
    let exe = PathBuf::from(root).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut cmd = std::process::Command::new(exe);
    use std::os::windows::process::CommandExt;
    let mut child = cmd
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            SCRIPT,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    let mut stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = Vec::new();
        stdout
            .by_ref()
            .take(4 * 1024 * 1024)
            .read_to_end(&mut buf)
            .map(|_| buf)
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        if let Some(exit) = child.try_wait()? {
            ensure!(
                exit.success(),
                "Windows networking evidence unavailable (PowerShell/CIM failed)"
            );
            break;
        }
        if std::time::Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            anyhow::bail!("Windows evidence query timed out");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let bytes = reader
        .join()
        .map_err(|_| anyhow::anyhow!("Evidence reader failed"))??;
    serde_json::from_slice(&bytes).context("Invalid structured Windows evidence")
}
