//! Test-only PostgreSQL frame relay; trusted disposable loopback databases only.
use sqlx::postgres::{PgConnectOptions, PgSslMode};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Notify,
    task::JoinHandle,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fault {
    Pass,
    DropCommitRequest,
    DropCommitResponse,
    GateCommitResponse,
}
#[derive(Default)]
pub(crate) struct Counters {
    pub(crate) commands: AtomicUsize,
    pub(crate) parses: AtomicUsize,
    pub(crate) exchanges: AtomicUsize,
    pub(crate) bind_bytes: AtomicUsize,
    pub(crate) max_bind_bytes: AtomicUsize,
}
/// One consistent reading of the relay counters.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Sample {
    pub(crate) commands: usize,
    pub(crate) parses: usize,
    pub(crate) exchanges: usize,
    pub(crate) bind_bytes: usize,
    pub(crate) max_bind_bytes: usize,
}
impl Counters {
    pub(crate) fn sample(&self) -> Sample {
        Sample {
            commands: self.commands.load(Ordering::Relaxed),
            parses: self.parses.load(Ordering::Relaxed),
            exchanges: self.exchanges.load(Ordering::Relaxed),
            bind_bytes: self.bind_bytes.load(Ordering::Relaxed),
            max_bind_bytes: self.max_bind_bytes.load(Ordering::Relaxed),
        }
    }
}
struct State {
    fault: Fault,
    armed: AtomicBool,
    reached: Notify,
    release: Notify,
    delivered: Notify,
    counters: Counters,
}
pub(crate) struct Relay {
    pub(crate) options: PgConnectOptions,
    state: Arc<State>,
    task: JoinHandle<()>,
}
impl Relay {
    pub(crate) async fn start(options: &PgConnectOptions, fault: Fault) -> Self {
        assert!(
            matches!(options.get_host(), "localhost" | "127.0.0.1" | "::1"),
            "relay requires loopback PostgreSQL"
        );
        let backend = (options.get_host().to_owned(), options.get_port());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let state = Arc::new(State {
            fault,
            armed: AtomicBool::new(false),
            reached: Notify::new(),
            release: Notify::new(),
            delivered: Notify::new(),
            counters: Counters::default(),
        });
        let shared = Arc::clone(&state);
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted=listener.accept()=>{
                        let (client,_)=accepted.unwrap();let backend=backend.clone();let state=Arc::clone(&shared);
                        connections.spawn(async move {let _=forward(client,backend,state).await;});
                    }
                    _=connections.join_next(), if !connections.is_empty()=>{}
                }
            }
        });
        Self {
            options: options
                .clone()
                .host("127.0.0.1")
                .port(port)
                .ssl_mode(PgSslMode::Disable),
            state,
            task,
        }
    }
    pub(crate) fn arm(&self) {
        self.state.armed.store(true, Ordering::SeqCst);
    }
    pub(crate) async fn reached(&self) {
        self.state.reached.notified().await;
    }
    pub(crate) fn release(&self) {
        self.state.release.notify_one();
    }
    pub(crate) async fn delivered(&self) {
        self.state.delivered.notified().await;
    }
    pub(crate) fn counters(&self) -> &Counters {
        &self.state.counters
    }
}
impl Drop for Relay {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn frame(reader: &mut (impl AsyncRead + Unpin)) -> io::Result<Vec<u8>> {
    let tag = reader.read_u8().await?;
    let length = reader.read_u32().await? as usize;
    if !(4..=16 * 1024 * 1024).contains(&length) {
        return Err(io::Error::other("invalid test protocol frame length"));
    }
    let mut bytes = vec![0; length + 1];
    bytes[0] = tag;
    bytes[1..5].copy_from_slice(&(length as u32).to_be_bytes());
    reader.read_exact(&mut bytes[5..]).await?;
    Ok(bytes)
}
async fn forward(
    mut client: TcpStream,
    backend: (String, u16),
    state: Arc<State>,
) -> io::Result<()> {
    let mut server = TcpStream::connect(backend).await?;
    client.set_nodelay(true)?;
    server.set_nodelay(true)?;
    let size = client.read_u32().await? as usize;
    if !(8..=65536).contains(&size) {
        return Err(io::Error::other("invalid startup packet"));
    }
    let mut startup = vec![0; size];
    startup[..4].copy_from_slice(&(size as u32).to_be_bytes());
    client.read_exact(&mut startup[4..]).await?;
    server.write_all(&startup).await?;
    let (mut cr, mut cw) = client.into_split();
    let (mut sr, mut sw) = server.into_split();
    let committing = AtomicBool::new(false);
    tokio::select! {
        result=upload(&mut cr,&mut sw,&state,&committing)=>result,
        result=download(&mut sr,&mut cw,&state,&committing)=>result,
    }
}
async fn upload(
    reader: &mut (impl AsyncRead + Unpin),
    writer: &mut (impl AsyncWrite + Unpin),
    state: &State,
    committing: &AtomicBool,
) -> io::Result<()> {
    loop {
        let bytes = frame(reader).await?;
        let c = &state.counters;
        match bytes[0] {
            b'Q' => {
                c.commands.fetch_add(1, Ordering::Relaxed);
                c.exchanges.fetch_add(1, Ordering::Relaxed);
            }
            b'E' => {
                c.commands.fetch_add(1, Ordering::Relaxed);
            }
            b'P' => {
                c.parses.fetch_add(1, Ordering::Relaxed);
            }
            b'S' => {
                c.exchanges.fetch_add(1, Ordering::Relaxed);
            }
            b'B' => {
                c.bind_bytes.fetch_add(bytes.len(), Ordering::Relaxed);
                c.max_bind_bytes.fetch_max(bytes.len(), Ordering::Relaxed);
            }
            _ => {}
        }
        if bytes[0] == b'Q'
            && bytes[5..].eq_ignore_ascii_case(b"COMMIT\0")
            && state.armed.swap(false, Ordering::SeqCst)
        {
            if state.fault == Fault::DropCommitRequest {
                state.reached.notify_one();
                return Ok(());
            }
            committing.store(true, Ordering::SeqCst);
        }
        writer.write_all(&bytes).await?;
    }
}
async fn download(
    reader: &mut (impl AsyncRead + Unpin),
    writer: &mut (impl AsyncWrite + Unpin),
    state: &State,
    committing: &AtomicBool,
) -> io::Result<()> {
    loop {
        let mut bytes = frame(reader).await?;
        if bytes[0] == b'C'
            && bytes[5..] == *b"COMMIT\0"
            && committing.swap(false, Ordering::SeqCst)
        {
            let ready = frame(reader).await?;
            assert_eq!(ready[0], b'Z');
            bytes.extend(ready);
            state.reached.notify_one();
            match state.fault {
                Fault::DropCommitResponse => return Ok(()),
                Fault::GateCommitResponse => state.release.notified().await,
                _ => {}
            }
            writer.write_all(&bytes).await?;
            state.delivered.notify_one();
        } else {
            writer.write_all(&bytes).await?;
        }
    }
}
