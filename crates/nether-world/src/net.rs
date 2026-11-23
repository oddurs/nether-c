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
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

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

/// How much of one may be the status line and the headers.
///
/// The body has had a bound since this was written and nothing above it did,
/// so a server that sent one line and never a newline grew a `String` until
/// the timeout. Sixty-four kilobytes is more than any real response uses.
const MOST_HEAD: u64 = 64 * 1024;

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

    /// Sending only: `post`, at depth 6.
    ///
    /// §9.1 / §8.2: each capability grants its own stratum and no other;
    /// granting `net!` does not grant stratum-5 `net` fetches.
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
            Some((h, p)) => h.eq_ignore_ascii_case(host) && p.parse() == Ok(port),
            None => named.eq_ignore_ascii_case(host),
        })
    }
}

/// A URL, as far as §9.6 needs one.
struct Asked {
    host: String,
    port: u16,
    path: String,
}

/// Whether every byte of this is one a URL may contain.
///
/// Everything from `!` to `~`, and nothing else. It is deliberately narrower
/// than the grammar of a URL: a space ends the request line early and a
/// carriage return starts a header, so a program that puts either in a URL is
/// writing HTTP rather than asking for a resource, and §5.1.1 has a word for
/// something that is not the shape it claims to be. 0180.
fn printable(text: &str) -> bool {
    text.chars().all(|c| ('\u{21}'..='\u{7e}').contains(&c))
}

/// Whether that is a host and not something wearing one.
///
/// Letters, digits, `-` and `.`. No `@`, because `a@b` is a host with another
/// host in front of it and only one of the two is the party being reached;
/// no `_`, no `%`, nothing that has to be decoded before it means anything.
fn hostlike(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}

/// Split a URL, or say which no it is.
///
/// Hand-written, like everything else here. What §9.6 needs is the scheme, the
/// authority and the rest — a general URL parser would be a larger thing that
/// this has no use for.
fn split(url: &str) -> Result<Asked, Refusal> {
    if !printable(url) {
        return Err(Refusal::Malformed);
    }
    let url = url.split_once('#').map_or(url, |(before, _)| before);
    let Some((scheme, rest)) = url.split_once("://") else {
        return Err(Refusal::Malformed);
    };
    if !scheme.eq_ignore_ascii_case("http") {
        return Err(Refusal::Denied);
    }
    let (authority, path) = match rest.find(['/', '?']) {
        Some(i) if rest.as_bytes()[i] == b'/' => (&rest[..i], rest[i..].to_owned()),
        Some(i) => (&rest[..i], format!("/{}", &rest[i..])),
        None => (rest, "/".to_owned()),
    };
    if authority.is_empty() {
        return Err(Refusal::Malformed);
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => {
            if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
                return Err(Refusal::Malformed);
            }
            (h, p.parse().map_err(|_| Refusal::Malformed)?)
        }
        None => (authority, 80u16),
    };
    if port == 0 || !hostlike(host) {
        return Err(Refusal::Malformed);
    }
    Ok(Asked { host: host.to_owned(), port, path })
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
            "get" => !self.sending,
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

fn remaining(deadline: Instant) -> Result<Duration, Refusal> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or(Refusal::Unreachable)
}

fn connect_with_deadline(host: &str, port: u16, deadline: Instant) -> Result<TcpStream, Refusal> {
    let addrs = (host, port).to_socket_addrs().map_err(|e| why(&e))?;
    let mut last = Refusal::Unreachable;
    for addr in addrs {
        let left = remaining(deadline)?;
        match TcpStream::connect_timeout(&addr, left) {
            Ok(sock) => return Ok(sock),
            Err(e) => last = why(&e),
        }
    }
    Err(last)
}

fn write_all_with_deadline(
    socket: &mut TcpStream,
    mut buf: &[u8],
    deadline: Instant,
) -> Result<(), Refusal> {
    while !buf.is_empty() {
        socket.set_write_timeout(Some(remaining(deadline)?)).map_err(|e| why(&e))?;
        match socket.write(buf) {
            Ok(0) => return Err(Refusal::Unreachable),
            Ok(n) => buf = &buf[n..],
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(why(&e)),
        }
    }
    socket.flush().map_err(|e| why(&e))
}

/// HTTP/1.1, by hand, over one connection that is closed when it is done.
fn fetch(function: &str, at: &Asked, body: &[u8]) -> Result<Vec<u8>, Refusal> {
    let verb = if function == "post" { "POST" } else { "GET" };
    let host_header =
        if at.port == 80 { at.host.clone() } else { format!("{}:{}", at.host, at.port) };
    let mut request = format!(
        "{verb} {} HTTP/1.1\r\nHost: {host_header}\r\nConnection: close\r\nAccept: */*\r\n",
        at.path
    );
    if verb == "POST" {
        let _ = write!(request, "Content-Length: {}\r\n", body.len());
    }
    request.push_str("\r\n");
    let mut sending = request.into_bytes();
    sending.extend_from_slice(body);

    let deadline = Instant::now() + PATIENCE;
    let mut socket = connect_with_deadline(&at.host, at.port, deadline)?;
    write_all_with_deadline(&mut socket, &sending, deadline)?;

    let mut reading = BufReader::new(socket);
    let head = head_of(&mut reading, deadline)?;
    match head.status {
        204 => return Ok(Vec::new()),
        200..=299 => {}
        404 | 410 => return Err(Refusal::Absent),
        401 | 403 => return Err(Refusal::Denied),
        409 | 412 => return Err(Refusal::Conflict),
        _ => return Err(Refusal::Unreachable),
    }
    let said = if head.chunked {
        read_chunked(&mut reading, deadline)?
    } else {
        read_plain(&mut reading, head.content_length, deadline)?
    };
    Ok(said)
}

struct ResponseHead {
    status: u16,
    content_length: Option<u64>,
    chunked: bool,
}

fn read_line_bytes(
    reading: &mut BufReader<TcpStream>,
    budget: &mut u64,
    deadline: Instant,
) -> Result<Vec<u8>, Refusal> {
    let mut line = Vec::new();
    loop {
        if *budget == 0 {
            return Err(Refusal::Exhausted);
        }
        reading.get_ref().set_read_timeout(Some(remaining(deadline)?)).map_err(|e| why(&e))?;
        let avail = match reading.fill_buf() {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(why(&e)),
        };
        if avail.is_empty() {
            return if line.is_empty() { Err(Refusal::Unreachable) } else { Ok(line) };
        }
        let max_take = usize::try_from(*budget).unwrap_or(usize::MAX).min(avail.len());
        let slice = &avail[..max_take];
        if let Some(pos) = slice.iter().position(|&b| b == b'\n') {
            let take = pos + 1;
            line.extend_from_slice(&slice[..take]);
            reading.consume(take);
            *budget -= take as u64;
            return Ok(line);
        }
        line.extend_from_slice(slice);
        reading.consume(max_take);
        *budget -= max_take as u64;
    }
}

fn is_http_tchar(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

/// Parse HTTP/1.x status line and framing headers on raw bytes, skipping `1xx`.
fn head_of(reading: &mut BufReader<TcpStream>, deadline: Instant) -> Result<ResponseHead, Refusal> {
    let mut budget = MOST_HEAD;
    loop {
        let status_line = read_line_bytes(reading, &mut budget, deadline)?;
        let raw_line = core::str::from_utf8(&status_line).map_err(|_| Refusal::Unreachable)?;
        let line = raw_line
            .strip_suffix("\r\n")
            .or_else(|| raw_line.strip_suffix('\n'))
            .unwrap_or(raw_line);
        let (version, after_ver) = line.split_once(' ').ok_or(Refusal::Unreachable)?;
        if version != "HTTP/1.0" && version != "HTTP/1.1" {
            return Err(Refusal::Unreachable);
        }
        let status_tok = after_ver.split_once(' ').map_or(after_ver, |(code, _)| code);
        if status_tok.len() != 3 || !status_tok.bytes().all(|b| b.is_ascii_digit()) {
            return Err(Refusal::Unreachable);
        }
        let status: u16 = status_tok.parse().map_err(|_| Refusal::Unreachable)?;
        if !(100..=599).contains(&status) {
            return Err(Refusal::Unreachable);
        }
        let mut content_length: Option<u64> = None;
        let mut chunked = false;
        loop {
            let hdr = read_line_bytes(reading, &mut budget, deadline)?;
            if hdr == b"\r\n" || hdr == b"\n" {
                break;
            }
            let Some(colon) = hdr.iter().position(|&b| b == b':') else {
                return Err(Refusal::Unreachable);
            };
            let name = &hdr[..colon];
            if name.is_empty() || !name.iter().copied().all(is_http_tchar) {
                return Err(Refusal::Unreachable);
            }
            if name.eq_ignore_ascii_case(b"content-length") {
                let value = core::str::from_utf8(&hdr[colon + 1..])
                    .map_err(|_| Refusal::Unreachable)?
                    .trim();
                if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(Refusal::Unreachable);
                }
                let parsed: u64 = value.parse().map_err(|_| Refusal::Unreachable)?;
                if let Some(prev) = content_length
                    && prev != parsed
                {
                    return Err(Refusal::Unreachable);
                }
                content_length = Some(parsed);
            } else if name.eq_ignore_ascii_case(b"transfer-encoding") {
                let value =
                    core::str::from_utf8(&hdr[colon + 1..]).map_err(|_| Refusal::Unreachable)?;
                let codings: Vec<&str> =
                    value.split(',').map(str::trim).filter(|tok| !tok.is_empty()).collect();
                if chunked || codings.len() != 1 || !codings[0].eq_ignore_ascii_case("chunked") {
                    return Err(Refusal::Unreachable);
                }
                chunked = true;
            }
        }
        if chunked && content_length.is_some() {
            return Err(Refusal::Unreachable);
        }
        if (100..=199).contains(&status) {
            continue;
        }
        return Ok(ResponseHead { status, content_length, chunked });
    }
}

fn read_plain(
    reading: &mut BufReader<TcpStream>,
    expected: Option<u64>,
    deadline: Instant,
) -> Result<Vec<u8>, Refusal> {
    if expected.is_some_and(|n| n > MOST) {
        return Err(Refusal::Exhausted);
    }
    let mut said = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        if let Some(len) = expected
            && said.len() as u64 == len
        {
            break;
        }
        reading.get_ref().set_read_timeout(Some(remaining(deadline)?)).map_err(|e| why(&e))?;
        let want = match expected {
            Some(len) => {
                usize::try_from(len - said.len() as u64).unwrap_or(buf.len()).min(buf.len())
            }
            None => buf.len(),
        };
        let n = match reading.read(&mut buf[..want]) {
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(why(&e)),
        };
        if n == 0 {
            break;
        }
        if (said.len() + n) as u64 > MOST {
            return Err(Refusal::Exhausted);
        }
        said.extend_from_slice(&buf[..n]);
    }
    if let Some(len) = expected
        && (said.len() as u64) != len
    {
        return Err(Refusal::Unreachable);
    }
    Ok(said)
}

fn read_chunked(reading: &mut BufReader<TcpStream>, deadline: Instant) -> Result<Vec<u8>, Refusal> {
    let mut said = Vec::new();
    let mut framing_budget = MOST_HEAD;
    loop {
        let size_line = read_line_bytes(reading, &mut framing_budget, deadline)?;
        let text = core::str::from_utf8(&size_line).map_err(|_| Refusal::Unreachable)?;
        let hex = text.split(';').next().unwrap_or("").trim();
        if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Refusal::Unreachable);
        }
        let size = u64::from_str_radix(hex, 16).map_err(|_| Refusal::Unreachable)?;
        if size == 0 {
            loop {
                let trailer = read_line_bytes(reading, &mut framing_budget, deadline)?;
                if trailer == b"\r\n" || trailer == b"\n" {
                    return Ok(said);
                }
            }
        }
        if (said.len() as u64).saturating_add(size) > MOST {
            return Err(Refusal::Exhausted);
        }
        let mut left = usize::try_from(size).map_err(|_| Refusal::Exhausted)?;
        let mut buf = [0u8; 8192];
        while left > 0 {
            reading.get_ref().set_read_timeout(Some(remaining(deadline)?)).map_err(|e| why(&e))?;
            let want = left.min(buf.len());
            let n = match reading.read(&mut buf[..want]) {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(why(&e)),
            };
            if n == 0 {
                return Err(Refusal::Unreachable);
            }
            said.extend_from_slice(&buf[..n]);
            left -= n;
        }
        let crlf = read_line_bytes(reading, &mut framing_budget, deadline)?;
        if crlf != b"\r\n" && crlf != b"\n" {
            return Err(Refusal::Unreachable);
        }
    }
}
