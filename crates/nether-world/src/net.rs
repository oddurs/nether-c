//! Strata 5 and 6: the network.
//!
//! `spec/09-prelude.md` §9.6. Reading is depth 5 and writing is depth 6,
//! because §1.8 orders them by how much of the world a mistake reaches: a
//! `get` that is wrong is a build that is wrong, and a `post` that is wrong is
//! something a remote party now remembers.
//!
//! Every response is sealed on arrival, before the program is told, so replay
//! serves it from the ledger and opens nothing. That is the whole of what this
//! stratum is for, and it is [`crate::Replay`] rather than anything here.
//!
//! ## What this build serves
//!
//! `http`, and not `https`. §9.6 requires an implementation to say which, and
//! to refuse the rest with `denied` rather than `unreachable` — this build
//! saying no, not the world failing to answer. `https` is TLS, and
//! `spec/90-rationale.md` has the argument and what it costs.

use core::fmt::Write as _;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use nether_core::{Capability, Depth};
use nether_ledger::{Call, Refusal, Span, Store, Stored, Value};

use crate::provider::{Provider, Refuse, given, refused};
use crate::recorder::{Recorded, Recorder};

/// How long a socket may take to say anything. §6.4's argument, one layer out:
/// a burial with no bound on it is a burial that does not finish.
const PATIENCE: Duration = Duration::from_secs(30);

/// How much of a response is read. A body larger than this is `exhausted`,
/// which is a refusal the program can see rather than a machine that stops.
const MOST: u64 = 64 * 1024 * 1024;

/// The network, reaching only where the invocation said. §8.3.2.
pub struct Net {
    reach: Vec<String>,
    /// Whether stratum 6 was granted as well as stratum 5.
    sending: bool,
}

impl Net {
    /// Fetching only: `get`, at depth 5.
    #[must_use]
    pub fn fetching(reach: Vec<String>) -> Self {
        Self { reach, sending: false }
    }

    /// Fetching and sending: `post` as well, at depth 6.
    #[must_use]
    pub fn sending(reach: Vec<String>) -> Self {
        Self { reach, sending: true }
    }

    /// Whether §8.3.2 declared this authority.
    ///
    /// A bare host matches any port, because the port is a detail of the
    /// service and the host is the party being trusted. It matches that host
    /// exactly: a subdomain is a different party, and a reach that spread to
    /// one would be a reach nobody declared.
    fn reaches(&self, host: &str, port: u16) -> bool {
        self.reach.iter().any(|named| match named.rsplit_once(':') {
            Some((h, p)) => h == host && p.parse() == Ok(port),
            None => named == host,
        })
    }
}

/// A URL, as far as §9.6 needs one.
struct Asked {
    host: String,
    port: u16,
    path: String,
}

/// Split a URL, or say which no it is.
///
/// Hand-written, like everything else here. What §9.6 needs is the scheme, the
/// authority and the rest — a general URL parser would be a larger thing that
/// this has no use for.
fn split(url: &str) -> Result<Asked, Refusal> {
    // §9.6: a scheme this build will not serve is `denied`, because `denied`
    // is this build saying no and `unreachable` is the world not answering.
    let Some(("http", rest)) = url.split_once("://") else { return Err(Refusal::Denied) };
    let (authority, path) = rest.split_once('/').map_or((rest, ""), |(a, p)| (a, p));
    if authority.is_empty() {
        return Err(Refusal::Malformed);
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h, p.parse().map_err(|_| Refusal::Malformed)?),
        None => (authority, 80u16),
    };
    if host.is_empty() {
        return Err(Refusal::Malformed);
    }
    Ok(Asked { host: host.to_owned(), port, path: format!("/{path}") })
}

/// The refusal an `io::Error` is. §9.6 lists which each function may give.
fn why(e: &std::io::Error) -> Refusal {
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => Refusal::Denied,
        std::io::ErrorKind::NotFound => Refusal::Absent,
        // Everything a socket does wrong is the remote party not answering,
        // which is what `unreachable` means. §9.6 gives `get` that one.
        _ => Refusal::Unreachable,
    }
}

/// The `Str` first argument both §9.6 functions take.
fn url_of(store: &Store, call: &Call) -> Option<String> {
    match store.get(*call.args.first()?) {
        Ok(Stored::Value(Value::Str(s))) => Some(s),
        _ => None,
    }
}

impl Provider for Net {
    fn capability(&self) -> Capability {
        if self.sending { Capability::NetWrite } else { Capability::Net }
    }

    fn answers(&self, function: &str) -> bool {
        match function {
            "get" => true,
            "post" => self.sending,
            _ => false,
        }
    }

    fn answer(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, Refuse> {
        let stratum = if call.function == "post" { Depth::NET_WRITE } else { Depth::NET };
        // §6.3: a world-call whose argument is not finished cannot have become
        // a hole, and §04 types this one as a `Str`.
        let Some(url) = url_of(into.store(), call) else {
            return into.record(call, stratum, span, &refused(Refusal::Malformed));
        };
        let said = match split(&url) {
            Err(no) => refused(no),
            // §8.3.2: a host nobody named is `denied`. Before the socket, not
            // after: a reach that is checked by whether the connection failed
            // is not a reach.
            Ok(at) if !self.reaches(&at.host, at.port) => refused(Refusal::Denied),
            Ok(at) => Self::speak(&call.function, &at, call, into),
        };
        into.record(call, stratum, span, &said)
    }
}

impl Net {
    /// One request, one response, and what it said.
    fn speak(function: &str, at: &Asked, call: &Call, into: &Recorder) -> Value {
        let body = if function == "post" {
            match call.args.get(1).map(|c| into.store().get(*c)) {
                Some(Ok(Stored::Value(Value::Bytes(b)))) => b,
                _ => return refused(Refusal::Malformed),
            }
        } else {
            Vec::new()
        };
        match fetch(function, at, &body) {
            Ok(bytes) => given(function, Value::Bytes(bytes)),
            Err(no) => refused(no),
        }
    }
}

/// HTTP/1.1, by hand, over one connection that is closed when it is done.
///
/// No keep-alive, no redirects, no chunked encoding on the way out. A redirect
/// followed silently is a trace whose witness records a URL the program never
/// asked for, and `Location` is in the response for a program that wants to
/// ask again.
fn fetch(function: &str, at: &Asked, body: &[u8]) -> Result<Vec<u8>, Refusal> {
    let verb = if function == "post" { "POST" } else { "GET" };
    let mut request = format!(
        "{verb} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nAccept: */*\r\n",
        at.path, at.host
    );
    if verb == "POST" {
        let _ = write!(request, "Content-Length: {}\r\n", body.len());
    }
    request.push_str("\r\n");
    // Headers and body in one write. Two writes is two packets, and a reader
    // that took the first for the whole request would be right about the
    // bytes and wrong about the request.
    let mut sending = request.into_bytes();
    sending.extend_from_slice(body);

    let mut socket = TcpStream::connect((at.host.as_str(), at.port)).map_err(|e| why(&e))?;
    socket.set_read_timeout(Some(PATIENCE)).map_err(|e| why(&e))?;
    socket.set_write_timeout(Some(PATIENCE)).map_err(|e| why(&e))?;
    socket.write_all(&sending).map_err(|e| why(&e))?;
    socket.flush().map_err(|e| why(&e))?;

    let mut reading = BufReader::new(socket);
    let status = status_of(&mut reading)?;
    // The body is what the program asked for; the status is how the world said
    // no. §9.6 gives `get` absent and `post` conflict, and both are answers.
    let mut said = Vec::new();
    reading.take(MOST + 1).read_to_end(&mut said).map_err(|e| why(&e))?;
    if said.len() as u64 > MOST {
        return Err(Refusal::Exhausted);
    }
    match status {
        200..=299 => Ok(said),
        404 | 410 => Err(Refusal::Absent),
        401 | 403 => Err(Refusal::Denied),
        409 | 412 => Err(Refusal::Conflict),
        _ => Err(Refusal::Unreachable),
    }
}

/// The status line and the headers, discarded down to the number.
///
/// The headers are not recorded. A witness records what the program was told
/// (§1.1), the program is told the body, and a `Date` header would make two
/// identical fetches two different witnesses.
fn status_of(reading: &mut BufReader<TcpStream>) -> Result<u16, Refusal> {
    let mut line = String::new();
    reading.read_line(&mut line).map_err(|e| why(&e))?;
    let code =
        line.split_whitespace().nth(1).and_then(|c| c.parse().ok()).ok_or(Refusal::Unreachable)?;
    loop {
        let mut header = String::new();
        let read = reading.read_line(&mut header).map_err(|e| why(&e))?;
        if read == 0 || header.trim_end().is_empty() {
            return Ok(code);
        }
    }
}
