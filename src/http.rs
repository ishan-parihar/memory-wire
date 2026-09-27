//! Minimal dependency-free HTTP/1.1 client for loopback traffic.
//!
//! The project ships as one static binary with "no runtime" as a headline
//! claim, so shelling out to `curl` would quietly break it — and there is no
//! HTTP client in the dependency set. Every call a hook or `doctor` makes is
//! small JSON against a known server, so a `TcpStream`, a `Connection: close`
//! request, and a read to EOF is the whole client.
//!
//! `https://` is not supported: TLS needs a dependency, and a caller that wants
//! TLS should front the server with a terminating proxy.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/// A parsed HTTP response. A non-2xx status is a value, not an error: `404`
/// from a server without a route is information, not a fault.
#[derive(Debug, Clone)]
pub struct Response {
    /// Status code from the status line.
    pub status: u16,
    /// Body bytes, decoded as UTF-8 lossily.
    pub body: String,
}

impl Response {
    /// True for 2xx.
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// `GET url`, reading up to `timeout` for connect and transfer.
pub fn get(url: &str, timeout: Duration) -> std::io::Result<Response> {
    request("GET", url, None, timeout)
}

/// `POST url` with a JSON body.
pub fn post_json(url: &str, body: &str, timeout: Duration) -> std::io::Result<Response> {
    request("POST", url, Some(body), timeout)
}

struct Target {
    host: String,
    port: u16,
    path: String,
}

fn io_err(msg: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, msg.into())
}

/// Split `http://host[:port][/path]` into its parts.
fn parse_url(url: &str) -> std::io::Result<Target> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| io_err(format!("only http:// URLs are supported, got {url}")))?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if authority.is_empty() {
        return Err(io_err(format!("no host in URL {url}")));
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().map_err(|_| io_err(format!("bad port in {url}")))?),
        None => (authority, 80),
    };
    Ok(Target {
        host: host.to_string(),
        port,
        path: path.to_string(),
    })
}

/// Budget left against `deadline`.
///
/// Floored at a millisecond because a socket rejects a zero timeout outright,
/// and a spent deadline must fail fast rather than buy another full period.
fn remaining(deadline: Instant) -> Duration {
    deadline
        .saturating_duration_since(Instant::now())
        .max(Duration::from_millis(1))
}

/// Send one request, read one response.
///
/// `timeout` is one total deadline rather than a per-phase allowance. It is
/// recorded before the host is even resolved, so name resolution — which has no
/// timeout of its own — spends the same budget as the connect, the write, and
/// the read. Handing each phase the full `timeout` made a server that accepted
/// and then stalled cost `3 × timeout`, which is the difference between a hook
/// that is briefly slow and a session that hangs.
fn request(
    method: &str,
    url: &str,
    body: Option<&str>,
    timeout: Duration,
) -> std::io::Result<Response> {
    let deadline = Instant::now() + timeout;
    let t = parse_url(url)?;
    let addr = (t.host.as_str(), t.port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| io_err(format!("no address for {}", t.host)))?;
    let mut sock = TcpStream::connect_timeout(&addr, remaining(deadline))?;
    sock.set_read_timeout(Some(remaining(deadline)))?;
    sock.set_write_timeout(Some(remaining(deadline)))?;
    sock.set_nodelay(true).ok();

    let mut req = format!(
        "{method} {} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\nUser-Agent: memory-wire/{}\r\nAccept: */*\r\n",
        t.path,
        t.host,
        t.port,
        env!("CARGO_PKG_VERSION")
    );
    if let Some(b) = body {
        req.push_str("Content-Type: application/json\r\n");
        req.push_str(&format!("Content-Length: {}\r\n", b.len()));
    }
    req.push_str("\r\n");
    sock.write_all(req.as_bytes())?;
    if let Some(b) = body {
        sock.write_all(b.as_bytes())?;
    }
    sock.flush()?;

    // `Connection: close` means the server closes when the body is done, so
    // read-to-EOF sidesteps both content-length and chunked framing.
    let mut raw = Vec::new();
    sock.read_to_end(&mut raw)?;
    parse_response(&raw)
}

/// Split raw bytes at the header/body boundary and pull the status code.
fn parse_response(raw: &[u8]) -> std::io::Result<Response> {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| io_err("truncated response: no header terminator"))?;
    let status_line = head.lines().next().unwrap_or_default();
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse::<u16>().ok())
        .ok_or_else(|| io_err(format!("no status code in {status_line:?}")))?;
    Ok(Response {
        status,
        body: body.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    // One loopback round trip: request bytes in, response parsed out. The
    // listener lives on an ephemeral port, so nothing outside this process
    // is contacted and nothing real is needed.
    #[test]
    fn client_should_post_json_and_parse_response() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().expect("accept");
            // Read the whole request, not one packet: a single `read` can
            // return headers only, and answering mid-request resets the
            // connection on the client side.
            let mut raw = Vec::new();
            let mut buf = [0u8; 1024];
            loop {
                let n = sock.read(&mut buf).expect("read");
                if n == 0 {
                    break;
                }
                raw.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&raw).into_owned();
                if let Some(head_end) = text.find("\r\n\r\n") {
                    let want: usize = text[..head_end]
                        .to_ascii_lowercase()
                        .split("content-length:")
                        .nth(1)
                        .and_then(|r| r.split("\r\n").next())
                        .and_then(|n| n.trim().parse().ok())
                        .unwrap_or(0);
                    if raw.len() >= head_end + 4 + want {
                        break;
                    }
                }
            }
            sock.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 13\r\n\r\n[\"jose\",\"jwt\"]",
            )
            .expect("write");
            String::from_utf8_lossy(&raw).into_owned()
        });

        let resp = post_json(
            &format!("http://{addr}/banks/demo/recall"),
            r#"{"query":"jose"}"#,
            Duration::from_secs(5),
        )
        .expect("request");

        let req = server.join().expect("join");
        assert!(resp.ok());
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, "[\"jose\",\"jwt\"]");
        assert!(req.starts_with("POST /banks/demo/recall HTTP/1.1\r\n"), "{req}");
        assert!(req.contains("Content-Type: application/json\r\n"));
        assert!(req.contains("Content-Length: 16\r\n"), "{req}");
        assert!(req.ends_with(r#"{"query":"jose"}"#), "{req}");
    }

    // A server that accepts the connection and then never says anything: the
    // read phase can only end at the deadline, so this is where a per-phase
    // timeout would show up as `3 × timeout` instead of one.
    #[test]
    fn a_stalled_server_should_cost_one_timeout_not_one_per_phase() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let hold_open = Duration::from_secs(30);
        std::thread::spawn(move || {
            while let Ok((sock, _)) = listener.accept() {
                // Answer nothing, and keep the socket open well past the
                // client's deadline: dropping it would make the read an instant
                // EOF and the test would pass without ever hitting a timeout.
                std::thread::sleep(hold_open);
                drop(sock);
            }
        });

        let timeout = Duration::from_millis(400);
        let started = std::time::Instant::now();
        let err = get(&format!("http://{addr}/health"), timeout).expect_err("a stall must fail");
        let elapsed = started.elapsed();
        assert!(
            elapsed < 2 * timeout,
            "one deadline, not one per phase: {elapsed:?} for a {timeout:?} budget"
        );
        assert!(
            matches!(
                err.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ),
            "the request must end at the deadline, not by accident: {err}"
        );
    }

    // The budget the phases share: it shrinks as the request runs, and a
    // deadline already spent fails fast instead of handing a socket the zero
    // timeout it rejects (or a fresh full period it should not have).
    #[test]
    fn remaining_should_shrink_and_never_reach_zero() {
        let deadline = Instant::now() + Duration::from_millis(50);
        let left = remaining(deadline);
        assert!(left <= Duration::from_millis(50) && left >= Duration::from_millis(1), "{left:?}");
        let spent = Instant::now() - Duration::from_secs(1);
        assert_eq!(remaining(spent), Duration::from_millis(1));
    }

    #[test]
    fn parse_url_should_default_the_port_and_path() {
        let t = parse_url("http://127.0.0.1:8888/banks/a/recall").expect("parse");
        assert_eq!((t.host.as_str(), t.port, t.path.as_str()), ("127.0.0.1", 8888, "/banks/a/recall"));
        let t = parse_url("http://localhost").expect("parse");
        assert_eq!((t.host.as_str(), t.port, t.path.as_str()), ("localhost", 80, "/"));
        assert!(parse_url("https://example.com").is_err());
    }

    #[test]
    fn parse_response_should_reject_a_malformed_reply() {
        assert!(parse_response(b"garbage").is_err());
        let r = parse_response(b"HTTP/1.1 404 Not Found\r\n\r\nnope").expect("parse");
        assert_eq!(r.status, 404);
        assert!(!r.ok());
    }
}
