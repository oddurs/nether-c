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
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use nether_core::{Capability, Depth};
use nether_ledger::{Call, Refusal, Span, Value};

use crate::provider::{Provider, Refuse, given, refused};
use crate::recorder::{Recorded, Recorder};

/// Maximum concurrent DNS lookup threads so timed-out `getaddrinfo` calls
/// cannot exhaust OS threads or panic on spawn failure.
const MAX_DNS_THREADS: usize = 16;

static ACTIVE_DNS_THREADS: AtomicUsize = AtomicUsize::new(0);

struct DnsThreadGuard;

impl Drop for DnsThreadGuard {
    fn drop(&mut self) {
        ACTIVE_DNS_THREADS.fetch_sub(1, Ordering::AcqRel);
    }
}

/// How long a socket may take to say anything. §6.4's argument, one layer out:
/// a burial with no bound on it is a burial that does not finish.
const PATIENCE: Duration = Duration::from_secs(30);

/// How much of a response is read. A body larger than this is `exhausted`,
/// which is a refusal the program can see rather than a machine that stops.
const MOST: u64 = 64 * 1024 * 1024;

/// How much of one may be the status line, headers, or chunked trailers.
const MOST_HEAD: u64 = 64 * 1024;

/// Maximum length of a single chunk-size or chunk-terminator line.
const MAX_CHUNK_LINE: u64 = 8 * 1024;

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
    /// Supports hostnames, IPv4, bare IPv6 (`::1`), and RFC 3986 bracketed
    /// IPv6 (`[::1]` or `[::1]:8080`). A bare host matches any port; host
    /// comparison is exact so subdomains never inherit a parent's reach.
    fn reaches(&self, host: &str, port: u16) -> bool {
        let host_v6 = host.parse::<Ipv6Addr>().ok();
        self.reach.iter().any(|named| {
            let Some((entry_host, entry_port)) = parse_reach_entry(named) else {
                return false;
            };
            if entry_port.is_some_and(|p| p != port) {
                return false;
            }
            if let (Some(a), Ok(b)) = (host_v6, entry_host.parse::<Ipv6Addr>()) {
                return a == b;
            }
            entry_host.eq_ignore_ascii_case(host)
        })
    }
}

fn parse_reach_entry(named: &str) -> Option<(&str, Option<u16>)> {
    if let Some(rest) = named.strip_prefix('[') {
        let (inner, after) = rest.split_once(']')?;
        if after.is_empty() {
            Some((inner, None))
        } else {
            Some((inner, Some(after.strip_prefix(':')?.parse::<u16>().ok()?)))
        }
    } else if named.parse::<Ipv6Addr>().is_ok() {
        Some((named, None))
    } else if let Some((h, p)) = named.rsplit_once(':') {
        Some((h, Some(p.parse::<u16>().ok()?)))
    } else {
        Some((named, None))
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

/// RFC 3986 §3.1: `scheme = ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )`.
fn valid_scheme(scheme: &str) -> bool {
    let mut bytes = scheme.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_alphabetic())
        && bytes.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
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

fn parse_port(p: &str) -> Result<u16, Refusal> {
    if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Refusal::Malformed);
    }
    let port: u16 = p.parse().map_err(|_| Refusal::Malformed)?;
    if port == 0 { Err(Refusal::Malformed) } else { Ok(port) }
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
    if !valid_scheme(scheme) {
        return Err(Refusal::Malformed);
    }
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
    let (host, port) = if let Some(after_open) = authority.strip_prefix('[') {
        let Some((inner, after_close)) = after_open.split_once(']') else {
            return Err(Refusal::Malformed);
        };
        inner.parse::<Ipv6Addr>().map_err(|_| Refusal::Malformed)?;
        let port = if after_close.is_empty() {
            80u16
        } else if let Some(p) = after_close.strip_prefix(':') {
            parse_port(p)?
        } else {
            return Err(Refusal::Malformed);
        };
        (inner, port)
    } else {
        let (h, p) = match authority.rsplit_once(':') {
            Some((h, p)) => (h, parse_port(p)?),
            None => (authority, 80u16),
        };
        if !hostlike(h) {
            return Err(Refusal::Malformed);
        }
        (h, p)
    };
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
        let func = call.function();
        let stratum = if func == "post" { Depth::NET_WRITE } else { Depth::NET };
        // §6.3: a world-call whose argument is not finished cannot have become
        // a hole, and §04 types this one as a `Str`.
        let Some(url) = into.arg_str(call, 0)? else {
            return into.record(call, stratum, span, &refused(Refusal::Malformed));
        };
        let said = match split(&url) {
            Err(no) => refused(no),
            // §8.3.2: a host nobody named is `denied`. Before the socket, not
            // after: a reach that is checked by whether the connection failed
            // is not a reach.
            Ok(at) if !self.reaches(&at.host, at.port) => refused(Refusal::Denied),
            Ok(at) => Self::speak(func, &at, call, into)?,
        };
        into.record(call, stratum, span, &said)
    }
}

impl Net {
    /// One request, one response, and what it said.
    fn speak(function: &str, at: &Asked, call: &Call, into: &Recorder) -> Result<Value, Refuse> {
        let body = if function == "post" {
            let Some(b) = into.arg_bytes(call, 1)? else {
                return Ok(refused(Refusal::Malformed));
            };
            b
        } else {
            Vec::new()
        };
        Ok(match fetch(function, at, &body) {
            Ok(bytes) => given(function, Value::Bytes(bytes)),
            Err(no) => refused(no),
        })
    }
}

fn remaining(deadline: Instant) -> Result<Duration, Refusal> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or(Refusal::Unreachable)
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_multicast()
        || ip.is_broadcast()
        || a == 0
        // Carrier-grade NAT (RFC 6598): 100.64.0.0/10
        || (a == 100 && (64..=127).contains(&b))
        // IETF protocol assignments (RFC 6890): 192.0.0.0/24
        || (a == 192 && b == 0 && c == 0)
        // TEST-NET-1/2/3 (RFC 5737)
        || (a == 192 && b == 0 && c == 2)
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        // Benchmarking (RFC 2544): 198.18.0.0/15
        || (a == 198 && (b == 18 || b == 19))
        // Reserved (RFC 1112): 240.0.0.0/4
        || a >= 240)
}

/// Whether `ip` is globally routable (rejecting loopback, private/ULA,
/// link-local, unspecified, multicast, documentation, and IPv4-mapped forms).
fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public_v4(v4);
            }
            let segs = v6.segments();
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                // Link-local unicast: fe80::/10
                || (segs[0] & 0xffc0) == 0xfe80
                // Unique local (ULA): fc00::/7
                || (segs[0] & 0xfe00) == 0xfc00
                // Site-local (deprecated): fec0::/10
                || (segs[0] & 0xffc0) == 0xfec0
                // Documentation: 2001:db8::/32
                || (segs[0] == 0x2001 && segs[1] == 0x0db8)
                // Discard-only: 100::/64
                || (segs[0] == 0x0100 && segs[1..4] == [0; 3])
                // Well-known NAT64 prefix: 64:ff9b::/96
                || (segs[0] == 0x0064 && segs[1] == 0xff9b && segs[2..6] == [0; 4])
                // IPv4-compatible (deprecated): ::/96
                || segs[..6] == [0; 6])
        }
    }
}

fn allows_nonpublic_resolution(host: &str) -> bool {
    host.parse::<IpAddr>().is_ok()
        || host.eq_ignore_ascii_case("localhost")
        || host.eq_ignore_ascii_case("localhost.")
}

fn resolve_with_deadline(
    host: &str,
    port: u16,
    deadline: Instant,
) -> Result<Vec<SocketAddr>, Refusal> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(ip, port)]);
    }
    ACTIVE_DNS_THREADS
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            (n < MAX_DNS_THREADS).then_some(n + 1)
        })
        .map_err(|_| Refusal::Exhausted)?;
    let (tx, rx) = std::sync::mpsc::channel();
    let owned = host.to_owned();
    let spawn = std::thread::Builder::new().name("nether-dns".to_owned()).spawn(move || {
        let _guard = DnsThreadGuard;
        let res = (owned.as_str(), port).to_socket_addrs().map(Iterator::collect::<Vec<_>>);
        let _ = tx.send(res);
    });
    if spawn.is_err() {
        ACTIVE_DNS_THREADS.fetch_sub(1, Ordering::AcqRel);
        return Err(Refusal::Exhausted);
    }
    match rx.recv_timeout(remaining(deadline)?) {
        Ok(Ok(addrs)) => Ok(addrs),
        Ok(Err(e)) => Err(why(&e)),
        Err(_) => Err(Refusal::Unreachable),
    }
}

fn connect_with_deadline(host: &str, port: u16, deadline: Instant) -> Result<TcpStream, Refusal> {
    let addrs = resolve_with_deadline(host, port, deadline)?;
    if !allows_nonpublic_resolution(host) && addrs.iter().any(|addr| !is_public_ip(addr.ip())) {
        return Err(Refusal::Denied);
    }
    let mut last = Refusal::Unreachable;
    for (idx, addr) in addrs.iter().enumerate() {
        let left = remaining(deadline)?;
        let rem_addrs = u32::try_from(addrs.len() - idx).unwrap_or(1);
        let slice = (left / rem_addrs).max(Duration::from_millis(500)).min(left);
        match TcpStream::connect_timeout(addr, slice) {
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
    let bracketed = if at.host.contains(':') { format!("[{}]", at.host) } else { at.host.clone() };
    let host_header = if at.port == 80 { bracketed } else { format!("{bracketed}:{}", at.port) };
    let mut request = format!(
        "{verb} {} HTTP/1.1\r\nHost: {host_header}\r\nConnection: close\r\nAccept: */*\r\n",
        at.path
    );
    if verb == "POST" {
        let _ = write!(request, "Content-Length: {}\r\n", body.len());
    }
    request.push_str("\r\n");

    let deadline = Instant::now() + PATIENCE;
    let mut socket = connect_with_deadline(&at.host, at.port, deadline)?;
    write_all_with_deadline(&mut socket, request.as_bytes(), deadline)?;
    if !body.is_empty() {
        write_all_with_deadline(&mut socket, body, deadline)?;
    }

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

fn arm_read_timeout(reading: &BufReader<TcpStream>, deadline: Instant) -> Result<(), Refusal> {
    if reading.buffer().is_empty() {
        reading.get_ref().set_read_timeout(Some(remaining(deadline)?)).map_err(|e| why(&e))?;
    }
    Ok(())
}

fn read_some(
    reading: &mut BufReader<TcpStream>,
    buf: &mut [u8],
    deadline: Instant,
) -> Result<usize, Refusal> {
    loop {
        arm_read_timeout(reading, deadline)?;
        match reading.read(buf) {
            Ok(n) => return Ok(n),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(why(&e)),
        }
    }
}

fn is_http_eol(b: &[u8]) -> bool {
    b == b"\r\n" || b == b"\n"
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
        arm_read_timeout(reading, deadline)?;
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
    b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)
}

/// Strip one trailing `\r\n` or `\n` and reject bare `\r` inside HTTP framing.
fn strip_http_eol(raw: &str) -> Result<&str, Refusal> {
    let line =
        raw.strip_suffix("\r\n").or_else(|| raw.strip_suffix('\n')).ok_or(Refusal::Unreachable)?;
    if line.as_bytes().contains(&b'\r') {
        return Err(Refusal::Unreachable);
    }
    Ok(line)
}

/// Trim RFC 9110 optional whitespace (`SP` / `HTAB`) only.
fn trim_ows(s: &str) -> &str {
    s.trim_matches([' ', '\t'])
}

fn parse_status_line(raw: &[u8]) -> Result<u16, Refusal> {
    let raw_line = core::str::from_utf8(raw).map_err(|_| Refusal::Unreachable)?;
    let line = strip_http_eol(raw_line)?;
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
    Ok(status)
}

/// Parse HTTP/1.x status line and framing headers on raw bytes, skipping
/// interim `1xx` responses other than `101 Switching Protocols`.
fn head_of(reading: &mut BufReader<TcpStream>, deadline: Instant) -> Result<ResponseHead, Refusal> {
    let mut budget = MOST_HEAD;
    loop {
        let status_line = read_line_bytes(reading, &mut budget, deadline)?;
        let status = parse_status_line(&status_line)?;
        let mut content_length: Option<u64> = None;
        let mut chunked = false;
        loop {
            let hdr = read_line_bytes(reading, &mut budget, deadline)?;
            if is_http_eol(&hdr) {
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
                let raw =
                    core::str::from_utf8(&hdr[colon + 1..]).map_err(|_| Refusal::Unreachable)?;
                let value = trim_ows(strip_http_eol(raw)?);
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
                let raw =
                    core::str::from_utf8(&hdr[colon + 1..]).map_err(|_| Refusal::Unreachable)?;
                let value = strip_http_eol(raw)?;
                let codings: Vec<&str> =
                    value.split(',').map(trim_ows).filter(|tok| !tok.is_empty()).collect();
                if chunked || codings.len() != 1 || !codings[0].eq_ignore_ascii_case("chunked") {
                    return Err(Refusal::Unreachable);
                }
                chunked = true;
            }
        }
        if chunked && content_length.is_some() {
            return Err(Refusal::Unreachable);
        }
        if (100..=199).contains(&status) && status != 101 {
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
        let want = match expected {
            Some(len) => {
                usize::try_from(len - said.len() as u64).unwrap_or(buf.len()).min(buf.len())
            }
            None => buf.len(),
        };
        let n = read_some(reading, &mut buf[..want], deadline)?;
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
    let mut wire_framing = MOST;
    loop {
        let mut line_budget = MAX_CHUNK_LINE.min(wire_framing);
        let size_line = read_line_bytes(reading, &mut line_budget, deadline)?;
        wire_framing =
            wire_framing.checked_sub(size_line.len() as u64).ok_or(Refusal::Exhausted)?;
        let text = core::str::from_utf8(&size_line).map_err(|_| Refusal::Unreachable)?;
        let line = strip_http_eol(text)?;
        let hex = line.split(';').next().unwrap_or("").trim_end_matches([' ', '\t']);
        if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Refusal::Unreachable);
        }
        let size = u64::from_str_radix(hex, 16).map_err(|_| Refusal::Unreachable)?;
        if size == 0 {
            let mut trailer_budget = MOST_HEAD.min(wire_framing);
            loop {
                let trailer = read_line_bytes(reading, &mut trailer_budget, deadline)?;
                if is_http_eol(&trailer) {
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
            let want = left.min(buf.len());
            let n = read_some(reading, &mut buf[..want], deadline)?;
            if n == 0 {
                return Err(Refusal::Unreachable);
            }
            said.extend_from_slice(&buf[..n]);
            left -= n;
        }
        let mut crlf_budget = MAX_CHUNK_LINE.min(wire_framing);
        let crlf = read_line_bytes(reading, &mut crlf_budget, deadline)?;
        wire_framing = wire_framing.checked_sub(crlf.len() as u64).ok_or(Refusal::Exhausted)?;
        if !is_http_eol(&crlf) {
            return Err(Refusal::Unreachable);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{allows_nonpublic_resolution, is_public_ip};
    use std::net::IpAddr;

    #[test]
    fn ssrf_ip_filter_rejects_loopback_private_link_local_and_v4_mapped() {
        for blocked in [
            "127.0.0.1",
            "127.255.255.254",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "0.0.0.0",
            "100.64.0.1",
            "224.0.0.1",
            "255.255.255.255",
            "::1",
            "::",
            "::ffff:127.0.0.1",
            "::ffff:169.254.169.254",
            "::ffff:10.0.0.1",
            "fe80::1",
            "fc00::1",
            "fd12:3456::1",
            "ff02::1",
            "2001:db8::1",
            "64:ff9b::1",
        ] {
            let ip: IpAddr = blocked.parse().unwrap();
            assert!(!is_public_ip(ip), "{blocked} must be rejected");
        }
        for public in ["93.184.216.34", "1.1.1.1", "2606:2800:220:1:248:1893:25c8:1946"] {
            let ip: IpAddr = public.parse().unwrap();
            assert!(is_public_ip(ip), "{public} must be allowed");
        }
        assert!(allows_nonpublic_resolution("127.0.0.1"));
        assert!(allows_nonpublic_resolution("::1"));
        assert!(allows_nonpublic_resolution("localhost"));
        assert!(!allows_nonpublic_resolution("partner.example"));
    }
}
