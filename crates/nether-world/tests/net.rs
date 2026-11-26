//! The proof for *the world: strata 5 and 6*.
//!
//! `spec/09-prelude.md` §9.6, and §8.3.2 for how far it reaches. The recording
//! discipline is `discipline.rs`; this is about what the provider says, and
//! about the one thing it must not do.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use nether_ledger::{AnswerOf, Cairn, Call, Node, Refusal, Span, Store, Stored, Value};
use nether_world::{Net, Provider, Recorder, World};

/// A server that answers one request and stops.
///
/// Hand-written, like the client. What it has to be is a socket on a port
/// nothing else holds, which is what port 0 is for.
struct Once {
    port: u16,
    thread: Option<std::thread::JoinHandle<Vec<u8>>>,
}

impl Once {
    fn saying(status: &'static str, body: &'static str) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("a port");
        let port = listener.local_addr().expect("an address").port();
        let thread = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().expect("one connection");
            // Until the whole request is here, not until the first packet is.
            // A `read` that returns the headers and not the body is a correct
            // `read` and a wrong test.
            let mut heard = Vec::new();
            let mut chunk = [0u8; 4096];
            while !whole(&heard) {
                match socket.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => heard.extend_from_slice(&chunk[..n]),
                }
            }
            let said = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(said.as_bytes());
            heard
        });
        Self { port, thread: Some(thread) }
    }

    /// What the server was sent. Joins, so the request is complete.
    fn heard(mut self) -> String {
        let heard = self.thread.take().expect("a thread").join().expect("it finished");
        String::from_utf8_lossy(&heard).into_owned()
    }
}

/// Whether those bytes are a whole HTTP request: the headers, and as much
/// body as `Content-Length` promised.
fn whole(heard: &[u8]) -> bool {
    let Ok(text) = core::str::from_utf8(heard) else { return false };
    let Some((headers, body)) = text.split_once("\r\n\r\n") else { return false };
    let promised: usize = headers
        .lines()
        .find_map(|l| l.strip_prefix("Content-Length: "))
        .and_then(|n| n.trim().parse().ok())
        .unwrap_or(0);
    body.len() >= promised
}

struct Scratch(std::path::PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Asked {
    store: Store,
    world: World,
    _dir: Scratch,
}

impl Asked {
    fn reaching(what: &str, reach: Vec<String>, sending: bool) -> Self {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "nether-net-{what}-{}-{}",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).expect("a directory");
        let store = Store::open(root.join("ledger")).expect("a store");
        let net: Box<dyn Provider> =
            if sending { Box::new(Net::sending(reach)) } else { Box::new(Net::fetching(reach)) };
        Self { store, world: World::sealed().granting(net), _dir: Scratch(root) }
    }

    fn arg(&self, v: Value) -> Cairn {
        self.store.put(&Stored::Value(v)).expect("put")
    }

    fn ask(&self, function: &str, args: Vec<Cairn>) -> AnswerOf {
        let call = Call { function: function.to_owned(), args };
        let span = Span { source: self.arg(Value::Bytes(b"x.nc".to_vec())), start: 0, end: 1 };
        let recorded = self.world.ask(&call, span, &Recorder::new(&self.store)).expect("granted");

        // §1.4, and §9.6's "sealed on arrival": the witness names the answer,
        // and both were written before this returned.
        let Ok(Stored::Node(Node::Witness { answer, .. })) = self.store.get(recorded.witness())
        else {
            panic!("no witness")
        };
        assert_eq!(answer, recorded.answer(), "the witness names something else");
        match self.store.get(recorded.answer()) {
            Ok(Stored::Value(Value::Answer(a))) => *a,
            other => panic!("not an answer: {other:?}"),
        }
    }

    fn get(&self, url: &str) -> AnswerOf {
        self.ask("get", vec![self.arg(Value::Str(url.into()))])
    }
}

#[test]
fn a_fetch_is_sealed_before_the_program_is_told() {
    let server = Once::saying("200 OK", "the body");
    let a = Asked::reaching("fetch", vec![format!("127.0.0.1:{}", server.port)], false);
    let said = a.get(&format!("http://127.0.0.1:{}/thing", server.port));
    assert_eq!(said, AnswerOf::Given(Value::Bytes(b"the body".to_vec())));

    let heard = server.heard();
    assert!(heard.starts_with("GET /thing HTTP/1.1\r\n"), "{heard}");
    assert!(heard.contains("Host: 127.0.0.1"), "{heard}");
    assert!(heard.contains("Connection: close"), "no keep-alive: {heard}");
}

#[test]
fn a_post_carries_its_body_and_is_stratum_six() {
    let server = Once::saying("200 OK", "ok");
    let port = server.port;
    let a = Asked::reaching("post", vec![format!("127.0.0.1:{port}")], true);
    let said = a.ask(
        "post",
        vec![
            a.arg(Value::Str(format!("http://127.0.0.1:{port}/in"))),
            a.arg(Value::Bytes(b"what was sent".to_vec())),
        ],
    );
    assert_eq!(said, AnswerOf::Given(Value::Bytes(b"ok".to_vec())));

    let heard = server.heard();
    assert!(heard.starts_with("POST /in HTTP/1.1\r\n"), "{heard}");
    assert!(heard.contains("Content-Length: 13"), "{heard}");
    assert!(heard.ends_with("what was sent"), "{heard}");

    assert_eq!(a.world.depth(), nether_core::Depth::NET_WRITE);
    assert!(a.world.holds(nether_core::Capability::NetWrite));
    assert!(!a.world.holds(nether_core::Capability::Net), "a grant of net! is a grant of net");
}

#[test]
fn fetching_alone_will_not_send() {
    // §1.8: sending is deeper, because the world remembers what was said.
    let a = Asked::reaching("no-post", vec!["example.invalid".into()], false);
    let call = Call { function: "post".into(), args: Vec::new() };
    let span = Span { source: a.arg(Value::Bytes(b"x".to_vec())), start: 0, end: 1 };
    let no = a.world.ask(&call, span, &Recorder::new(&a.store)).expect_err("not granted");
    assert!(no.to_string().contains("net!"), "{no}");
}

#[test]
fn a_host_nobody_reached_is_denied_before_the_socket() {
    // §8.3.2. Before the socket, not after: a reach checked by whether the
    // connection failed is not a reach. The server is up and is not asked.
    let server = Once::saying("200 OK", "should not be read");
    let a = Asked::reaching("unreached", vec!["somewhere.else".into()], false);
    assert_eq!(
        a.get(&format!("http://127.0.0.1:{}/thing", server.port)),
        AnswerOf::Refused(Refusal::Denied)
    );
    // And the reach is exact: a subdomain is a different party.
    let b = Asked::reaching("subdomain", vec!["example.com".into()], false);
    assert_eq!(b.get("http://sub.example.com/"), AnswerOf::Refused(Refusal::Denied));

    // Nothing connected, so the server is still waiting. Let it go.
    let _ = TcpStream::connect(("127.0.0.1", server.port));
    let _ = server.heard();
}

#[test]
fn https_is_denied_by_name_and_not_reported_as_unreachable() {
    // §9.6: `denied` is this build saying no and `unreachable` is the world
    // not answering, and a trace that recorded the second for the first would
    // say the host was down when nothing ever dialled it.
    let a = Asked::reaching("tls", vec!["example.com".into()], false);
    assert_eq!(a.get("https://example.com/"), AnswerOf::Refused(Refusal::Denied));
    assert_eq!(a.get("ftp://example.com/"), AnswerOf::Refused(Refusal::Denied));
}

#[test]
fn a_status_the_world_said_no_with_is_the_refusal_it_means() {
    for (status, expected) in
        [("404 Not Found", Refusal::Absent), ("403 Forbidden", Refusal::Denied)]
    {
        let server = Once::saying(status, "");
        let a = Asked::reaching("status", vec![format!("127.0.0.1:{}", server.port)], false);
        let said = a.get(&format!("http://127.0.0.1:{}/gone", server.port));
        assert_eq!(said, AnswerOf::Refused(expected), "{status}");
        let _ = server.heard();
    }
}

#[test]
fn a_program_cannot_write_headers_of_its_own() {
    // 0180. §8.3.2's reach decides which socket opens; it does not decide what
    // goes down it, and §3.6 gives a string literal `\r` and `\n`. The server
    // is up and declared, and is still never asked.
    let server = Once::saying("200 OK", "should not be read");
    let host = format!("127.0.0.1:{}", server.port);
    let a = Asked::reaching("smuggle", vec![host.clone()], false);

    for attempt in [
        format!("http://{host}/x\r\nHost: internal-admin\r\n"),
        format!("http://{host}/x HTTP/1.1\r\n\r\nGET /admin HTTP/1.1\r\n"),
        format!("http://{host}/x\nX-Evil: 1"),
        format!("http://{host}/a b"),
    ] {
        assert_eq!(
            a.get(&attempt),
            AnswerOf::Refused(Refusal::Malformed),
            "this was sent: {attempt:?}"
        );
    }

    // A host wearing another host is not the host that was reached.
    let b = Asked::reaching("userinfo", vec!["example.com".into()], false);
    assert_eq!(b.get("http://example.com@evil.invalid/"), AnswerOf::Refused(Refusal::Malformed));

    // Nothing connected. Let the server go.
    let _ = TcpStream::connect(("127.0.0.1", server.port));
    let _ = server.heard();
}

#[test]
fn a_response_that_never_stops_is_exhausted_rather_than_endless() {
    // The body has had a bound since this was written and nothing above it
    // did. A server that sends a status line and then headers forever used to
    // grow a String until the timeout. 0180.
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("a port");
    let port = listener.local_addr().expect("an address").port();
    std::thread::spawn(move || {
        use std::io::Write as _;
        let Ok((mut socket, _)) = listener.accept() else { return };
        let _ = socket.write_all(b"HTTP/1.1 200 OK\r\n");
        // Never a blank line, and never an end.
        while socket.write_all(b"X-Padding: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\r\n").is_ok() {}
    });

    let a = Asked::reaching("endless", vec![format!("127.0.0.1:{port}")], false);
    assert_eq!(a.get(&format!("http://127.0.0.1:{port}/")), AnswerOf::Refused(Refusal::Exhausted));
}

#[test]
fn chunked_transfer_encoding_truncated_body_and_url_shapes_work() {
    // 1. Chunked HTTP/1.1 body is decoded rather than sealed as raw wire framing.
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("port");
    let port = listener.local_addr().expect("addr").port();
    let handle = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().expect("accept");
        let mut req = [0u8; 512];
        let n = s.read(&mut req).expect("read");
        let _ = s.write_all(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n",
        );
        String::from_utf8_lossy(&req[..n]).into_owned()
    });
    let a = Asked::reaching("chunked", vec![format!("127.0.0.1:{port}")], false);
    assert_eq!(
        a.get(&format!("http://127.0.0.1:{port}/p?q=1#frag")),
        AnswerOf::Given(Value::Bytes(b"hello world".to_vec()))
    );
    let heard = handle.join().expect("join");
    assert!(heard.starts_with("GET /p?q=1 HTTP/1.1\r\n"), "fragment leaked: {heard}");
    assert!(
        heard.contains(&format!("Host: 127.0.0.1:{port}\r\n")),
        "missing port in Host: {heard}"
    );

    // 2. Truncated body (fewer bytes than Content-Length) is refused as Unreachable.
    let l2 = TcpListener::bind(("127.0.0.1", 0)).expect("port");
    let p2 = l2.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        let Ok((mut s, _)) = l2.accept() else { return };
        let mut req = [0u8; 256];
        let _ = s.read(&mut req);
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nshort");
    });
    let b = Asked::reaching("trunc", vec![format!("127.0.0.1:{p2}")], false);
    assert_eq!(b.get(&format!("http://127.0.0.1:{p2}/")), AnswerOf::Refused(Refusal::Unreachable));

    // 3. Garbage without `://` is Malformed, not Denied; port 0 is Malformed.
    assert_eq!(b.get("not-a-url"), AnswerOf::Refused(Refusal::Malformed));
    assert_eq!(b.get("http://127.0.0.1:0/"), AnswerOf::Refused(Refusal::Malformed));
}

#[test]
fn http_framing_handles_204_keepalive_conflicting_cl_and_bad_te() {
    // 1. 204 No Content and keep-alive Content-Length return without waiting for EOF.
    let l1 = TcpListener::bind(("127.0.0.1", 0)).expect("port");
    let p1 = l1.local_addr().expect("addr").port();
    let hold1 = std::thread::spawn(move || {
        let Ok((mut s, _)) = l1.accept() else { return };
        let mut req = [0u8; 256];
        let _ = s.read(&mut req);
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\npingEXTRA");
        std::thread::sleep(std::time::Duration::from_millis(500));
    });
    let client1 = Asked::reaching("keepalive-exact", vec![format!("127.0.0.1:{p1}")], false);
    assert_eq!(
        client1.get(&format!("http://127.0.0.1:{p1}/")),
        AnswerOf::Given(Value::Bytes(b"ping".to_vec()))
    );
    let _ = hold1.join();

    // 2. 204 No Content returns empty bytes immediately even when socket stays open.
    let l2 = TcpListener::bind(("127.0.0.1", 0)).expect("port");
    let p2 = l2.local_addr().expect("addr").port();
    let hold2 = std::thread::spawn(move || {
        let Ok((mut s, _)) = l2.accept() else { return };
        let mut req = [0u8; 256];
        let _ = s.read(&mut req);
        let _ = s.write_all(b"HTTP/1.1 204 No Content\r\n\r\n");
        std::thread::sleep(std::time::Duration::from_millis(500));
    });
    let client2 = Asked::reaching("no-content-204", vec![format!("127.0.0.1:{p2}")], false);
    assert_eq!(
        client2.get(&format!("http://127.0.0.1:{p2}/")),
        AnswerOf::Given(Value::Bytes(Vec::new()))
    );
    let _ = hold2.join();

    // 3. Conflicting/malformed Content-Length, unsupported or duplicate Transfer-Encoding,
    //    whitespace before header colon, and combined TE+CL are all refused.
    for raw in [
        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Length: 4\r\n\r\nabcd".as_slice(),
        b"HTTP/1.1 200 OK\r\nContent-Length: +4\r\n\r\nabcd".as_slice(),
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: notchunked\r\n\r\n0\r\n\r\n".as_slice(),
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked, gzip\r\n\r\n0\r\n\r\n".as_slice(),
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip, chunked\r\n\r\n0\r\n\r\n".as_slice(),
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding : chunked\r\n\r\n0\r\n\r\n".as_slice(),
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 0\r\n\r\n0\r\n\r\n"
            .as_slice(),
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n+1\r\na\r\n0\r\n\r\n".as_slice(),
        b"HTTP/1.99 200 OK\r\nContent-Length: 0\r\n\r\n".as_slice(),
    ] {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("port");
        let port = listener.local_addr().expect("addr").port();
        let raw = raw.to_vec();
        std::thread::spawn(move || {
            let Ok((mut sock, _)) = listener.accept() else { return };
            let mut req = [0u8; 256];
            let _ = sock.read(&mut req);
            let _ = sock.write_all(&raw);
        });
        let client = Asked::reaching("bad-framing", vec![format!("127.0.0.1:{port}")], false);
        assert_eq!(
            client.get(&format!("http://127.0.0.1:{port}/")),
            AnswerOf::Refused(Refusal::Unreachable)
        );
    }

    // 4. `Net::sending` (stratum 6) must not answer stratum-5 `get` holes, and `+80` port is Malformed.
    let sender = Asked::reaching("sending-only", vec!["127.0.0.1".into()], true);
    let get_call = Call {
        function: "get".into(),
        args: vec![sender.arg(Value::Str("http://127.0.0.1/".into()))],
    };
    let span = Span { source: sender.arg(Value::Bytes(b"x.nc".to_vec())), start: 0, end: 1 };
    assert!(matches!(
        sender.world.ask(&get_call, span, &Recorder::new(&sender.store)),
        Err(nether_world::Unanswered::Ungranted { .. })
    ));
    let fetcher = Asked::reaching("plus-port", vec!["127.0.0.1:80".into()], false);
    assert_eq!(fetcher.get("http://127.0.0.1:+80/"), AnswerOf::Refused(Refusal::Malformed));
}

#[test]
fn http_101_does_not_stall_and_strict_ows_framing_is_enforced() {
    // 1. 101 Switching Protocols returns Unreachable immediately instead of waiting for another header.
    let l1 = TcpListener::bind(("127.0.0.1", 0)).expect("port");
    let p1 = l1.local_addr().expect("addr").port();
    let hold = std::thread::spawn(move || {
        let Ok((mut s, _)) = l1.accept() else { return };
        let mut req = [0u8; 256];
        let _ = s.read(&mut req);
        let _ = s.write_all(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n\r\n");
        std::thread::sleep(std::time::Duration::from_millis(500));
    });
    let c1 = Asked::reaching("proto-101", vec![format!("127.0.0.1:{p1}")], false);
    assert_eq!(c1.get(&format!("http://127.0.0.1:{p1}/")), AnswerOf::Refused(Refusal::Unreachable));
    let _ = hold.join();

    // 2. Non-OWS whitespace (\x0b, \x0c), bare \r, and leading space in chunk size are refused.
    for raw in [
        b"HTTP/1.1 200 OK\r\nContent-Length: \x0b4\r\n\r\nabcd".as_slice(),
        b"HTTP/1.1 200 OK\r\nContent-Length: \x0c4\r\n\r\nabcd".as_slice(),
        b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\r\n\r\nabcd".as_slice(),
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: \x0bchunked\r\n\r\n0\r\n\r\n".as_slice(),
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n 1\r\na\r\n0\r\n\r\n".as_slice(),
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\r\na\r\n0\r\n\r\n".as_slice(),
    ] {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("port");
        let port = listener.local_addr().expect("addr").port();
        let raw = raw.to_vec();
        std::thread::spawn(move || {
            let Ok((mut sock, _)) = listener.accept() else { return };
            let mut req = [0u8; 256];
            let _ = sock.read(&mut req);
            let _ = sock.write_all(&raw);
        });
        let client = Asked::reaching("strict-ows", vec![format!("127.0.0.1:{port}")], false);
        assert_eq!(
            client.get(&format!("http://127.0.0.1:{port}/")),
            AnswerOf::Refused(Refusal::Unreachable)
        );
    }
}

#[test]
fn chunked_many_chunks_ipv6_urls_and_invalid_scheme_syntax() {
    // M19: syntactically invalid schemes are Malformed, not Denied.
    let client = Asked::reaching("bad-schemes", vec!["127.0.0.1".into()], false);
    assert_eq!(client.get("://127.0.0.1/"), AnswerOf::Refused(Refusal::Malformed));
    assert_eq!(client.get("1http://127.0.0.1/"), AnswerOf::Refused(Refusal::Malformed));
    assert_eq!(client.get("ht_tp://127.0.0.1/"), AnswerOf::Refused(Refusal::Malformed));

    // H17: valid multi-chunk response with >64 KiB of total chunk framing succeeds.
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("port");
    let port = listener.local_addr().expect("addr").port();
    let chunks = 12_000usize;
    let server = std::thread::spawn(move || {
        let Ok((mut sock, _)) = listener.accept() else { return };
        let mut req = [0u8; 512];
        let _ = sock.read(&mut req);
        let _ = sock.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n");
        let chunk = b"1\r\nx\r\n";
        let mut batch = Vec::with_capacity(chunk.len() * 256);
        for _ in 0..256 {
            batch.extend_from_slice(chunk);
        }
        let mut remaining = chunks;
        while remaining >= 256 {
            let _ = sock.write_all(&batch);
            remaining -= 256;
        }
        for _ in 0..remaining {
            let _ = sock.write_all(chunk);
        }
        let _ = sock.write_all(b"0\r\n\r\n");
    });
    let chunk_client = Asked::reaching("many-chunks", vec![format!("127.0.0.1:{port}")], false);
    assert_eq!(
        chunk_client.get(&format!("http://127.0.0.1:{port}/")),
        AnswerOf::Given(Value::Bytes(vec![b'x'; chunks]))
    );
    let _ = server.join();

    // H18: RFC 3986 IPv6 literal URLs and --reach entries (bracketed and bare).
    if let Ok(v6_listener) = TcpListener::bind(("::1", 0)) {
        let v6_port = v6_listener.local_addr().expect("v6 addr").port();
        let v6_srv = std::thread::spawn(move || {
            for _ in 0..2 {
                let Ok((mut sock, _)) = v6_listener.accept() else { return };
                let mut req = [0u8; 512];
                let n = sock.read(&mut req).unwrap_or(0);
                let text = String::from_utf8_lossy(&req[..n]);
                assert!(
                    text.contains(&format!("Host: [::1]:{v6_port}\r\n")),
                    "missing bracketed IPv6 Host header: {text}"
                );
                let _ = sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nv6");
            }
        });
        let bracket_client =
            Asked::reaching("ipv6-bracket", vec![format!("[::1]:{v6_port}")], false);
        assert_eq!(
            bracket_client.get(&format!("http://[::1]:{v6_port}/")),
            AnswerOf::Given(Value::Bytes(b"v6".to_vec()))
        );
        let bare_client = Asked::reaching("ipv6-bare", vec!["::1".into()], false);
        assert_eq!(
            bare_client.get(&format!("http://[::1]:{v6_port}/")),
            AnswerOf::Given(Value::Bytes(b"v6".to_vec()))
        );
        let _ = v6_srv.join();
    }
}
