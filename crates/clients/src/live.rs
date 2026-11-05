//! The live transport (design part 8.1): `ureq` over rustls on ring with the compiled-in `webpki-roots`. One agent
//! per transport, every status an answer, no redirect followed (Ruling T4-R12), no proxy read from the environment,
//! and only the caller's own `User-Agent`. A failure's text is one of six fixed sentences chosen by the error's
//! variant alone, so no URL, header, host or port ever reaches it (engine.md, "the token kept out of error text").

use std::io;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use ureq::config::AutoHeaderValue;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
use ureq::Agent;

use crate::transport::{HttpTransport, Response, TransportError, TIMEOUT_S};

/// The longest answer body read, in bytes: 8 MiB. The cap is exclusive: a body of 8 MiB or longer is
/// `Unreachable(BODY_TOO_LONG)`. Design part 8.1.
/// Def: T4-7. Use: T4-7.
pub const MAX_BODY_BYTES: u64 = 8 * 1024 * 1024;

/// The text of a refused connection. Design part 8.1. Def: T4-7. Use: T4-7.
pub const REFUSED: &str = "the connection was refused";

/// The text of a host name that could not be resolved. Design part 8.1. Def: T4-7. Use: T4-7.
pub const NO_HOST: &str = "the host name could not be resolved";

/// The text of a failed Transport Layer Security (TLS) handshake. Design part 8.1. Def: T4-7. Use: T4-7.
pub const TLS_FAILED: &str = "the TLS handshake failed";

/// The text of any other failed connection. Design part 8.1. Def: T4-7. Use: T4-7.
pub const CONNECTION_FAILED: &str = "the connection failed";

/// The text of an answer body longer than [`MAX_BODY_BYTES`]. Design part 8.1. Def: T4-7. Use: T4-7.
pub const BODY_TOO_LONG: &str = "the answer is longer than 8 MiB";

/// The text of an answer that could not be read for any other reason. Design part 8.1. Def: T4-7. Use: T4-7.
pub const BAD_ANSWER: &str = "the answer could not be read";

/// The live HTTP transport (design part 8.1): one `ureq` agent built with the timeout, every status an `Ok(Response)`,
/// no redirect followed so a bearer token never reaches another host (Ruling T4-R12), no proxy from the environment,
/// no `User-Agent` but the caller's, and bodies read up to [`MAX_BODY_BYTES`]. Def: T4-7. Use: T4-10, T4-17, T4-19.
pub struct UreqTransport {
    agent: Agent,
    timeout: Duration,
}

impl UreqTransport {
    /// The production transport, timing out after [`TIMEOUT_S`] seconds (design part 8.1). Def: T4-7. Use: T4-10,
    /// T4-17, T4-19.
    #[allow(clippy::new_without_default)] // a Default would hide that this builds the live transport
    pub fn new() -> UreqTransport {
        UreqTransport::with_timeout(Duration::from_secs(TIMEOUT_S))
    }

    /// A transport whose every request and connection gives up after `timeout` (design part 8.1; the tests pass a
    /// short one). Def: T4-7. Use: T4-10, T4-17, T4-19.
    pub fn with_timeout(timeout: Duration) -> UreqTransport {
        let tls = TlsConfig::builder().provider(TlsProvider::Rustls).root_certs(RootCerts::WebPki).build();
        let config = Agent::config_builder()
            .timeout_global(Some(timeout))
            .http_status_as_error(false)
            .max_redirects(0)
            .max_redirects_will_error(false)
            .proxy(None)
            .user_agent(AutoHeaderValue::None)
            .tls_config(tls)
            .build();
        UreqTransport { agent: Agent::new_with_config(config), timeout }
    }

    /// The timeout this transport was built with (design part 8.1). Def: T4-7. Use: T4-10, T4-17, T4-19.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

impl HttpTransport for UreqTransport {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, TransportError> {
        let mut request = self.agent.get(url);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        answer(request.call())
    }

    fn post(&self, url: &str, headers: &[(&str, &str)], body: &[u8]) -> Result<Response, TransportError> {
        let mut request = self.agent.post(url);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        answer(request.send(body))
    }

    fn connect(&self, host: &str, port: u16) -> Result<(), TransportError> {
        let started = Instant::now();
        let addresses = match (host, port).to_socket_addrs() {
            Ok(found) => found.collect::<Vec<_>>(),
            Err(_) => return Err(unreachable(NO_HOST)),
        };
        let mut last = unreachable(NO_HOST);
        for address in addresses {
            let left = self.timeout.saturating_sub(started.elapsed());
            if left.is_zero() {
                return Err(TransportError::Timeout);
            }
            match TcpStream::connect_timeout(&address, left) {
                Ok(stream) => {
                    drop(stream);
                    return Ok(());
                }
                Err(error) => last = from_io(&error),
            }
        }
        Err(last)
    }
}

/// The status and the body read up to [`MAX_BODY_BYTES`], or the failure in fixed words.
fn answer(sent: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<Response, TransportError> {
    let mut response = sent.map_err(from_ureq)?;
    let status = response.status().as_u16();
    let body = response.body_mut().with_config().limit(MAX_BODY_BYTES).read_to_vec().map_err(from_ureq)?;
    Ok(Response { status, body })
}

fn unreachable(text: &str) -> TransportError {
    TransportError::Unreachable(text.to_string())
}

/// A `ureq` error by its variant alone; its `Display` and `Debug` may hold the URL, so neither is read.
fn from_ureq(error: ureq::Error) -> TransportError {
    match error {
        ureq::Error::Timeout(_) => TransportError::Timeout,
        ureq::Error::Io(error) => from_io(&error),
        ureq::Error::HostNotFound => unreachable(NO_HOST),
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) => unreachable(TLS_FAILED),
        ureq::Error::ConnectionFailed => unreachable(CONNECTION_FAILED),
        ureq::Error::BodyExceedsLimit(_) => unreachable(BODY_TOO_LONG),
        _ => unreachable(BAD_ANSWER),
    }
}

/// An I/O error by its kind alone; its text may name the address, so it is not read.
fn from_io(error: &io::Error) -> TransportError {
    match error.kind() {
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => TransportError::Timeout,
        io::ErrorKind::ConnectionRefused => unreachable(REFUSED),
        _ => unreachable(CONNECTION_FAILED),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::user_agent;
    use std::io::{ErrorKind, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{mpsc, Mutex};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    const SENTINEL: &str = "token-sentinel-5d2e";
    const HOOK: &str = "/api/webhook/hook-sentinel-7f3a";
    const QUICK: Duration = Duration::from_millis(500);
    /// Windows answers a connection to a closed loopback port only after about 2 s of connection retries.
    const REFUSAL: Duration = Duration::from_secs(5);
    /// Taken by every test that sends or reads the environment, and not by `the_production_timeout_is_ten_seconds`:
    /// `no_proxy_is_read_from_the_environment` sets proxy variables process-wide.
    static ENV: Mutex<()> = Mutex::new(());

    fn answer(status: &str, body: &[u8]) -> Vec<u8> {
        let mut bytes = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
        bytes.extend_from_slice(body);
        bytes
    }

    /// The request's head, then as many body bytes as its Content-Length names.
    fn read_request(stream: &mut TcpStream) -> Vec<u8> {
        let (mut bytes, mut byte) = (Vec::new(), [0u8; 1]);
        while !bytes.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap_or(0) == 1 { bytes.push(byte[0]); }
        let head = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
        let length = head.lines().find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse::<usize>().ok()).unwrap_or(0);
        let mut body = vec![0u8; length];
        let _ = stream.read_exact(&mut body);
        bytes.extend(body);
        bytes
    }

    /// A loopback listener answering `answers` to one connection each, in order; joins to each request's bytes.
    fn serve(answers: Vec<Vec<u8>>) -> (String, JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let handle = thread::spawn(move || answers.into_iter().map(|answer| {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            let _ = stream.write_all(&answer);
            String::from_utf8_lossy(&request).into_owned()
        }).collect());
        (base, handle)
    }

    /// A port nothing listens on: bound, then dropped.
    fn closed_port() -> u16 { TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port() }

    /// Whether nothing connected to `listener`.
    fn untouched(listener: &TcpListener) -> bool {
        listener.set_nonblocking(true).unwrap();
        matches!(listener.accept(), Err(error) if error.kind() == ErrorKind::WouldBlock)
    }

    fn bearer() -> String { format!("Bearer {SENTINEL}") }

    #[test]
    fn a_get_sends_the_headers_it_is_given_and_reads_any_status() {
        let _env = ENV.lock().unwrap_or_else(|p| p.into_inner());
        let (base, seen) = serve(vec![answer("404 Not Found", b"gone"), answer("500 Internal Server Error", b"oops")]);
        let t = UreqTransport::with_timeout(QUICK);
        let (ua, auth) = (user_agent(), bearer());
        let headers = [("Authorization", auth.as_str()), ("User-Agent", ua.as_str())];
        assert_eq!(t.get(&format!("{base}/a"), &headers), Ok(Response { status: 404, body: b"gone".to_vec() }));
        assert_eq!(t.get(&format!("{base}/b"), &headers), Ok(Response { status: 500, body: b"oops".to_vec() }));
        for request in seen.join().unwrap() {
            let head = request.to_ascii_lowercase();
            assert!(head.contains(&format!("user-agent: {}", user_agent())), "{head}");
            assert_eq!(head.matches("user-agent:").count(), 1, "ureq adds no User-Agent of its own: {head}");
            assert!(head.contains(&format!("authorization: bearer {SENTINEL}")), "{head}");
        }
    }

    #[test]
    fn a_post_sends_its_body_and_content_type() {
        let _env = ENV.lock().unwrap_or_else(|p| p.into_inner());
        let (base, seen) = serve(vec![answer("200 OK", b"")]);
        let ua = user_agent();
        let headers = [("Content-Type", "application/json"), ("User-Agent", ua.as_str())];
        let sent = UreqTransport::with_timeout(QUICK).post(&format!("{base}{HOOK}"), &headers, br#"{"event":"filed"}"#);
        assert_eq!(sent, Ok(Response { status: 200, body: Vec::new() }));
        let request = seen.join().unwrap().remove(0);
        assert!(request.starts_with(&format!("POST {HOOK} HTTP/1.1\r\n")), "{request}");
        assert!(request.to_ascii_lowercase().contains("content-type: application/json"), "{request}");
        assert!(request.ends_with("\r\n\r\n{\"event\":\"filed\"}"), "{request}");
    }

    #[test]
    fn a_refused_port_is_unreachable_and_the_text_holds_no_url_or_header() {
        let _env = ENV.lock().unwrap_or_else(|p| p.into_inner());
        let url = format!("http://127.0.0.1:{}{HOOK}", closed_port());
        let t = UreqTransport::with_timeout(REFUSAL);
        let auth = bearer();
        let got = t.get(&url, &[("Authorization", auth.as_str())]);
        let posted = t.post(&url, &[("Authorization", auth.as_str())], b"{}");
        for error in [got, posted] {
            assert_eq!(error, Err(TransportError::Unreachable(REFUSED.to_string())));
            let text = format!("{error:?}");
            for secret in [SENTINEL, HOOK, "hook-sentinel-7f3a", "127.0.0.1"] { assert!(!text.contains(secret), "{text}"); }
        }
    }

    #[test]
    fn a_silent_server_times_out() {
        let _env = ENV.lock().unwrap_or_else(|p| p.into_inner());
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}/x", listener.local_addr().unwrap().port());
        let (done, wait) = mpsc::channel::<()>();
        let holder = thread::spawn(move || { let held = listener.accept(); let _ = wait.recv(); drop(held); });
        let started = Instant::now();
        assert_eq!(UreqTransport::with_timeout(QUICK).get(&url, &[]), Err(TransportError::Timeout));
        // A "gives up in time" bound: it does not scale with ci_factor (Global Constraints, Tests).
        assert!(started.elapsed() < QUICK * 4, "{:?}", started.elapsed());
        done.send(()).unwrap();
        holder.join().unwrap();
    }

    #[test]
    fn a_redirect_is_not_followed_and_reads_as_its_status() {
        let _env = ENV.lock().unwrap_or_else(|p| p.into_inner());
        let elsewhere = TcpListener::bind("127.0.0.1:0").unwrap();
        let location = format!("http://127.0.0.1:{}/login", elsewhere.local_addr().unwrap().port());
        let moved = format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        let (base, seen) = serve(vec![moved.into_bytes()]);
        let auth = bearer();
        let got = UreqTransport::with_timeout(QUICK).get(&format!("{base}/movie/1"), &[("Authorization", auth.as_str())]);
        assert_eq!(got, Ok(Response { status: 302, body: Vec::new() }));
        assert_eq!(seen.join().unwrap().len(), 1, "the bearer token went to the first host once");
        assert!(untouched(&elsewhere), "the redirect's target saw a connection");
    }

    #[test]
    fn a_body_over_8_mib_is_unreachable_and_says_so() {
        let _env = ENV.lock().unwrap_or_else(|p| p.into_inner());
        let body = vec![b'x'; (MAX_BODY_BYTES + 1) as usize];
        let (base, seen) = serve(vec![answer("200 OK", &body)]);
        let got = UreqTransport::with_timeout(REFUSAL).get(&format!("{base}/big"), &[]);
        assert_eq!(got, Err(TransportError::Unreachable(BODY_TOO_LONG.to_string())));
        let _ = seen.join();
    }

    #[test]
    fn no_proxy_is_read_from_the_environment() {
        let _env = ENV.lock().unwrap_or_else(|p| p.into_inner());
        let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
        let named = format!("http://127.0.0.1:{}", proxy.local_addr().unwrap().port());
        let names = ["HTTP_PROXY", "http_proxy", "HTTPS_PROXY", "ALL_PROXY", "all_proxy"];
        for name in names { std::env::set_var(name, &named); }
        let (base, seen) = serve(vec![answer("200 OK", b"direct")]);
        let got = UreqTransport::with_timeout(QUICK).get(&format!("{base}/x"), &[]);
        for name in names { std::env::remove_var(name); }
        assert_eq!(got, Ok(Response { status: 200, body: b"direct".to_vec() }));
        assert_eq!(seen.join().unwrap().len(), 1);
        assert!(untouched(&proxy), "the proxy the environment names saw a connection");
    }

    #[test]
    fn connect_reaches_a_listening_port_and_refuses_a_closed_one() {
        let _env = ENV.lock().unwrap_or_else(|p| p.into_inner());
        let open = TcpListener::bind("127.0.0.1:0").unwrap();
        let t = UreqTransport::with_timeout(REFUSAL);
        assert_eq!(t.connect("127.0.0.1", open.local_addr().unwrap().port()), Ok(()));
        assert_eq!(t.connect("127.0.0.1", closed_port()), Err(TransportError::Unreachable(REFUSED.to_string())));
    }

    #[test]
    fn the_production_timeout_is_ten_seconds() {
        // Construction only: nothing is sent (Global Constraints, No network on CI).
        assert_eq!(UreqTransport::new().timeout(), Duration::from_secs(TIMEOUT_S));
        assert_eq!(UreqTransport::with_timeout(QUICK).timeout(), QUICK);
    }
}
