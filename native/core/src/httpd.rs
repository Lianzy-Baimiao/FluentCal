//! A small HTTP/1.1 server for what this program offers to its surroundings: the web view
//! (web.rs) and the handoff to a phone (pairing.rs) in the local network, assistants
//! (mcp/http.rs) on this PC. No TLS and no streams: a request is read whole, within limits,
//! and gets one answer that says how long it is.
//!
//! It is written here because of what the library used before (tiny_http 0.12) did with a
//! request that promises more than it sends: it set aside as much memory as was promised, so
//! "Content-Length: 1099511627776" from anybody who could reach the port ended the program.
//! It also had no time limits (a silent connection kept a thread for good) and kept five
//! threads waiting while nobody asked anything.
//!
//! No server: nothing here runs. A server nobody talks to: one thread waiting in accept().
//! Each connection has a thread of its own for as long as it lasts.

use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

/// The head of a request (request line and headers). Browsers send the cookies of every
/// program on "localhost" along, which can be some kilobytes.
const HEAD_MAX: usize = 64 * 1024;
const HEADERS_MAX: usize = 100;
/// How often a head is looked at before it is complete (a request comes in a piece or two).
const HEAD_TRIES: u32 = 32;
/// A response waits this long for the other side to take more of it.
const WRITE_TIME: Duration = Duration::from_secs(30);
/// After a refusal, what the other side is still sending is taken for this long and up to
/// this much, so that it gets to read why (closing on unread data resets the connection).
const LINGER_TIME: Duration = Duration::from_secs(2);
const LINGER_MAX: usize = 4 << 20;
/// A body up to this size is sent in one piece with its head.
const ONE_PIECE: usize = 16 * 1024;

pub struct Request {
    /// As sent: "GET", "POST"…
    pub method: String,
    /// Path and query, as sent.
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub peer: SocketAddr,
}

impl Request {
    /// A header by its name, whatever its case (the first one when it was sent twice).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

pub struct Response {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
    /// Called when the response has been handed to the network in full (dropped uncalled
    /// when it could not be).
    pub sent: Option<Box<dyn FnOnce() + Send>>,
}

impl Response {
    pub fn new(status: u16) -> Response {
        Response { status, headers: Vec::new(), body: Vec::new(), sent: None }
    }
}

/// How much a server takes.
#[derive(Clone, Debug)]
pub struct Limits {
    /// The largest request body; a longer one is refused (413) without being read.
    pub body: usize,
    /// Connections at the same time; one more is told to come back later (503).
    pub connections: usize,
    /// The head of a request has to be there this long after its first byte, and the first
    /// byte of a new connection this long after it was opened.
    pub head: Duration,
    /// The body has to follow within this.
    pub read: Duration,
    /// A kept connection may be silent this long between two requests.
    pub idle: Duration,
}

impl Limits {
    pub fn new(body: usize, connections: usize) -> Limits {
        Limits { body, connections, head: Duration::from_secs(10), read: Duration::from_secs(30), idle: Duration::from_secs(60) }
    }
}

/// Why a server did not start.
#[derive(Debug)]
pub enum ListenError {
    /// Another program has the port.
    InUse,
    Other(String),
}

struct Shared {
    stop: AtomicBool,
    limits: Limits,
    /// The connections that are open (a second handle of each), to end them with the server.
    open: Mutex<Vec<(u64, TcpStream)>>,
}

fn open(shared: &Shared) -> MutexGuard<'_, Vec<(u64, TcpStream)>> {
    shared.open.lock().unwrap_or_else(|e| e.into_inner())
}

/// The running server; dropping it stops it and frees the port.
pub struct Running {
    shared: Arc<Shared>,
    /// Closed by the accept thread when it has let go of the port.
    ended: Receiver<()>,
    pub addr: SocketAddr,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        // The accept thread waits for a connection, so it gets one. (Nobody can connect to
        // 0.0.0.0 on Windows: this PC's own address then.)
        let wake = if self.addr.ip().is_unspecified() { SocketAddr::from(([127, 0, 0, 1], self.addr.port())) } else { self.addr };
        let _ = TcpStream::connect_timeout(&wake, Duration::from_millis(300));
        // Not waited for without end: should the wake-up not have arrived, the thread ends
        // with the next connection instead.
        let _ = self.ended.recv_timeout(Duration::from_secs(2));
        // Connections end here, whatever they were waiting for; their threads follow.
        for (_, conn) in open(&self.shared).drain(..) {
            let _ = conn.shutdown(Shutdown::Both);
        }
    }
}

/// Takes a connection off the list when its thread ends, however it ends.
struct Closed {
    shared: Arc<Shared>,
    id: u64,
}

impl Drop for Closed {
    fn drop(&mut self) {
        open(&self.shared).retain(|(id, _)| *id != self.id);
    }
}

type Serve = dyn Fn(&Request) -> Response + Send + Sync;

/// Starts a server on `bind` ("127.0.0.1:18766"; port 0: any free one). `serve` answers each
/// request, on the thread of its connection ("<name>-conn"); several may ask at once.
pub fn listen(bind: &str, name: &str, limits: Limits, serve: impl Fn(&Request) -> Response + Send + Sync + 'static) -> Result<Running, ListenError> {
    let listener = TcpListener::bind(bind).map_err(|e| match e.kind() {
        io::ErrorKind::AddrInUse => ListenError::InUse,
        _ => ListenError::Other(e.to_string()),
    })?;
    let addr = listener.local_addr().map_err(|e| ListenError::Other(e.to_string()))?;
    let shared = Arc::new(Shared { stop: AtomicBool::new(false), limits, open: Mutex::new(Vec::new()) });
    let serve: Arc<Serve> = Arc::new(serve);
    let (end, ended) = channel::<()>();
    let (state, name) = (shared.clone(), name.to_owned());
    std::thread::Builder::new()
        .name(format!("{name}-accept"))
        .spawn(move || {
            let mut id = 0u64;
            for stream in listener.incoming() {
                if state.stop.load(Ordering::Relaxed) {
                    break;
                }
                // (a connection given up before it was taken is an error here)
                let Ok(stream) = stream else {
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                };
                let (Ok(peer), Ok(twin)) = (stream.peer_addr(), stream.try_clone()) else { continue };
                {
                    let mut all = open(&state);
                    if all.len() >= state.limits.connections {
                        drop(all);
                        busy(stream);
                        continue;
                    }
                    id += 1;
                    all.push((id, twin));
                }
                let (shared, serve) = (state.clone(), serve.clone());
                let spawned = std::thread::Builder::new().name(format!("{name}-conn")).spawn(move || {
                    let _closed = Closed { shared: shared.clone(), id };
                    connection(&shared, stream, peer, serve.as_ref());
                });
                if spawned.is_err() {
                    open(&state).retain(|(i, _)| *i != id);
                }
            }
            drop(listener); // the port is free…
            drop(end); // …and whoever stopped the server is told
        })
        .map_err(|e| ListenError::Other(e.to_string()))?;
    Ok(Running { shared, ended, addr })
}

/// More connections than the server takes: come back later.
fn busy(mut stream: TcpStream) {
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
    let _ = stream.write_all(b"HTTP/1.1 503 Service Unavailable\r\nRetry-After: 1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
}

struct Conn {
    stream: TcpStream,
    /// Received and not used yet.
    buf: Vec<u8>,
}

impl Conn {
    /// Waits for more until `deadline`; None when the other side is gone or stays silent.
    fn more(&mut self, deadline: Instant) -> Option<()> {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return None;
        }
        self.stream.set_read_timeout(Some(left)).ok()?;
        let mut piece = [0u8; 8192];
        match self.stream.read(&mut piece) {
            Ok(0) | Err(_) => None,
            Ok(n) => {
                self.buf.extend_from_slice(&piece[..n]);
                Some(())
            }
        }
    }

    /// Waits until `buf` holds `n` bytes.
    fn fill(&mut self, n: usize, deadline: Instant) -> Option<()> {
        while self.buf.len() < n {
            self.more(deadline)?;
        }
        Some(())
    }

    /// Takes the next line out of `buf`, without its line break; None also when it is longer
    /// than `max`.
    fn line(&mut self, max: usize, deadline: Instant) -> Option<Vec<u8>> {
        loop {
            if let Some(end) = self.buf.iter().position(|b| *b == b'\n') {
                if end > max {
                    return None;
                }
                let mut line: Vec<u8> = self.buf.drain(..=end).collect();
                line.pop();
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                return Some(line);
            }
            if self.buf.len() > max {
                return None;
            }
            self.more(deadline)?;
        }
    }
}

struct Head {
    len: usize,
    method: String,
    url: String,
    /// HTTP/1.1, not 1.0: the connection stays unless one side says otherwise.
    modern: bool,
    headers: Vec<(String, String)>,
}

/// The head of the request at the start of `buf`: Ok(None) while it is not all there, Err
/// with the status to refuse it with.
fn head(buf: &[u8]) -> Result<Option<Head>, u16> {
    let mut slots = [httparse::EMPTY_HEADER; HEADERS_MAX];
    let mut parsed = httparse::Request::new(&mut slots);
    match parsed.parse(buf) {
        Ok(httparse::Status::Complete(len)) if len > HEAD_MAX => Err(431),
        Ok(httparse::Status::Complete(len)) => Ok(Some(Head {
            len,
            method: parsed.method.unwrap_or("").to_owned(),
            url: parsed.path.unwrap_or("").to_owned(),
            modern: parsed.version == Some(1),
            headers: parsed.headers.iter().map(|h| (h.name.to_owned(), String::from_utf8_lossy(h.value).trim().to_owned())).collect(),
        })),
        Ok(httparse::Status::Partial) if buf.len() < HEAD_MAX => Ok(None),
        Ok(httparse::Status::Partial) | Err(httparse::Error::TooManyHeaders) => Err(431),
        Err(_) => Err(400),
    }
}

/// A body sent in chunks, put together. Err(Some(status)): refused; Err(None): it did not
/// arrive.
fn chunks(conn: &mut Conn, max: usize, deadline: Instant) -> Result<Vec<u8>, Option<u16>> {
    let mut body = Vec::new();
    loop {
        // "1a2b" or "1a2b;name=value"
        let line = conn.line(1024, deadline).ok_or(None)?;
        let digits = std::str::from_utf8(&line).ok().map(|l| l.split(';').next().unwrap_or("").trim()).unwrap_or("");
        if digits.is_empty() || digits.len() > 8 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Some(400));
        }
        let size = usize::from_str_radix(digits, 16).map_err(|_| Some(400))?;
        if size == 0 {
            // what follows the last chunk ("trailers") is of no use here, up to the empty line
            for _ in 0..HEADERS_MAX {
                if conn.line(8192, deadline).ok_or(None)?.is_empty() {
                    return Ok(body);
                }
            }
            return Err(Some(400));
        }
        if body.len() + size > max {
            return Err(Some(413));
        }
        conn.fill(size + 2, deadline).ok_or(None)?;
        if &conn.buf[size..size + 2] != b"\r\n" {
            return Err(Some(400));
        }
        body.extend(conn.buf.drain(..size + 2).take(size));
    }
}

enum Next {
    /// A request, and whether the connection stays after its answer.
    Request(Request, bool),
    /// Not a request this server takes: the status that says so, then the connection ends.
    Refuse(u16),
    /// Nothing (more) came.
    Close,
}

/// Reads the next request of a connection; `wait`: how long its first byte may take.
fn next(conn: &mut Conn, peer: SocketAddr, limits: &Limits, wait: Duration) -> Next {
    let mut deadline = Instant::now() + if conn.buf.is_empty() { wait } else { limits.head };
    // A head is looked at when it may be complete (an empty line has come), not with every
    // piece that arrives: sent a byte at a time, it would be read again 65536 times.
    let (mut scanned, mut tries) = (0usize, 0u32);
    let head = loop {
        let begun = !conn.buf.is_empty();
        if begun {
            let from = scanned.saturating_sub(2);
            let ended = conn.buf[from..].windows(2).any(|w| w == b"\n\n") || conn.buf[from..].windows(3).any(|w| w == b"\n\r\n");
            scanned = conn.buf.len();
            // (the first look also tells at once what is not HTTP at all)
            if ended || tries == 0 {
                tries += 1;
                match head(&conn.buf) {
                    Ok(Some(head)) => break head,
                    Ok(None) if tries < HEAD_TRIES => {}
                    Ok(None) => return Next::Refuse(400),
                    Err(status) => return Next::Refuse(status),
                }
            } else if conn.buf.len() >= HEAD_MAX {
                return Next::Refuse(431);
            }
        }
        if conn.more(deadline).is_none() {
            return Next::Close;
        }
        if !begun {
            deadline = Instant::now() + limits.head;
        }
    };
    conn.buf.drain(..head.len);
    let values = |name: &'static str| head.headers.iter().filter(move |(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str());
    let says = |name: &'static str, word: &str| values(name).any(|v| v.split(',').any(|w| w.trim().eq_ignore_ascii_case(word)));

    // How long the body is: said as a number, or chunk by chunk.
    let coding: Vec<&str> = values("Transfer-Encoding").collect();
    let lengths: Vec<&str> = values("Content-Length").collect();
    let chunked = match (coding.as_slice(), lengths.as_slice()) {
        ([], [] | [_]) => false,
        ([c], []) if c.eq_ignore_ascii_case("chunked") => true,
        // two lengths, a length and chunks, or a coding unknown here: where the body ends
        // would be a guess
        _ => return Next::Refuse(400),
    };
    let length = match lengths.first() {
        None => 0,
        Some(l) if !l.is_empty() && l.bytes().all(|b| b.is_ascii_digit()) => match l.parse::<u64>() {
            Ok(n) if n <= limits.body as u64 => n as usize,
            // larger than is taken (or than a number can say): refused before any of it is
            // read, and no memory is set aside for it
            _ => return Next::Refuse(413),
        },
        Some(_) => return Next::Refuse(400),
    };
    // A client that asks first whether its body is welcome (curl does, above 1 KB).
    let here = !chunked && conn.buf.len() >= length;
    if head.modern && !here && says("Expect", "100-continue") && conn.stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").is_err() {
        return Next::Close;
    }
    let deadline = Instant::now() + limits.read;
    let body = if chunked {
        match chunks(conn, limits.body, deadline) {
            Ok(body) => body,
            Err(Some(status)) => return Next::Refuse(status),
            Err(None) => return Next::Close,
        }
    } else {
        if conn.fill(length, deadline).is_none() {
            return Next::Close;
        }
        conn.buf.drain(..length).collect()
    };
    let keep = head.modern && !says("Connection", "close");
    Next::Request(Request { method: head.method, url: head.url, headers: head.headers, body, peer }, keep)
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        204 => "No Content",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        410 => "Gone",
        413 => "Content Too Large",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        100..=199 => "Continue",
        200..=299 => "OK",
        300..=399 => "Redirect",
        400..=499 => "Bad Request",
        _ => "Server Error",
    }
}

/// Sends a response. How long it is is always said (Edge left every other request of a kept
/// connection hanging on answers that were sent in chunks instead), and the body is left out
/// where there is none to send: for HEAD and for "nothing new" (304).
fn send(stream: &mut TcpStream, res: &Response, head_only: bool, close: bool) -> io::Result<()> {
    let no_length = matches!(res.status, 100..=199 | 204);
    let body: &[u8] = if no_length || head_only || res.status == 304 { &[] } else { &res.body };
    let mut out = Vec::with_capacity(512 + if body.len() <= ONE_PIECE { body.len() } else { 0 });
    write!(out, "HTTP/1.1 {} {}\r\nDate: {}\r\n", res.status, reason(res.status), httpdate::fmt_http_date(SystemTime::now()))?;
    for (name, value) in &res.headers {
        // these are set here; and a value with a line break in it would be two headers
        let ours = ["Content-Length", "Connection", "Transfer-Encoding", "Date"].iter().any(|h| name.eq_ignore_ascii_case(h));
        if !ours && !value.bytes().any(|b| matches!(b, b'\r' | b'\n' | 0)) {
            write!(out, "{name}: {value}\r\n")?;
        }
    }
    if !no_length {
        write!(out, "Content-Length: {}\r\n", res.body.len())?;
    }
    out.extend_from_slice(if close { &b"Connection: close\r\n\r\n"[..] } else { &b"\r\n"[..] });
    if body.len() <= ONE_PIECE {
        out.extend_from_slice(body);
        stream.write_all(&out)?;
    } else {
        stream.write_all(&out)?;
        stream.write_all(body)?;
    }
    stream.flush()
}

/// Ends a connection whose request was not read to its end: says that nothing more will be
/// sent, then takes what still arrives for a moment.
fn linger(stream: &mut TcpStream) {
    let _ = stream.shutdown(Shutdown::Write);
    let deadline = Instant::now() + LINGER_TIME;
    let (mut seen, mut piece) = (0usize, [0u8; 8192]);
    while seen < LINGER_MAX {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() || stream.set_read_timeout(Some(left)).is_err() {
            break;
        }
        match stream.read(&mut piece) {
            Ok(0) | Err(_) => break,
            Ok(n) => seen += n,
        }
    }
}

/// One connection, request after request, until either side has enough.
fn connection(shared: &Shared, stream: TcpStream, peer: SocketAddr, serve: &Serve) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_write_timeout(Some(WRITE_TIME));
    let mut conn = Conn { stream, buf: Vec::new() };
    let mut wait = shared.limits.head;
    while !shared.stop.load(Ordering::Relaxed) {
        match next(&mut conn, peer, &shared.limits, wait) {
            Next::Close => return,
            Next::Refuse(status) => {
                let mut res = Response::new(status);
                res.headers.push(("Content-Type", "text/plain; charset=utf-8".into()));
                res.body = format!("{}\n", reason(status)).into_bytes();
                if send(&mut conn.stream, &res, false, true).is_ok() {
                    linger(&mut conn.stream);
                }
                return;
            }
            Next::Request(req, keep) => {
                // A slip in answering one request ends that request, not the program.
                let mut res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| serve(&req))).unwrap_or_else(|_| Response::new(500));
                let keep = keep && !shared.stop.load(Ordering::Relaxed);
                if send(&mut conn.stream, &res, req.method == "HEAD", !keep).is_err() {
                    return;
                }
                if let Some(sent) = res.sent.take() {
                    sent();
                }
                if !keep {
                    return;
                }
            }
        }
        wait = shared.limits.idle;
    }
}
