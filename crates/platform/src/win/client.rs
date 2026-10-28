//! lazymkvctl's end of the engine's named pipe (design parts 3.3, 9.6; S38; Ruling W11): the open, which lets a server
//! at most identify the client, the one wait for a busy pipe, and the check that the server is an elevated process of
//! the client's own user, made before anything is sent. The stream halves are `pipe_stream`'s, as the server's are.

use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    GetLastError, ERROR_PIPE_BUSY, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_OVERLAPPED, OPEN_EXISTING, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT,
};
use windows_sys::Win32::System::Pipes::{GetNamedPipeServerProcessId, WaitNamedPipeW};

use super::pipe::pipe_stream;
use super::{own_token, token_of_process, wide};
use crate::security::{connect_error, token_verdict};
use crate::{ConnectError, IpcConnector, IpcStream};

/// How long the client waits, once, for a busy pipe, in milliseconds (Ruling W11). Def: T3-12. Use: T3-12.
pub(crate) const BUSY_WAIT_MS: u32 = 5000;

/// lazymkvctl's end of the named pipe (design parts 3.3, 9.6; S38; Ruling W11). It opens `name` with
/// `GENERIC_READ | GENERIC_WRITE`, `OPEN_EXISTING` and `FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT |
/// SECURITY_IDENTIFICATION`, so a server can at most identify the client, never impersonate it. It waits once for a
/// busy pipe, then refuses a server that is not an elevated process of its own user before it sends anything.
/// Def: T3-12. Use: T3-12, T3-23.
pub struct PipeConnector {
    /// The name as `CreateFileW` and `WaitNamedPipeW` take it: `wide(name)`.
    name: Vec<u16>,
    /// The security identifier (SID) of the user the server must run as; None: this process's user, read at each
    /// connect.
    own_user_sid: Option<String>,
}

impl PipeConnector {
    /// The connector for `name` (`PIPE_NAME` in the binary, a `test_name` pipe in tests). Def: T3-12.
    /// Use: T3-12, T3-23.
    pub fn new(name: &str) -> PipeConnector {
        PipeConnector { name: wide(name), own_user_sid: None }
    }

    /// `new`, but the server is checked against `own_user_sid`. Def: T3-12. Use: T3-12.
    #[cfg(test)]
    pub(crate) fn new_as(name: &str, own_user_sid: &str) -> PipeConnector {
        PipeConnector { name: wide(name), own_user_sid: Some(own_user_sid.to_string()) }
    }

    /// One open of the pipe's client end (step 2 of `connect`): the handle, or the failure's operating-system code,
    /// read at once.
    fn open(&self) -> Result<OwnedHandle, i32> {
        // SAFETY: the name is NUL-terminated and lives in `self`; the attributes and the template may be null, for a
        // default descriptor, a handle no child inherits and no template.
        let raw = unsafe {
            CreateFileW(
                self.name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                null_mut(),
            )
        };
        if raw == INVALID_HANDLE_VALUE {
            // SAFETY: GetLastError takes nothing; it is read at once, before any other call. Windows' error codes are
            // far below i32::MAX, so the cast keeps the code.
            return Err(unsafe { GetLastError() } as i32);
        }
        // SAFETY: `raw` is the open handle CreateFileW just returned, which nothing else owns.
        Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
    }
}

impl IpcConnector for PipeConnector {
    fn connect(&self) -> Result<IpcStream, ConnectError> {
        // 1. This process's token: its elevation tells what an access denied means, and its user is the user the
        // server must run as.
        let own = own_token().map_err(|_| ConnectError::Refused)?;
        // 2. The open; a busy pipe is waited for once, whatever the wait returns, then opened once more.
        let opened = match self.open() {
            Err(code) if code == ERROR_PIPE_BUSY as i32 => {
                // SAFETY: the name is NUL-terminated and lives in `self`; the call only waits, at most BUSY_WAIT_MS.
                unsafe { WaitNamedPipeW(self.name.as_ptr(), BUSY_WAIT_MS) };
                self.open()
            }
            first => first,
        };
        let handle = opened.map_err(|code| connect_error(code, own.elevated))?;
        // 3. The server's process id. Every refusal below returns before anything is written, and the return drops
        // `handle`, which closes it.
        let mut pid = 0u32;
        // SAFETY: `handle` is the open client end of a pipe; the call writes the server's process id to `pid`.
        if unsafe { GetNamedPipeServerProcessId(handle.as_raw_handle() as HANDLE, &mut pid) } == 0 {
            return Err(ConnectError::Refused);
        }
        // The id in a binding whose address is never taken.
        let pid = pid;
        // 4. The server's token and the verdict: another user is refused first, then a token that is not elevated.
        let own_user_sid = self.own_user_sid.as_deref().unwrap_or(own.user_sid.as_str());
        let facts = token_of_process(pid).map_err(|_| ConnectError::Refused)?;
        token_verdict(&facts, own_user_sid).map_err(|_| ConnectError::Refused)?;
        // 5. The stream.
        pipe_stream(handle).map_err(|_| ConnectError::Refused)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::test_name;
    use crate::testutil::{line, within};
    use crate::win::pipe::{PIPE_BUFFER, PIPE_MODE};
    use crate::win::{wide, PipeServer};
    use crate::{ConnectError, IpcConnector, IpcStream, IpcTransport};
    use std::io::{self, Read, Write};
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::ptr::null;
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
    use windows_sys::Win32::System::Pipes::{ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe};

    /// The bound of a "does not block" proof (K12), never scaled.
    const NO_BLOCK: Duration = Duration::from_secs(5);
    /// A SID that is not the test's own user.
    const OTHER_SID: &str = "S-1-5-21-1-2-3-1001";
    const SNAPSHOT: &str = r#"{"v":1,"type":"snapshot","id":null,"payload":{"jobs":[]}}"#;

    fn pipe_name() -> String {
        format!(r"\\.\pipe\{}", test_name("client"))
    }

    /// A synchronous instance of `name` with room for one instance and Windows' default security, as a busy engine or
    /// a squatter holds a pipe:
    /// `CreateNamedPipeW(PIPE_ACCESS_DUPLEX, PIPE_MODE, 1, PIPE_BUFFER, PIPE_BUFFER, 0, null)`.
    fn single_instance(name: &str) -> OwnedHandle {
        let name = wide(name);
        // SAFETY: `name` is NUL-terminated and outlives the call; null attributes give Windows' default descriptor and
        // a handle no child inherits.
        let raw = unsafe {
            CreateNamedPipeW(name.as_ptr(), PIPE_ACCESS_DUPLEX, PIPE_MODE, 1, PIPE_BUFFER, PIPE_BUFFER, 0, null())
        };
        assert!(raw != INVALID_HANDLE_VALUE && !raw.is_null(), "CreateNamedPipeW: {}", io::Error::last_os_error());
        // SAFETY: `raw` is the open instance CreateNamedPipeW just returned, which nothing else owns.
        unsafe { OwnedHandle::from_raw_handle(raw) }
    }

    #[test]
    fn no_server_is_not_running() {
        assert_eq!(PipeConnector::new(&pipe_name()).connect().err(), Some(ConnectError::NotRunning));
    }

    #[test]
    fn a_server_of_this_user_elevated_is_reached_and_its_snapshot_line_read() {
        let name = pipe_name();
        let server = Arc::new(PipeServer::bind(&name).unwrap());
        let serving = Arc::clone(&server);
        let peer = thread::spawn(move || {
            let mut stream = serving.accept().unwrap();
            stream.writer.write_all(format!("{SNAPSHOT}\n").as_bytes()).unwrap();
            stream
        });
        let mut client = PipeConnector::new(&name).connect().unwrap();
        assert_eq!(line(&mut *client.reader), format!("{SNAPSHOT}\n"));
        drop(peer.join().unwrap());
    }

    #[test]
    fn a_server_of_another_user_is_refused_before_anything_is_sent() {
        let name = pipe_name();
        let server = Arc::new(PipeServer::bind(&name).unwrap());
        let serving = Arc::clone(&server);
        let peer = thread::spawn(move || match serving.accept() {
            Ok(mut stream) => {
                let mut seen = Vec::new();
                stream.reader.read_to_end(&mut seen).unwrap();
                seen
            }
            // The client refused the server and left before this accept met it (232, ConnectionAborted): it sent
            // nothing either way.
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionAborted => Vec::new(),
            Err(error) => panic!("{error}"),
        });
        assert_eq!(PipeConnector::new_as(&name, OTHER_SID).connect().err(), Some(ConnectError::Refused));
        assert_eq!(within(NO_BLOCK, move || peer.join().unwrap()), Vec::<u8>::new());
    }

    #[test]
    fn a_busy_pipe_is_waited_for_once() {
        // Freed during the wait: the second client connects once the instance listens again.
        let name = pipe_name();
        let instance = single_instance(&name);
        let first = PipeConnector::new(&name).connect().unwrap();
        let freer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(500));
            let handle = instance.as_raw_handle() as HANDLE;
            // SAFETY: `handle` is the instance this thread owns, open for both calls; ConnectNamedPipe on this
            // synchronous instance takes no OVERLAPPED and returns once a client connects.
            unsafe {
                DisconnectNamedPipe(handle);
                ConnectNamedPipe(handle, std::ptr::null_mut());
            }
            instance
        });
        let started = Instant::now();
        let second = PipeConnector::new(&name).connect();
        assert!(second.is_ok(), "{:?}", second.as_ref().err());
        assert!(started.elapsed() >= Duration::from_millis(400), "the pipe was not busy");
        drop((first, second, freer.join().unwrap()));
        // Still busy after the one wait: Refused, once BUSY_WAIT_MS has passed.
        let name = pipe_name();
        let _instance = single_instance(&name);
        let _occupant = PipeConnector::new(&name).connect().unwrap();
        let started = Instant::now();
        assert_eq!(PipeConnector::new(&name).connect().err(), Some(ConnectError::Refused));
        assert!(started.elapsed() >= Duration::from_millis(u64::from(BUSY_WAIT_MS) - 500));
    }

    #[test]
    fn the_client_and_the_server_each_write_while_the_other_reads() {
        let name = pipe_name();
        let server = PipeServer::bind(&name).unwrap();
        let IpcStream { reader: client_reader, writer: mut client_writer } =
            PipeConnector::new(&name).connect().unwrap();
        let IpcStream { reader: server_reader, writer: mut server_writer } = server.accept().unwrap();
        let request = "{\"v\":1,\"type\":\"history\",\"id\":1,\"payload\":{}}\n";
        let reply = "{\"v\":1,\"type\":\"ok\",\"id\":1,\"payload\":{\"result\":[]}}\n";
        // Both readers wait first, so each end's writer writes while its own reader's read is pending (K12).
        let client_read = thread::spawn(move || {
            let mut reader = client_reader;
            line(&mut *reader)
        });
        let server_read = thread::spawn(move || {
            let mut reader = server_reader;
            line(&mut *reader)
        });
        thread::sleep(Duration::from_millis(100));
        let writers = within(NO_BLOCK, move || {
            client_writer.write_all(request.as_bytes()).unwrap();
            server_writer.write_all(reply.as_bytes()).unwrap();
            (client_writer, server_writer)
        });
        assert_eq!(within(NO_BLOCK, move || server_read.join().unwrap()), request);
        assert_eq!(within(NO_BLOCK, move || client_read.join().unwrap()), reply);
        drop(writers);
    }
}
