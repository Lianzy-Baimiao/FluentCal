//! The HTTP server that the web view, the handoff to a phone and the MCP endpoint share
//! (httpd.rs), spoken to byte for byte on a loopback port.
use fluentcal_core::httpd::{listen, Limits, ListenError, Request, Response, Running};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Says back what was asked.
fn echo(req: &Request) -> Response {
    let mut res = Response::new(200);
    res.headers.push(("Content-Type", "application/json".into()));
    res.body = json!({ "method": req.method, "url": req.url, "body": String::from_utf8_lossy(&req.body), "agent": req.header("user-agent"), "peer": req.peer.ip().to_string() }).to_string().into_bytes();
    res
}

/// Answers of every kind, by path.
fn odd(req: &Request) -> Response {
    match req.url.as_str() {
        "/304" => {
            let mut res = Response::new(304);
            res.headers.push(("ETag", "\"x\"".into()));
            res
        }
        "/204" => Response::new(204),
        "/panic" => panic!("on purpose (a test)"),
        "/inject" => {
            let mut res = Response::new(200);
            res.headers = vec![("X-Evil", "a\r\nSet-Cookie: x=1".into()), ("X-Fine", "yes".into()), ("Content-Length", "999".into())];
            res.body = b"ok".to_vec();
            res
        }
        "/big" => {
            let mut res = Response::new(200);
            res.body = (0..300_000u32).map(|i| b'a' + (i % 26) as u8).collect();
            res
        }
        _ => echo(req),
    }
}

fn server(limits: Limits) -> Running {
    listen("127.0.0.1:0", "test", limits, odd).unwrap()
}

struct Got {
    /// 0: no response (the connection ended).
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Got {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|_| panic!("not JSON: {}", String::from_utf8_lossy(&self.body)))
    }
}

struct Client {
    r: BufReader<TcpStream>,
    w: TcpStream,
}

impl Client {
    fn new(addr: SocketAddr) -> Client {
        let conn = TcpStream::connect(addr).unwrap();
        conn.set_read_timeout(Some(Duration::from_secs(5))).unwrap(); // no answer: fail, don't hang
        Client { w: conn.try_clone().unwrap(), r: BufReader::new(conn) }
    }

    fn say(&mut self, text: &str) {
        self.w.write_all(text.as_bytes()).unwrap();
    }

    /// The next response; `body`: whether one follows its head (none does after HEAD).
    fn hear(&mut self, body: bool) -> Got {
        let mut got = Got { status: 0, headers: Vec::new(), body: Vec::new() };
        let mut status = String::new();
        if self.r.read_line(&mut status).is_err() {
            return got;
        }
        got.status = status.split(' ').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        loop {
            let mut line = String::new();
            if self.r.read_line(&mut line).is_err() {
                return got;
            }
            let Some((k, v)) = line.trim_end().split_once(':') else { break };
            got.headers.push((k.to_ascii_lowercase(), v.trim().to_owned()));
        }
        if body {
            got.body = vec![0; got.header("content-length").map_or(0, |v| v.parse().unwrap())];
            self.r.read_exact(&mut got.body).unwrap();
        }
        got
    }

    /// The server ends the connection within `within` (and sends nothing before).
    fn ended(&mut self, within: Duration) -> bool {
        self.r.get_ref().set_read_timeout(Some(within)).unwrap();
        let mut one = [0u8; 1];
        match self.r.read(&mut one) {
            Ok(0) => true,
            Ok(_) => false,
            Err(e) => !matches!(e.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock),
        }
    }
}

const GET: &str = "GET /x HTTP/1.1\r\nHost: x\r\n\r\n";

#[test]
fn one_connection_many_requests() {
    let s = server(Limits::new(1024, 8));
    let mut c = Client::new(s.addr);
    // two requests in one piece: answered one after the other
    c.say("GET /a?x=1 HTTP/1.1\r\nHost: x\r\nUser-Agent: t\r\n\r\nPOST /b HTTP/1.1\r\nHost: x\r\nContent-Length: 5\r\n\r\nhello");
    let a = c.hear(true);
    assert_eq!((a.status, a.json()["method"].as_str(), a.json()["url"].as_str(), a.json()["agent"].as_str(), a.json()["peer"].as_str()), (200, Some("GET"), Some("/a?x=1"), Some("t"), Some("127.0.0.1")));
    assert!(a.header("date").is_some_and(|d| d.ends_with("GMT")), "{:?}", a.headers);
    assert!(a.header("transfer-encoding").is_none() && a.header("connection").is_none() && a.header("content-type") == Some("application/json"));
    let b = c.hear(true);
    assert_eq!((b.json()["method"].as_str(), b.json()["body"].as_str()), (Some("POST"), Some("hello")));
    // a body that arrives in pieces
    c.say("POST /c HTTP/1.1\r\ncontent-length: 10\r\n\r\nhel");
    std::thread::sleep(Duration::from_millis(60));
    c.say("lo, you");
    assert_eq!(c.hear(true).json()["body"], "hello, you");
    // HEAD: the length of what GET would send, and nothing after it
    c.say("HEAD /a HTTP/1.1\r\n\r\n");
    let head = c.hear(false);
    assert!(head.status == 200 && head.header("content-length").unwrap().parse::<usize>().unwrap() > 10);
    c.say("GET /after HTTP/1.1\r\n\r\n");
    assert_eq!(c.hear(true).json()["url"], "/after", "nothing was left over from HEAD");
    // an empty line before a request is nothing
    c.say("\r\nGET /blank HTTP/1.1\r\n\r\n");
    assert_eq!(c.hear(true).json()["url"], "/blank");
    // "close": said back, and done
    c.say("GET /bye HTTP/1.1\r\nConnection: Keep-Alive, Close\r\n\r\n");
    let bye = c.hear(true);
    assert_eq!((bye.status, bye.header("connection")), (200, Some("close")));
    assert!(c.ended(Duration::from_secs(2)));
    // HTTP/1.0 does not stay
    let mut old = Client::new(s.addr);
    old.say("GET /old HTTP/1.0\r\n\r\n");
    let got = old.hear(true);
    assert_eq!((got.json()["url"].as_str(), got.header("connection")), (Some("/old"), Some("close")));
    assert!(old.ended(Duration::from_secs(2)));
}

#[test]
fn answers_of_every_kind() {
    let s = server(Limits::new(1024, 8));
    let mut c = Client::new(s.addr);
    // "nothing new": no body, and the length says so (what Edge got from the web view before)
    c.say("GET /304 HTTP/1.1\r\n\r\n");
    let same = c.hear(true);
    assert_eq!((same.status, same.header("content-length"), same.header("etag")), (304, Some("0"), Some("\"x\"")));
    c.say("GET /204 HTTP/1.1\r\n\r\n");
    let nothing = c.hear(false);
    assert_eq!((nothing.status, nothing.header("content-length")), (204, None));
    // a header cannot bring another one along, and the length is the server's to say
    c.say("GET /inject HTTP/1.1\r\n\r\n");
    let got = c.hear(true);
    assert_eq!((got.status, got.header("x-fine"), got.header("x-evil"), got.header("set-cookie"), got.header("content-length")), (200, Some("yes"), None, None, Some("2")));
    assert_eq!(got.body, b"ok");
    // more than goes in one piece
    c.say("GET /big HTTP/1.1\r\n\r\n");
    let big = c.hear(true);
    assert_eq!(big.body.len(), 300_000);
    assert!(big.body.iter().enumerate().all(|(i, b)| *b == b'a' + (i % 26) as u8));
    // a slip while answering: that request fails, the connection and the server go on
    c.say("GET /panic HTTP/1.1\r\n\r\n");
    assert_eq!(c.hear(true).status, 500);
    c.say(GET);
    assert_eq!(c.hear(true).status, 200);
    let mut other = Client::new(s.addr);
    other.say(GET);
    assert_eq!(other.hear(true).status, 200);
}

#[test]
fn bodies_in_chunks_and_on_request() {
    let s = server(Limits::new(1024, 8));
    let mut c = Client::new(s.addr);
    c.say("POST /chunks HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n3;note=1\r\n, y\r\n");
    std::thread::sleep(Duration::from_millis(60));
    c.say("2\r\nou\r\n0\r\nTrailer: x\r\n\r\n");
    assert_eq!(c.hear(true).json()["body"], "hello, you");
    c.say("POST /none HTTP/1.1\r\nTransfer-Encoding: Chunked\r\n\r\n0\r\n\r\n");
    assert_eq!(c.hear(true).json()["body"], "");
    // a client that asks first whether its body is welcome (curl, above 1 KB)
    c.say("POST /ask HTTP/1.1\r\nContent-Length: 5\r\nExpect: 100-continue\r\n\r\n");
    assert_eq!(c.hear(false).status, 100);
    c.say("hello");
    let got = c.hear(true);
    assert_eq!((got.status, got.json()["body"].as_str()), (200, Some("hello")));
    // …and one that sends it along with the question is not told to go on
    c.say("POST /ask HTTP/1.1\r\nContent-Length: 5\r\nExpect: 100-continue\r\n\r\nhello");
    assert_eq!(c.hear(true).status, 200);
    // the limit itself is taken
    c.say(&format!("POST /full HTTP/1.1\r\nContent-Length: 1024\r\n\r\n{}", "x".repeat(1024)));
    assert_eq!(c.hear(true).json()["body"].as_str().map(str::len), Some(1024));
}

#[test]
fn what_is_not_taken() {
    let s = server(Limits::new(1024, 8));
    // (status, told that the connection ends, and it does)
    let refused = |bytes: &[u8]| {
        let mut c = Client::new(s.addr);
        c.w.write_all(bytes).unwrap();
        let got = c.hear(true);
        (got.status, got.header("connection") == Some("close") && c.ended(Duration::from_secs(3)))
    };
    // Longer than is taken: refused on its announcement, before any of it is read. The
    // second is the request that ended the program with tiny_http (it set aside 1 TB).
    let started = Instant::now();
    assert_eq!(refused(b"POST / HTTP/1.1\r\nContent-Length: 1025\r\n\r\n"), (413, true));
    assert_eq!(refused(b"POST / HTTP/1.1\r\nContent-Length: 1099511627776\r\n\r\n{\"jsonrpc\""), (413, true));
    assert_eq!(refused(b"POST / HTTP/1.1\r\nContent-Length: 99999999999999999999999999999\r\n\r\n"), (413, true), "more than a number holds");
    assert_eq!(refused(format!("POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n401\r\n{}", "x".repeat(10)).as_bytes()), (413, true), "0x401 = 1025");
    assert_eq!(refused(b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n400\r\n").0, 0, "within the limit: waited for (here: in vain)");
    assert!(started.elapsed() < Duration::from_secs(20));
    // where the body ends must not be a guess
    assert_eq!(refused(b"POST / HTTP/1.1\r\nContent-Length: abc\r\n\r\n"), (400, true));
    assert_eq!(refused(b"POST / HTTP/1.1\r\nContent-Length: -5\r\n\r\n"), (400, true));
    assert_eq!(refused(b"POST / HTTP/1.1\r\nContent-Length: 3\r\nContent-Length: 3\r\n\r\nabc"), (400, true));
    assert_eq!(refused(b"POST / HTTP/1.1\r\nContent-Length: 3\r\nTransfer-Encoding: chunked\r\n\r\n"), (400, true));
    assert_eq!(refused(b"POST / HTTP/1.1\r\nTransfer-Encoding: gzip\r\n\r\n"), (400, true));
    assert_eq!(refused(b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\n"), (400, true));
    assert_eq!(refused(b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabcde\r\n"), (400, true), "a chunk longer than it says");
    // not HTTP/1.x
    assert_eq!(refused(b"NOT HTTP AT ALL\r\n\r\n"), (400, true));
    assert_eq!(refused(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"), (400, true));
    assert_eq!(refused(b"\x16\x03\x01\x02\x00\x01\x00\x01\xfc\x03\x03\r\n\r\n"), (400, true), "TLS on a plain port");
    // heads without end
    assert_eq!(refused(format!("GET / HTTP/1.1\r\n{}\r\n", "X-A: b\r\n".repeat(101)).as_bytes()), (431, true));
    assert_eq!(refused(format!("GET / HTTP/1.1\r\nCookie: {}\r\n\r\n", "c".repeat(70_000)).as_bytes()), (431, true));
    let mut long = Client::new(s.addr);
    long.say(&format!("GET / HTTP/1.1\r\nCookie: {}\r\n\r\n", "c".repeat(30_000)));
    assert_eq!(long.hear(true).status, 200, "what a browser with many cookies sends");
    // A head is not read anew with every byte that comes: empty lines one at a time (each
    // could be the end of a head) are given up on after a few looks.
    let mut drip = Client::new(s.addr);
    let started = Instant::now();
    for _ in 0..200 {
        if drip.w.write_all(b"\n").is_err() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let got = drip.hear(true);
    assert_eq!(got.status, 400, "{:?}", started.elapsed());

    // a server that takes no bodies at all (the handoff to a phone)
    let none = listen("127.0.0.1:0", "test", Limits::new(0, 4), echo).unwrap();
    let mut c = Client::new(none.addr);
    c.say("GET /x HTTP/1.1\r\nContent-Length: 0\r\n\r\n");
    assert_eq!(c.hear(true).status, 200);
    c.say("GET /x HTTP/1.1\r\nContent-Length: 1\r\n\r\nx");
    assert_eq!(c.hear(true).status, 413);

    // and after all of it the first server answers
    let mut c = Client::new(s.addr);
    c.say(GET);
    assert_eq!(c.hear(true).status, 200);
}

#[test]
fn nobody_waits_for_ever() {
    let mut limits = Limits::new(1024, 8);
    (limits.head, limits.read, limits.idle) = (Duration::from_millis(300), Duration::from_millis(300), Duration::from_millis(900));
    let s = server(limits);
    let between = |t: Instant, least: u64, most: u64| {
        let ms = t.elapsed().as_millis() as u64;
        assert!((least..=most).contains(&ms), "{ms} ms, expected {least} to {most}");
    };
    // connects and says nothing
    let (t, mut c) = (Instant::now(), Client::new(s.addr));
    assert!(c.ended(Duration::from_secs(5)));
    between(t, 200, 3000);
    // A head that comes drop by drop: the time runs from its first byte, not from the last
    // one, so it is over while the drops still fall.
    let mut c = Client::new(s.addr);
    c.say("GET / HTTP/1.1\r\nX-Slow: ");
    for _ in 0..12 {
        std::thread::sleep(Duration::from_millis(100));
        let _ = c.w.write_all(b"a"); // (fails once the server has gone)
    }
    assert!(c.ended(Duration::from_millis(100)), "the server gave up long ago");
    // a body that does not follow
    let (t, mut c) = (Instant::now(), Client::new(s.addr));
    c.say("POST / HTTP/1.1\r\nContent-Length: 10\r\n\r\nabc");
    assert!(c.ended(Duration::from_secs(5)));
    between(t, 200, 3000);
    // between two requests a connection may rest longer, but not for good
    let mut c = Client::new(s.addr);
    c.say(GET);
    assert_eq!(c.hear(true).status, 200);
    std::thread::sleep(Duration::from_millis(500));
    c.say(GET);
    assert_eq!(c.hear(true).status, 200, "500 ms of rest are fine");
    let t = Instant::now();
    assert!(c.ended(Duration::from_secs(5)));
    between(t, 700, 3500);
}

#[test]
fn no_more_connections_than_it_takes() {
    let s = server(Limits::new(1024, 2));
    let (mut a, mut b) = (Client::new(s.addr), Client::new(s.addr));
    for c in [&mut a, &mut b] {
        c.say(GET);
        assert_eq!(c.hear(true).status, 200);
    }
    let mut third = Client::new(s.addr);
    let busy = third.hear(true);
    assert_eq!((busy.status, busy.header("retry-after"), busy.header("connection")), (503, Some("1"), Some("close")));
    // the two that are in go on
    a.say(GET);
    assert_eq!(a.hear(true).status, 200);
    // one leaves: there is room again
    drop(a);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let mut c = Client::new(s.addr);
        let _ = c.w.write_all(GET.as_bytes());
        if c.hear(false).status == 200 {
            break;
        }
        assert!(Instant::now() < deadline, "no room after a connection ended");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn stopping_frees_the_port_and_ends_connections() {
    let s = server(Limits::new(1024, 8));
    let addr = s.addr;
    assert!(addr.ip().is_loopback() && addr.port() != 0);
    let mut resting = Client::new(addr);
    resting.say(GET);
    assert_eq!(resting.hear(true).status, 200);
    let mut silent = Client::new(addr);
    let t = Instant::now();
    drop(s);
    assert!(t.elapsed() < Duration::from_millis(1500), "stopping waits for nobody: {:?}", t.elapsed());
    assert!(resting.ended(Duration::from_secs(2)) && silent.ended(Duration::from_secs(2)), "connections end with the server");
    // the port can be used again at once, and by one server only
    let again = listen(&addr.to_string(), "test", Limits::new(1024, 8), echo).unwrap();
    assert!(matches!(listen(&addr.to_string(), "test", Limits::new(1024, 8), echo), Err(ListenError::InUse)));
    let mut c = Client::new(addr);
    c.say(GET);
    assert_eq!(c.hear(true).status, 200);
    drop(again);
    assert!(TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_err(), "nobody listens any more");
}

#[test]
fn tells_when_an_answer_has_left() {
    let count = Arc::new(AtomicUsize::new(0));
    let counted = count.clone();
    let s = listen("127.0.0.1:0", "test", Limits::new(0, 4), move |_| {
        let mut res = Response::new(200);
        let counted = counted.clone();
        res.sent = Some(Box::new(move || {
            counted.fetch_add(1, Ordering::SeqCst);
        }));
        res
    })
    .unwrap();
    let mut c = Client::new(s.addr);
    for n in 1..=3 {
        c.say(GET);
        assert_eq!(c.hear(true).status, 200);
        // on the same connection the next request is read after the last one was reported
        c.say(GET);
        assert_eq!(c.hear(true).status, 200);
        assert!(count.load(Ordering::SeqCst) >= 2 * n - 1);
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    while count.load(Ordering::SeqCst) < 6 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(count.load(Ordering::SeqCst), 6);
}
