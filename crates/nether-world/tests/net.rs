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

struct Asked {
    store: Store,
    world: World,
}

impl Asked {
    fn reaching(what: &str, reach: Vec<String>, sending: bool) -> Self {
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let root = std::env::temp_dir().join(format!("nether-net-{what}-{at}"));
        std::fs::create_dir_all(&root).expect("a directory");
        let store = Store::open(root.join("ledger")).expect("a store");
        let net: Box<dyn Provider> =
            if sending { Box::new(Net::sending(reach)) } else { Box::new(Net::fetching(reach)) };
        Self { store, world: World::sealed().granting(net) }
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
    assert!(a.world.holds(nether_core::Capability::Net), "§1.1 is a total order");
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
