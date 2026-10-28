//! The engine's named pipe (design parts 3.3, 3.4, 4.4; K12, P6; Ruling W10): every instance overlapped, under the
//! descriptor `pipe_sddl` gives with its high mandatory label, each client's token checked before its stream is handed
//! out, and the overlapped stream halves the pipe's client end shares (T3-12), whose writer never waits behind a
//! pending read.

use std::io::{self, BufReader, Read, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use windows_sys::Win32::Foundation::{
    GetLastError, ERROR_ACCESS_DENIED, ERROR_BROKEN_PIPE, ERROR_IO_PENDING, ERROR_NO_DATA, ERROR_OPERATION_ABORTED,
    ERROR_PIPE_CONNECTED, ERROR_PIPE_NOT_CONNECTED, HANDLE, INVALID_HANDLE_VALUE, WAIT_EVENT, WAIT_OBJECT_0,
};
use windows_sys::Win32::Storage::FileSystem::{
    ReadFile, WriteFile, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, SetEvent, WaitForMultipleObjects, WaitForSingleObject, INFINITE,
};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};

use super::{own_token, token_of_process, wide, Descriptor};
use crate::security::{pipe_sddl, token_verdict};
use crate::{IpcStream, IpcTransport};

/// The pipe mode every `CreateNamedPipeW` call passes (design parts 3.3, 3.4). Def: T3-11. Use: T3-11, T3-12.
pub(crate) const PIPE_MODE: u32 = PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS;
/// Each instance's buffer, each way, in bytes (K12). Def: T3-11. Use: T3-11, T3-12.
pub(crate) const PIPE_BUFFER: u32 = 65536;

/// The engine's end of the named pipe (design parts 3.3, 3.4, 4.4; K12, P6; Ruling W10). Every instance is created
/// overlapped, from `pipe_sddl` of this process's user, with `PIPE_MODE`, unlimited instances and `PIPE_BUFFER` each
/// way; the first with `FILE_FLAG_FIRST_PIPE_INSTANCE`. A client's token is checked, and the next instance created,
/// before its stream is handed out. Def: T3-11. Use: T3-12, T3-23.
pub struct PipeServer {
    /// The name as `CreateNamedPipeW` takes it: `wide(name)`.
    name: Vec<u16>,
    /// The name as given, for texts.
    shown: String,
    /// `Descriptor::from_sddl(&pipe_sddl(<own user SID>))`, made once in `bind_with`, for every instance.
    descriptor: Descriptor,
    /// The security identifier (SID) `token_verdict` compares a client with.
    own_user_sid: String,
    /// A manual-reset event `close` sets, which ends a pending connect.
    stop: OwnedHandle,
    /// Whether `close` ran.
    closed: AtomicBool,
    /// The instance the next client connects to.
    listening: Mutex<Option<OwnedHandle>>,
}

impl PipeServer {
    /// Creates the first instance of `name` (design part 3.3). Err `ErrorKind::AddrInUse` with the text
    /// `another process holds the pipe <name>` when `CreateNamedPipeW` refuses `FILE_FLAG_FIRST_PIPE_INSTANCE` with
    /// `ERROR_ACCESS_DENIED` (5); any other failure is that operating-system error. Def: T3-11. Use: T3-12, T3-23.
    pub fn bind(name: &str) -> io::Result<PipeServer> {
        PipeServer::bind_with(name, own_token()?.user_sid)
    }

    /// `bind`, but clients are checked against `own_user_sid` instead of this process's user; the descriptor still
    /// grants this process's user, so a test client opens the pipe and meets the check. Def: T3-11. Use: T3-11.
    #[cfg(test)]
    pub(crate) fn bind_as(name: &str, own_user_sid: &str) -> io::Result<PipeServer> {
        PipeServer::bind_with(name, own_user_sid.to_string())
    }

    /// The server of `name` whose descriptor grants this process's user and whose client check compares with
    /// `own_user_sid`, holding its first instance.
    fn bind_with(name: &str, own_user_sid: String) -> io::Result<PipeServer> {
        let descriptor = Descriptor::from_sddl(&pipe_sddl(&own_token()?.user_sid))?;
        let server = PipeServer {
            name: wide(name),
            shown: name.to_string(),
            descriptor,
            own_user_sid,
            stop: manual_event()?,
            closed: AtomicBool::new(false),
            listening: Mutex::new(None),
        };
        let first = server.create_instance(true)?;
        *lock(&server.listening) = Some(first);
        Ok(server)
    }

    /// One more instance of the pipe, overlapped and under the server's descriptor; with `first`, the instance that
    /// claims the name, `ERROR_ACCESS_DENIED` being `AddrInUse` (design part 3.3).
    fn create_instance(&self, first: bool) -> io::Result<OwnedHandle> {
        let attributes = self.descriptor.attributes();
        let claim = if first { FILE_FLAG_FIRST_PIPE_INSTANCE } else { 0 };
        // SAFETY: the name is NUL-terminated and lives in `self`; the attributes point at `self.descriptor`, which
        // outlives the call.
        let raw = unsafe {
            CreateNamedPipeW(
                self.name.as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | claim,
                PIPE_MODE,
                PIPE_UNLIMITED_INSTANCES,
                PIPE_BUFFER,
                PIPE_BUFFER,
                0,
                &attributes,
            )
        };
        if raw == INVALID_HANDLE_VALUE || raw.is_null() {
            let error = io::Error::last_os_error();
            if first && error.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32) {
                let text = format!("another process holds the pipe {}", self.shown);
                return Err(io::Error::new(io::ErrorKind::AddrInUse, text));
            }
            return Err(error);
        }
        // SAFETY: `raw` is the open instance CreateNamedPipeW just returned, which nothing else owns.
        Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
    }

    /// Waits for a client to connect to `instance` (step 3 of `accept`): Ok(None) on a connection, Ok(Some(code)) on
    /// a failure with that code, Err when `close` ended the wait. The connect is cancelled, and its end awaited,
    /// before its OVERLAPPED goes, whatever ended the wait.
    fn connect(&self, instance: &OwnedHandle, event: &OwnedHandle) -> Result<Option<u32>, Closed> {
        let handle = raw(instance);
        let mut overlapped = overlapped_on(event);
        // SAFETY: `instance` is an open overlapped instance; `overlapped` lives on this frame until the connect has
        // ended, as every path below waits for that, and its event is a manual-reset event alive as long.
        if unsafe { ConnectNamedPipe(handle, &mut overlapped) } != 0 {
            return Ok(None);
        }
        match last_error() {
            ERROR_PIPE_CONNECTED => Ok(None),
            ERROR_IO_PENDING => {
                let woke = wait_any(&[raw(event), raw(&self.stop)]);
                if woke == WAIT_OBJECT_0 {
                    return Ok(result_of(handle, &overlapped, false).err());
                }
                let failed = (woke != WAIT_OBJECT_0 + 1).then(last_error);
                cancel(handle, &overlapped);
                let ended = result_of(handle, &overlapped, true);
                match failed {
                    // The wait itself failed: its code, unless the connect had ended with one of its own.
                    Some(code) => Ok(Some(ended.err().filter(|&end| end != ERROR_OPERATION_ABORTED).unwrap_or(code))),
                    None => Err(Closed),
                }
            }
            code => Ok(Some(code)),
        }
    }

    /// The client check of design part 3.4 and P6 on the connected `instance`: the client's process id, its token,
    /// then `token_verdict`, which refuses another user first, then a token that is not elevated. The texts name the
    /// client's process id, never its account (Ruling W4).
    fn check_client(&self, instance: &OwnedHandle) -> io::Result<()> {
        let mut pid = 0u32;
        // SAFETY: `instance` is a connected instance; the call writes the client's process id to `pid`.
        if unsafe { GetNamedPipeClientProcessId(raw(instance), &mut pid) } == 0 {
            let text = format!("the client left before its check: {}", io::Error::last_os_error());
            return Err(io::Error::new(io::ErrorKind::ConnectionAborted, text));
        }
        // The id in a binding whose address is never taken.
        let pid = pid;
        let refused = |text: String| io::Error::new(io::ErrorKind::PermissionDenied, format!("process {pid}: {text}"));
        let facts = token_of_process(pid).map_err(|error| refused(format!("its token could not be read: {error}")))?;
        token_verdict(&facts, &self.own_user_sid).map_err(|verdict| refused(verdict.to_string()))
    }
}

/// `close` ran: the accept that met it returns `BrokenPipe`.
struct Closed;

/// `accept`'s error once `close` ran (Ruling W10).
fn closed_error() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "the pipe server is closed")
}

impl IpcTransport for PipeServer {
    fn accept(&self) -> io::Result<IpcStream> {
        // 1. A closed server accepts nothing.
        if self.closed.load(Ordering::SeqCst) {
            return Err(closed_error());
        }
        // Made before the listening instance is taken, so a failure here leaves the name held.
        let event = manual_event().map_err(not_closed)?;
        // 2. The listening instance, or a new one when an earlier accept left none.
        let taken = lock(&self.listening).take();
        let instance = match taken {
            Some(instance) => instance,
            None => self.create_instance(false).map_err(not_closed)?,
        };
        if self.closed.load(Ordering::SeqCst) {
            return Err(closed_error());
        }
        // 3. The connect; a failure is kept until the next instance exists.
        let failed = match self.connect(&instance, &event) {
            Ok(failed) => failed,
            Err(Closed) => return Err(closed_error()),
        };
        // 4. The next instance, before this one is handed out or dropped, so the name is never unheld.
        {
            let mut listening = lock(&self.listening);
            if self.closed.load(Ordering::SeqCst) {
                return Err(closed_error());
            }
            *listening = Some(self.create_instance(false).map_err(not_closed)?);
        }
        // 5. A failed connect drops its instance.
        if let Some(code) = failed {
            return Err(accept_error(code));
        }
        // 6. The client check; a refused client's instance is dropped, with no DisconnectNamedPipe.
        self.check_client(&instance)?;
        // 7. The stream.
        pipe_stream(instance).map_err(not_closed)
    }

    fn close(&self) {
        let listening = {
            let mut listening = lock(&self.listening);
            self.closed.store(true, Ordering::SeqCst);
            listening.take()
        };
        // SAFETY: `stop` is the server's open event; it stays set, so an accept that starts its wait later returns at
        // once.
        unsafe { SetEvent(raw(&self.stop)) };
        drop(listening);
    }
}

/// The error `accept` returns for a failed connect or client check with OS code `code` (Ruling W10):
/// `ERROR_BROKEN_PIPE` (109), `ERROR_NO_DATA` (232) and `ERROR_PIPE_NOT_CONNECTED` (233) are `ConnectionAborted`;
/// any other code is `not_closed(io::Error::from_raw_os_error(code))`. Def: T3-11. Use: T3-11.
pub(crate) fn accept_error(code: u32) -> io::Error {
    match code {
        ERROR_BROKEN_PIPE | ERROR_NO_DATA | ERROR_PIPE_NOT_CONNECTED => {
            let text = format!("the client left before its check: {}", io::Error::from_raw_os_error(code as i32));
            io::Error::new(io::ErrorKind::ConnectionAborted, text)
        }
        _ => not_closed(io::Error::from_raw_os_error(code as i32)),
    }
}

/// `error`, unless it is an operating-system error of kind `BrokenPipe`, `PermissionDenied` or `ConnectionAborted`,
/// which becomes `io::Error::new(ErrorKind::Other, error)` with its text kept, so only `close` ever gives
/// `BrokenPipe` and only the client check gives `PermissionDenied` or `ConnectionAborted` (engine.md:581 (d); Ruling
/// PF3-S10-12-2). Def: T3-11. Use: T3-11.
pub(crate) fn not_closed(error: io::Error) -> io::Error {
    use io::ErrorKind::{BrokenPipe, ConnectionAborted, PermissionDenied};
    if error.raw_os_error().is_some() && matches!(error.kind(), BrokenPipe | PermissionDenied | ConnectionAborted) {
        io::Error::new(io::ErrorKind::Other, error)
    } else {
        error
    }
}

/// One connected pipe handle, server or client end, opened overlapped, as an `IpcStream` (K12; Rulings T8-e, W10).
/// The reader half and the writer half each wait on an event of their own, so the writer never waits behind the
/// reader's pending read. The writer's `flush` does nothing: `FlushFileBuffers`, which on a pipe waits until the peer
/// has read, is never called per line. Dropping the writer sets the stream's closing event, which ends the reader's
/// pending or next read as end of file; the handle closes when both halves are gone, so bytes written before stay
/// readable by the peer. A read returns `Ok(0)` on `ERROR_BROKEN_PIPE` (109); a write after the peer is gone fails
/// with kind `BrokenPipe`. Def: T3-11. Use: T3-11, T3-12.
pub(crate) fn pipe_stream(handle: OwnedHandle) -> io::Result<IpcStream> {
    let shared = Arc::new(Shared { handle, closing: manual_event()? });
    let reader = PipeReader { shared: Arc::clone(&shared), event: manual_event()? };
    let writer = PipeWriter { shared, event: manual_event()? };
    Ok(IpcStream {
        reader: Box::new(BufReader::with_capacity(PIPE_BUFFER as usize, reader)),
        writer: Box::new(writer),
    })
}

/// What a stream's two halves share: the pipe handle, which closes when both are gone, and the manual-reset closing
/// event the writer's drop sets.
struct Shared {
    handle: OwnedHandle,
    closing: OwnedHandle,
}

/// A stream's reader half: overlapped reads on its own event, ended by the closing event (K12).
struct PipeReader {
    shared: Arc<Shared>,
    event: OwnedHandle,
}

impl Read for PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let closing = raw(&self.shared.closing);
        if buf.is_empty() || is_set(closing) {
            return Ok(0);
        }
        let handle = raw(&self.shared.handle);
        let length = u32::try_from(buf.len()).unwrap_or(u32::MAX);
        let mut overlapped = overlapped_on(&self.event);
        // SAFETY: the handle is open while `shared` lives; `buf` holds `length` bytes and, like `overlapped`, outlives
        // the read, since every path below waits for its end; the count pointer is null, as an overlapped read takes
        // its count from GetOverlappedResult.
        let started = unsafe { ReadFile(handle, buf.as_mut_ptr(), length, null_mut(), &mut overlapped) };
        let ended = if started != 0 {
            result_of(handle, &overlapped, false)
        } else {
            match last_error() {
                ERROR_IO_PENDING => {
                    // A completed read wins over the closing event: WaitForMultipleObjects reports the lowest index.
                    let woke = wait_any(&[raw(&self.event), closing]);
                    if woke == WAIT_OBJECT_0 {
                        result_of(handle, &overlapped, false)
                    } else {
                        // Closing, or a failed wait: the read is cancelled and its end awaited before `overlapped`
                        // goes; what it read before the cancel is returned.
                        let failed = (woke != WAIT_OBJECT_0 + 1).then(io::Error::last_os_error);
                        cancel(handle, &overlapped);
                        match (result_of(handle, &overlapped, true), failed) {
                            (Err(ERROR_OPERATION_ABORTED), Some(error)) => return Err(error),
                            (ended, _) => ended,
                        }
                    }
                }
                code => Err(code),
            }
        };
        match ended {
            Ok(transferred) => Ok(transferred as usize),
            // Ruling PF3-S10-12-7: the peer gone is the end at any time, a cancelled read only after closing.
            Err(ERROR_BROKEN_PIPE) => Ok(0),
            Err(ERROR_OPERATION_ABORTED) if is_set(closing) => Ok(0),
            Err(code) => Err(io::Error::from_raw_os_error(code as i32)),
        }
    }
}

/// A stream's writer half: overlapped writes on its own event, each awaited on that event alone (K12).
struct PipeWriter {
    shared: Arc<Shared>,
    event: OwnedHandle,
}

impl Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let handle = raw(&self.shared.handle);
        let length = u32::try_from(buf.len()).unwrap_or(u32::MAX);
        let mut overlapped = overlapped_on(&self.event);
        // SAFETY: the handle is open while `shared` lives; `buf` holds `length` bytes and, like `overlapped`, outlives
        // the write, which is awaited below; the count pointer is null, as an overlapped write takes its count from
        // GetOverlappedResult.
        let started = unsafe { WriteFile(handle, buf.as_ptr(), length, null_mut(), &mut overlapped) };
        let ended = if started != 0 {
            result_of(handle, &overlapped, false)
        } else {
            match last_error() {
                ERROR_IO_PENDING => result_of(handle, &overlapped, true),
                code => Err(code),
            }
        };
        // ERROR_NO_DATA (232) and ERROR_BROKEN_PIPE (109) read as kind BrokenPipe (Ruling T8-e).
        ended.map(|transferred| transferred as usize).map_err(|code| io::Error::from_raw_os_error(code as i32))
    }

    /// Nothing: `FlushFileBuffers` would wait until the peer has read (K12).
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for PipeWriter {
    fn drop(&mut self) {
        // SAFETY: the closing event is open while `shared` lives; setting it waits for nothing.
        unsafe { SetEvent(raw(&self.shared.closing)) };
    }
}

/// Locks `mutex`, ignoring a poison a panicked thread left.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `handle` as Win32 calls take it.
fn raw(handle: &OwnedHandle) -> HANDLE {
    handle.as_raw_handle() as HANDLE
}

/// The calling thread's last error code.
fn last_error() -> u32 {
    // SAFETY: GetLastError takes nothing; every caller reads it at once, before any other call.
    unsafe { GetLastError() }
}

/// A new manual-reset event, not set.
fn manual_event() -> io::Result<OwnedHandle> {
    // SAFETY: the attributes and the name may be null, for a default descriptor, a handle no child inherits and an
    // unnamed event.
    let event = unsafe { CreateEventW(null(), 1, 0, null()) };
    if event.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `event` is the open event CreateEventW just returned, which nothing else owns.
    Ok(unsafe { OwnedHandle::from_raw_handle(event) })
}

/// An OVERLAPPED for one operation that signals `event` when it ends.
fn overlapped_on(event: &OwnedHandle) -> OVERLAPPED {
    OVERLAPPED { hEvent: raw(event), ..OVERLAPPED::default() }
}

/// Whether the event `event` is set, without waiting.
fn is_set(event: HANDLE) -> bool {
    // SAFETY: every caller passes an open event; a wait of 0 ms only tests it.
    unsafe { WaitForSingleObject(event, 0) == WAIT_OBJECT_0 }
}

/// Waits without end until one of `handles` is set: `WAIT_OBJECT_0` plus the lowest index set, or `WAIT_FAILED`.
fn wait_any(handles: &[HANDLE]) -> WAIT_EVENT {
    // SAFETY: the array holds `handles.len()` open handles and outlives the call.
    unsafe { WaitForMultipleObjects(handles.len() as u32, handles.as_ptr(), 0, INFINITE) }
}

/// Cancels the operation `overlapped` describes on `handle`; it may have ended already, which the caller's
/// `result_of(.., true)` then reports.
fn cancel(handle: HANDLE, overlapped: &OVERLAPPED) {
    // SAFETY: `overlapped` is the OVERLAPPED the caller started the operation on `handle` with, alive until the caller
    // has awaited the operation's end.
    unsafe { CancelIoEx(handle, overlapped) };
}

/// The bytes the operation `overlapped` describes transferred, or its error code; with `wait`, once it has ended.
fn result_of(handle: HANDLE, overlapped: &OVERLAPPED, wait: bool) -> Result<u32, u32> {
    let mut transferred = 0u32;
    // SAFETY: `overlapped` is the OVERLAPPED the operation on `handle` was started with, alive for the call, and its
    // event is open; the call writes the count to `transferred`.
    if unsafe { GetOverlappedResult(handle, overlapped, &mut transferred, i32::from(wait)) } == 0 {
        return Err(last_error());
    }
    // The count in a binding whose address is never taken.
    let transferred = transferred;
    Ok(transferred)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::{test_name, HIGH_INTEGRITY_SID, SYSTEM_SID};
    use crate::testutil::{ci_factor, line, within};
    use crate::win::{own_token, read_handle, sid_string, wide, SE_GROUP_INTEGRITY};
    use crate::{IpcStream, IpcTransport};
    use std::io::{self, BufRead, Write};
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::ptr::{null, null_mut};
    use std::sync::mpsc::channel;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{
        LocalFree, ERROR_SUCCESS, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Security::Authorization::{ConvertStringSidToSidW, GetSecurityInfo, SE_KERNEL_OBJECT};
    use windows_sys::Win32::Security::{
        DuplicateTokenEx, GetAce, MapGenericMask, RevertToSelf, SecurityImpersonation, SetTokenInformation,
        TokenImpersonation, TokenIntegrityLevel, ACL, GENERIC_MAPPING, LABEL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
        PSID, SID_AND_ATTRIBUTES, SYSTEM_MANDATORY_LABEL_ACE, TOKEN_ALL_ACCESS, TOKEN_DUPLICATE, TOKEN_MANDATORY_LABEL,
        TOKEN_QUERY,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ALL_ACCESS, FILE_FLAG_OVERLAPPED, FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
        OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Pipes::{GetNamedPipeInfo, PIPE_SERVER_END, PIPE_TYPE_MESSAGE};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken, SetThreadToken};

    /// The bound of a "does not block" proof (K12): far above a local pipe's latency, never scaled (Global
    /// Constraints).
    const NO_BLOCK: Duration = Duration::from_secs(5);
    /// A SID that is not the test's own user.
    const OTHER_SID: &str = "S-1-5-21-1-2-3-1001";
    /// winnt.h's SYSTEM_MANDATORY_LABEL_NO_WRITE_UP and _NO_READ_UP, which windows-sys declares only under a feature
    /// Task 1 does not enable.
    const NO_WRITE_UP: u32 = 0x1;
    const NO_READ_UP: u32 = 0x2;
    /// winnt.h's ACCESS_ALLOWED_ACE_TYPE.
    const ALLOWED_ACE: u8 = 0x0;
    /// winnt.h's SYSTEM_MANDATORY_LABEL_ACE_TYPE, which windows-sys declares only under a feature Task 1 does not
    /// enable.
    const LABEL_ACE: u8 = 0x11;
    /// The SID of the medium mandatory integrity level, a non-elevated process's level.
    const MEDIUM_INTEGRITY_SID: &str = "S-1-16-8192";

    fn pipe_name() -> String {
        format!(r"\\.\pipe\{}", test_name("pipe"))
    }

    /// A client end of `name` as a test opens it: `CreateFileW(GENERIC_READ | GENERIC_WRITE, share 0, OPEN_EXISTING,
    /// FILE_FLAG_OVERLAPPED)`, then `pipe_stream`. Err is the open's raw OS error.
    fn open_client(name: &str) -> Result<IpcStream, i32> {
        let name = wide(name);
        // SAFETY: `name` is NUL-terminated and outlives the call; the attributes and the template may be null, for a
        // default descriptor, a handle no child inherits and no template.
        let raw = unsafe {
            CreateFileW(
                name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                null_mut(),
            )
        };
        if raw == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error().raw_os_error().expect("an operating-system error"));
        }
        // SAFETY: `raw` is the open handle CreateFileW just returned, which nothing else owns.
        let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
        Ok(pipe_stream(handle).expect("the client's stream"))
    }

    /// `mask` through `MapGenericMask` with the file mapping (`FILE_GENERIC_READ`, `FILE_GENERIC_WRITE`,
    /// `FILE_GENERIC_EXECUTE`, `FILE_ALL_ACCESS`), so a mask Windows kept generic and one it mapped at creation
    /// compare the same.
    fn file_mapped(mask: u32) -> u32 {
        let mapping = GENERIC_MAPPING {
            GenericRead: FILE_GENERIC_READ,
            GenericWrite: FILE_GENERIC_WRITE,
            GenericExecute: FILE_GENERIC_EXECUTE,
            GenericAll: FILE_ALL_ACCESS,
        };
        let mut mapped = mask;
        // SAFETY: both pointers point at locals that outlive the call, which rewrites `mapped` in place.
        unsafe { MapGenericMask(&mut mapped, &mapping) };
        mapped
    }

    /// The mandatory-label access control entries (ACEs) of `handle`'s system access control list, each as (its SID
    /// as `ConvertSidToStringSidW` writes it, its mask), in order, from one `GetSecurityInfo(handle, SE_KERNEL_OBJECT,
    /// LABEL_SECURITY_INFORMATION)`; the descriptor is freed with `LocalFree`. The discretionary list is read by
    /// `win::read_handle` (Ruling PF3-S10-12-5).
    fn labels_of(handle: &OwnedHandle) -> Vec<(String, u32)> {
        let (mut sacl, mut descriptor): (*mut ACL, PSECURITY_DESCRIPTOR) = (null_mut(), null_mut());
        // SAFETY: the handle is an open instance, whose duplex access holds READ_CONTROL; the owner, group and
        // discretionary list pointers may be null; the call writes the system list, which points into the descriptor
        // it allocates with LocalAlloc, and the descriptor.
        let status = unsafe {
            GetSecurityInfo(
                handle.as_raw_handle() as HANDLE,
                SE_KERNEL_OBJECT,
                LABEL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                null_mut(),
                &mut sacl,
                &mut descriptor,
            )
        };
        assert_eq!(status, ERROR_SUCCESS, "GetSecurityInfo: {}", io::Error::from_raw_os_error(status as i32));
        // The list in a binding whose address is never taken, so that the code scanner sees the null check below
        // cover its read.
        let sacl = sacl;
        // SAFETY: a non-null `sacl` points at the descriptor's system list, alive until the descriptor is freed below.
        let count = if sacl.is_null() { 0 } else { unsafe { (*sacl).AceCount } };
        let mut labels = Vec::new();
        for index in 0..u32::from(count) {
            let mut ace = null_mut();
            // SAFETY: `index` is below the list's AceCount; the call writes the entry's address to `ace`.
            assert_ne!(unsafe { GetAce(sacl, index, &mut ace) }, 0, "GetAce: {}", io::Error::last_os_error());
            // The entry in a binding whose address is never taken, as above.
            let ace = ace.cast::<SYSTEM_MANDATORY_LABEL_ACE>();
            let mask = if ace.is_null() {
                panic!("GetAce wrote no entry")
            } else {
                // SAFETY: a non-null `ace` points at the entry GetAce found, and every entry starts with an ACE_HEADER.
                let kind = unsafe { (*ace).Header.AceType };
                assert_eq!(kind, LABEL_ACE, "entry type {kind} is not a mandatory label");
                // SAFETY: as above, and a mandatory-label entry is a SYSTEM_MANDATORY_LABEL_ACE.
                unsafe { (*ace).Mask }
            };
            // A SYSTEM_MANDATORY_LABEL_ACE's SID starts at SidStart, inside the list.
            let sid = ace.cast::<u8>().wrapping_add(std::mem::offset_of!(SYSTEM_MANDATORY_LABEL_ACE, SidStart)).cast();
            labels.push((sid_string(sid).expect("the label's SID"), mask));
        }
        let descriptor = descriptor;
        if !descriptor.is_null() {
            // SAFETY: the call allocated the descriptor with LocalAlloc; it is freed once, here, after the last read
            // of the list inside it.
            unsafe { LocalFree(descriptor) };
        }
        labels
    }

    /// Ends the calling thread's impersonation when dropped.
    struct Revert;

    impl Drop for Revert {
        fn drop(&mut self) {
            // SAFETY: RevertToSelf takes nothing; it ends the calling thread's impersonation, if any.
            unsafe { RevertToSelf() };
        }
    }

    /// `raw`, a handle a call just wrote, owned so that it closes on drop.
    fn owned(raw: HANDLE) -> OwnedHandle {
        assert!(!raw.is_null(), "Windows returned no handle");
        // SAFETY: `raw` is an open handle that nothing else owns.
        unsafe { OwnedHandle::from_raw_handle(raw) }
    }

    /// Runs `work` on the calling thread while it impersonates a copy of the process token lowered to medium
    /// integrity: `OpenProcessToken(TOKEN_DUPLICATE | TOKEN_QUERY)`, `DuplicateTokenEx(TOKEN_ALL_ACCESS,
    /// SecurityImpersonation, TokenImpersonation)`, `SetTokenInformation(TokenIntegrityLevel)` with the SID
    /// `S-1-16-8192` and `crate::win::SE_GROUP_INTEGRITY`, `SetThreadToken(null, copy)`. `RevertToSelf` runs in a drop
    /// guard, so a failing `work` leaves no impersonation behind.
    fn as_medium_integrity<T>(work: impl FnOnce() -> T) -> T {
        let mut raw: HANDLE = null_mut();
        // SAFETY: GetCurrentProcess returns a pseudo handle that needs no closing; the call writes the token's handle
        // to `raw`.
        let opened = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_DUPLICATE | TOKEN_QUERY, &mut raw) };
        assert_ne!(opened, 0, "OpenProcessToken: {}", io::Error::last_os_error());
        let token = owned(raw);
        let mut raw: HANDLE = null_mut();
        // SAFETY: the token is open with TOKEN_DUPLICATE; the attributes pointer may be null, for a default descriptor
        // and a handle no child inherits; the call writes the copy's handle to `raw`.
        let copied = unsafe {
            DuplicateTokenEx(
                token.as_raw_handle() as HANDLE,
                TOKEN_ALL_ACCESS,
                null(),
                SecurityImpersonation,
                TokenImpersonation,
                &mut raw,
            )
        };
        assert_ne!(copied, 0, "DuplicateTokenEx: {}", io::Error::last_os_error());
        let copy = owned(raw);
        let text = wide(MEDIUM_INTEGRITY_SID);
        let mut sid: PSID = null_mut();
        // SAFETY: `text` is NUL-terminated and outlives the call; the call writes the SID it allocates with LocalAlloc
        // to `sid`.
        let converted = unsafe { ConvertStringSidToSidW(text.as_ptr(), &mut sid) };
        assert_ne!(converted, 0, "ConvertStringSidToSidW: {}", io::Error::last_os_error());
        // The SID in a binding whose address is never taken, checked before it is used.
        let sid = sid;
        assert!(!sid.is_null(), "Windows returned no SID");
        let label = TOKEN_MANDATORY_LABEL { Label: SID_AND_ATTRIBUTES { Sid: sid, Attributes: SE_GROUP_INTEGRITY } };
        // SAFETY: the copy is open with TOKEN_ALL_ACCESS, TOKEN_ADJUST_DEFAULT among it; `label` is the
        // TOKEN_MANDATORY_LABEL that TokenIntegrityLevel reads, the length passed is its size, and the SID it points
        // at stays allocated until after the call.
        let set = unsafe {
            SetTokenInformation(
                copy.as_raw_handle() as HANDLE,
                TokenIntegrityLevel,
                (&label as *const TOKEN_MANDATORY_LABEL).cast(),
                size_of::<TOKEN_MANDATORY_LABEL>() as u32,
            )
        };
        let error = io::Error::last_os_error();
        // SAFETY: ConvertStringSidToSidW allocated the SID with LocalAlloc; it is freed once, here, after its last use.
        unsafe { LocalFree(sid) };
        assert_ne!(set, 0, "SetTokenInformation: {error}");
        // SAFETY: a null thread pointer names the calling thread; the copy is an impersonation token open with
        // TOKEN_IMPERSONATE, and the thread keeps its own reference to it.
        let impersonating = unsafe { SetThreadToken(null(), copy.as_raw_handle() as HANDLE) };
        assert_ne!(impersonating, 0, "SetThreadToken: {}", io::Error::last_os_error());
        let _revert = Revert;
        work()
    }

    #[test]
    fn binding_a_held_name_fails_with_addr_in_use_naming_the_pipe() {
        let name = pipe_name();
        let _held = PipeServer::bind(&name).unwrap();
        let Err(error) = PipeServer::bind(&name) else { panic!("a second bind of {name} succeeded") };
        assert_eq!(error.kind(), io::ErrorKind::AddrInUse);
        assert_eq!(error.to_string(), format!("another process holds the pipe {name}"));
    }

    #[test]
    fn the_instance_carries_the_owner_and_system_list_and_the_high_label() {
        let server = PipeServer::bind(&pipe_name()).unwrap();
        let own = own_token().unwrap().user_sid;
        let listening = server.listening.lock().unwrap();
        let instance = listening.as_ref().unwrap();
        // The discretionary list through Task 2's read-back, each mask mapped by `file_mapped` (Ruling PF3-S10-12-5).
        let read_back = read_handle(instance.as_raw_handle() as HANDLE, SE_KERNEL_OBJECT).unwrap();
        let kinds: Vec<u8> = read_back.entries.iter().map(|entry| entry.kind).collect();
        assert_eq!(kinds, vec![ALLOWED_ACE, ALLOWED_ACE]);
        let allowed: Vec<(String, u32)> =
            read_back.entries.iter().map(|entry| (entry.sid.clone(), file_mapped(entry.mask))).collect();
        let read_write = FILE_GENERIC_READ | FILE_GENERIC_WRITE;
        assert!(read_back.protected);
        assert_eq!(allowed, vec![(own, read_write), (SYSTEM_SID.to_string(), read_write)]);
        let no_up = NO_WRITE_UP | NO_READ_UP;
        assert_eq!(labels_of(instance), vec![(HIGH_INTEGRITY_SID.to_string(), no_up)]);
    }

    #[test]
    fn the_pipe_rejects_remote_clients_as_written() {
        // Design part 12.4's "as written": no client from another machine can be made here, so the test reads the
        // flags the code passes and what Windows reports of the instance; neither GetNamedPipeInfo nor
        // GetNamedPipeHandleStateW reports the reject flag itself.
        assert_ne!(PIPE_MODE & PIPE_REJECT_REMOTE_CLIENTS, 0);
        let server = PipeServer::bind(&pipe_name()).unwrap();
        let listening = server.listening.lock().unwrap();
        let handle = listening.as_ref().unwrap().as_raw_handle() as HANDLE;
        let (mut flags, mut out_size, mut in_size, mut instances) = (0u32, 0u32, 0u32, 0u32);
        // SAFETY: `handle` is the server's open instance, kept open by the guard for the call; the four out pointers
        // are locals.
        let read = unsafe { GetNamedPipeInfo(handle, &mut flags, &mut out_size, &mut in_size, &mut instances) };
        assert_ne!(read, 0, "{}", io::Error::last_os_error());
        assert_ne!(flags & PIPE_SERVER_END, 0);
        assert_eq!(flags & PIPE_TYPE_MESSAGE, 0, "the byte type (PIPE_TYPE_BYTE is 0)");
        assert_eq!(instances, PIPE_UNLIMITED_INSTANCES);
    }

    #[test]
    fn a_lowered_token_cannot_open_the_pipe() {
        let name = pipe_name();
        let _server = PipeServer::bind(&name).unwrap();
        let opened = thread::spawn(move || as_medium_integrity(|| open_client(&name).err())).join().unwrap();
        // ERROR_ACCESS_DENIED: the high label refuses a medium-integrity token of the same user (P6). This tests the
        // label, not the engine's own token check.
        assert_eq!(opened, Some(5));
    }

    #[test]
    fn a_client_of_another_user_is_refused_with_permission_denied_naming_its_process() {
        let name = pipe_name();
        let server = PipeServer::bind_as(&name, OTHER_SID).unwrap();
        let refusal = format!("process {}: its user differs from this process's user", std::process::id());
        // The second client reaches the next accept and is refused the same way: the server serves on.
        for _ in 0..2 {
            let mut client = open_client(&name).unwrap();
            let Err(error) = server.accept() else { panic!("a client of another user was handed out") };
            assert_eq!((error.kind(), error.to_string()), (io::ErrorKind::PermissionDenied, refusal.clone()));
            assert_eq!(line(&mut *client.reader), "");
        }
    }

    #[test]
    fn a_push_to_a_silent_client_does_not_block() {
        let name = pipe_name();
        let server = PipeServer::bind(&name).unwrap();
        let mut client = open_client(&name).unwrap();
        let IpcStream { reader, mut writer } = server.accept().unwrap();
        // The server's reader waits for a line the silent client never sends (K12).
        let pending = thread::spawn(move || {
            let mut reader = reader;
            line(&mut *reader)
        });
        thread::sleep(Duration::from_millis(100));
        // As `Clients::add`'s writer thread writes each line (ipc_server.rs): write_all, then flush.
        let writer = within(NO_BLOCK, move || {
            for n in 0..10 {
                writer.write_all(format!("{{\"n\":{n}}}\n").as_bytes()).unwrap();
                writer.flush().unwrap();
            }
            writer
        });
        for n in 0..10 {
            assert_eq!(line(&mut *client.reader), format!("{{\"n\":{n}}}\n"));
        }
        drop(client);
        assert_eq!(within(NO_BLOCK, move || pending.join().unwrap()), "");
        drop(writer);
    }

    #[test]
    fn two_clients_at_once_each_read_their_own_lines() {
        let name = pipe_name();
        let server = PipeServer::bind(&name).unwrap();
        let mut first = open_client(&name).unwrap();
        let mut to_first = server.accept().unwrap();
        let mut second = open_client(&name).unwrap();
        let mut to_second = server.accept().unwrap();
        to_second.writer.write_all(b"second\n").unwrap();
        to_first.writer.write_all(b"first\n").unwrap();
        assert_eq!(line(&mut *first.reader), "first\n");
        assert_eq!(line(&mut *second.reader), "second\n");
    }

    #[test]
    fn the_name_stays_held_while_a_connection_is_handed_out() {
        let name = pipe_name();
        let server = PipeServer::bind(&name).unwrap();
        let _first = open_client(&name).unwrap();
        let _handed_out = server.accept().unwrap();
        // The next instance exists before accept returns, so a client neither finds the pipe busy (231) nor finds no
        // pipe (2), and needs no wait.
        assert_eq!(open_client(&name).err(), None);
    }

    #[test]
    fn the_last_queued_line_reaches_the_client_before_the_pipe_closes() {
        let name = pipe_name();
        let server = PipeServer::bind(&name).unwrap();
        let mut client = open_client(&name).unwrap();
        let IpcStream { reader, mut writer } = server.accept().unwrap();
        let reading = thread::spawn(move || {
            let mut reader = reader;
            line(&mut *reader)
        });
        let ok = "{\"v\":1,\"type\":\"ok\",\"id\":1,\"payload\":{\"result\":null}}\n";
        writer.write_all(ok.as_bytes()).unwrap();
        drop(writer);
        // Dropping the writer ends the server's own pending read, so its reader half goes and the handle closes,
        // with the line still unread by the client (Review Focus 2).
        assert_eq!(within(NO_BLOCK, move || reading.join().unwrap()), "");
        assert_eq!(line(&mut *client.reader), ok);
        assert_eq!(line(&mut *client.reader), "");
    }

    #[test]
    fn a_write_after_the_client_closed_is_broken_pipe_and_the_reader_reads_end_of_file() {
        let name = pipe_name();
        let server = PipeServer::bind(&name).unwrap();
        let client = open_client(&name).unwrap();
        let IpcStream { reader, mut writer } = server.accept().unwrap();
        drop(client);
        let ended = within(NO_BLOCK, move || {
            let mut reader = reader;
            line(&mut *reader)
        });
        assert_eq!(ended, "");
        assert_eq!(writer.write_all(b"late\n").unwrap_err().kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn a_client_that_opens_and_closes_before_accept_is_connection_aborted_and_the_next_is_served() {
        let name = pipe_name();
        let server = PipeServer::bind(&name).unwrap();
        drop(open_client(&name).unwrap());
        let Err(gone) = server.accept() else { panic!("a client that had closed was handed out") };
        assert_eq!(gone.kind(), io::ErrorKind::ConnectionAborted);
        let mut client = open_client(&name).unwrap();
        let mut stream = server.accept().unwrap();
        stream.writer.write_all(b"served\n").unwrap();
        assert_eq!(line(&mut *client.reader), "served\n");
    }

    #[test]
    fn a_client_connected_before_accept_is_handed_out() {
        let name = pipe_name();
        let server = PipeServer::bind(&name).unwrap();
        let mut client = open_client(&name).unwrap();
        // ConnectNamedPipe meets ERROR_PIPE_CONNECTED (535).
        let mut stream = server.accept().unwrap();
        client.writer.write_all(b"{\"v\":1}\n").unwrap();
        assert_eq!(line(&mut *stream.reader), "{\"v\":1}\n");
    }

    #[test]
    fn close_wakes_a_waiting_accept_with_broken_pipe() {
        let name = pipe_name();
        let server = Arc::new(PipeServer::bind(&name).unwrap());
        let waiting = Arc::clone(&server);
        let (done, result) = channel();
        thread::spawn(move || {
            let _ = done.send(waiting.accept().err().map(|error| error.kind()));
        });
        thread::sleep(Duration::from_millis(100));
        assert!(result.try_recv().is_err(), "accept returned before close");
        server.close();
        assert_eq!(result.recv_timeout(NO_BLOCK).unwrap(), Some(io::ErrorKind::BrokenPipe));
        assert_eq!(server.accept().err().map(|error| error.kind()), Some(io::ErrorKind::BrokenPipe));
        // ERROR_FILE_NOT_FOUND: no instance holds the name any more, so lazymkvctl would say no engine is running.
        assert_eq!(open_client(&name).err(), Some(2));
    }

    #[test]
    fn a_line_of_four_mebibytes_and_one_byte_passes_the_transport_intact() {
        let name = pipe_name();
        let server = PipeServer::bind(&name).unwrap();
        let IpcStream { reader: _kept, writer: mut client_writer } = open_client(&name).unwrap();
        let mut stream = server.accept().unwrap();
        let mut big = vec![b'x'; 4 * 1024 * 1024 + 1];
        big.push(b'\n');
        let sent = big.clone();
        let writing = thread::spawn(move || client_writer.write_all(&sent).map(|()| client_writer));
        let got = within(Duration::from_secs(20) * ci_factor(), move || {
            let mut got = Vec::new();
            stream.reader.read_until(b'\n', &mut got).unwrap();
            got
        });
        assert_eq!(got.len(), big.len());
        assert!(got == big, "the line came through changed");
        assert!(writing.join().unwrap().is_ok());
    }

    #[test]
    fn errors_of_a_client_gone_before_its_check_are_connection_aborted_and_none_reads_as_closed() {
        for code in [109, 232, 233] {
            assert_eq!(accept_error(code).kind(), io::ErrorKind::ConnectionAborted, "{code}");
        }
        // An access-denied connect or creation is one failed accept, never a refused client (Ruling PF3-S10-12-2).
        let denied = accept_error(5);
        assert_eq!(denied.kind(), io::ErrorKind::Other);
        assert!(denied.to_string().contains("os error 5"), "{denied}");
        for code in [6, 87, 995] {
            assert_eq!(accept_error(code).raw_os_error(), Some(code as i32), "{code}");
        }
        assert_eq!(not_closed(io::Error::from_raw_os_error(109)).kind(), io::ErrorKind::Other);
        assert_eq!(not_closed(io::Error::from_raw_os_error(5)).kind(), io::ErrorKind::Other);
        // WSAECONNABORTED, the operating-system error whose kind is ConnectionAborted.
        assert_eq!(not_closed(io::Error::from_raw_os_error(10053)).kind(), io::ErrorKind::Other);
        assert_eq!(not_closed(io::Error::from_raw_os_error(87)).raw_os_error(), Some(87));
        // Only operating-system errors are mapped: the client check's own refusal keeps its kind.
        let refused = io::Error::new(io::ErrorKind::PermissionDenied, "process 1: its token is not elevated");
        assert_eq!(not_closed(refused).kind(), io::ErrorKind::PermissionDenied);
    }
}
