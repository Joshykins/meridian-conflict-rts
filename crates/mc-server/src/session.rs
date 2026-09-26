//! A directory connection: sign-in, then requests until the player leaves.
//!
//! The connection's own thread reads; a writer thread drains its outbox, so the
//! housekeeping thread can push the game list to everyone without waiting on a
//! slow reader.

use std::net::{IpAddr, Shutdown, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use mc_net::directory::{
    encode_dir_frame, random_bytes, read_dir_frame, verify_proof, DirHello, NONCE_LEN, TICKET_LEN,
};
use mc_net::{check_name, fingerprint, DirMessage, DirRefuseReason, NetError};

use crate::door::{linger, ConnSlot};
use crate::{rooms, Session, Shared, Ticket};

pub(crate) type SessionId = u64;

/// Bytes a session may have waiting to be sent before it is dropped as too slow.
/// A full game list is well under a megabyte.
const MAX_OUTBOX_BYTES: usize = 8 << 20;

enum Out {
    Frame(Arc<[u8]>),
    /// Flush what is queued, then half-close so the client reads everything.
    Close,
}

/// The sending side of a session.
pub(crate) struct Outbox {
    tx: Sender<Out>,
    queued: Arc<AtomicUsize>,
    stream: TcpStream,
}

impl Outbox {
    fn start(stream: &TcpStream) -> std::io::Result<Outbox> {
        let (tx, rx) = mpsc::channel();
        let queued = Arc::new(AtomicUsize::new(0));
        let (write_half, q) = (stream.try_clone()?, queued.clone());
        thread::Builder::new()
            .name("mc-server-dir-tx".into())
            .spawn(move || writer(write_half, rx, &q))?;
        Ok(Outbox {
            tx,
            queued,
            stream: stream.try_clone()?,
        })
    }

    pub(crate) fn send(&self, msg: &DirMessage) {
        match encode_dir_frame(msg) {
            Ok(frame) => self.send_frame(&frame.into()),
            // Server-built messages keep within the limits: this is a bug, not bad input.
            Err(e) => log::error!("could not encode a directory message: {e}"),
        }
    }

    pub(crate) fn send_frame(&self, frame: &Arc<[u8]>) {
        let queued = self.queued.fetch_add(frame.len(), Ordering::Relaxed) + frame.len();
        if queued > MAX_OUTBOX_BYTES || self.tx.send(Out::Frame(frame.clone())).is_err() {
            // Wakes the session's reader, which ends the session.
            let _ = self.stream.shutdown(Shutdown::Both);
        }
    }

    /// Sends what is queued, then closes.
    pub(crate) fn close(&self) {
        let _ = self.tx.send(Out::Close);
    }
}

fn writer(mut stream: TcpStream, rx: Receiver<Out>, queued: &AtomicUsize) {
    use std::io::Write;
    let mut graceful = false;
    for out in rx {
        match out {
            Out::Frame(frame) => {
                if stream.write_all(&frame).is_err() {
                    break;
                }
                queued.fetch_sub(frame.len(), Ordering::Relaxed);
            }
            Out::Close => {
                graceful = true;
                break;
            }
        }
    }
    let _ = stream.shutdown(if graceful {
        Shutdown::Write
    } else {
        Shutdown::Both
    });
}

/// Serves one directory connection from `DirHello` to the end. `_slot` counts
/// the connection until it closes.
pub(crate) fn run(
    shared: &Arc<Shared>,
    stream: TcpStream,
    hello: DirHello,
    ip: IpAddr,
    _slot: ConnSlot,
) {
    let Ok(out) = Outbox::start(&stream) else {
        return;
    };
    let print = fingerprint(&hello.public_key);
    let refuse = |out: &Outbox, reason: DirRefuseReason, detail: &str| {
        log::info!(
            "{ip}: sign-in as {:?} [{print}] refused: {}",
            hello.name,
            reason.describe()
        );
        out.send(&DirMessage::Refused {
            reason,
            detail: detail.to_owned(),
        });
        // The writer half-closes once the refusal is out.
        out.close();
        linger(&stream);
    };
    if let Err(why) = check_name(&hello.name) {
        return refuse(&out, DirRefuseReason::BadName, why);
    }
    let Ok(nonce) = random_bytes::<NONCE_LEN>() else {
        return refuse(&out, DirRefuseReason::Other, "the server has no randomness");
    };
    out.send(&DirMessage::Challenge { nonce });
    let signature = match read_dir_frame(&mut &stream) {
        Ok(DirMessage::Proof { signature }) => signature,
        _ => {
            log::debug!("{ip}: no answer to the sign-in challenge");
            return;
        }
    };
    if !verify_proof(&hello.public_key, &nonce, &signature) {
        return refuse(&out, DirRefuseReason::BadSignature, "");
    }
    let id = match sign_in(shared, &hello, out) {
        Ok(id) => id,
        Err((out, reason, detail)) => return refuse(&out, reason, &detail),
    };
    log::info!("{ip}: {} signed in [{print}]", hello.name);
    let _ = stream.set_read_timeout(Some(shared.config.idle_timeout));
    let end = serve(shared, id, &stream);

    let mut state = shared.state();
    if let Some(session) = state.sessions.remove(&id) {
        session.out.close();
    }
    let expires = Instant::now() + shared.config.ticket_grace;
    for t in state.tickets.values_mut().filter(|t| t.session == Some(id)) {
        t.session = None;
        t.expires = Some(expires);
    }
    drop(state);
    match end {
        Ok(()) => log::info!("{ip}: {} signed out", hello.name),
        Err(e) => log::info!("{ip}: {} gone: {e}", hello.name),
    }
}

type SignInError = (Outbox, DirRefuseReason, String);

/// Checks the name against the registry, issues the ticket and opens the session.
fn sign_in(shared: &Shared, hello: &DirHello, out: Outbox) -> Result<SessionId, SignInError> {
    let Ok(ticket) = random_bytes::<TICKET_LEN>() else {
        let detail = "the server has no randomness".to_owned();
        return Err((out, DirRefuseReason::Other, detail));
    };
    let mut state = shared.state();
    if let Err(reason) = state.names.sign_in(&hello.name, &hello.public_key) {
        return Err((out, reason, String::new()));
    }
    let id = state.next_session;
    state.next_session += 1;
    state.tickets.insert(
        ticket,
        Ticket {
            name: hello.name.clone(),
            owner: hello.name.to_ascii_lowercase(),
            session: Some(id),
            expires: None,
        },
    );
    out.send(&DirMessage::SignedIn {
        name: hello.name.clone(),
        ticket,
        online: state.sessions.len() as u32 + 1,
        rooms: state.rooms.len() as u32,
        motd: shared.config.motd.clone(),
    });
    state.sessions.insert(
        id,
        Session {
            name: hello.name.clone(),
            out,
            subscribed: false,
        },
    );
    Ok(id)
}

/// Answers requests until the player leaves (`Ok`) or the connection fails.
fn serve(shared: &Shared, id: SessionId, stream: &TcpStream) -> Result<(), NetError> {
    loop {
        match read_dir_frame(&mut &*stream)? {
            DirMessage::Leave => return Ok(()),
            DirMessage::Ping(n) => reply(shared, id, &DirMessage::Pong(n)),
            DirMessage::Pong(_) => {}
            DirMessage::CreateRoom(new) => rooms::create(shared, id, new),
            DirMessage::FindRoom(code) => rooms::find(shared, id, code),
            DirMessage::Subscribe(on) => rooms::subscribe(shared, id, on),
            DirMessage::Hello(_) | DirMessage::Proof { .. } => {
                return Err(NetError::Malformed("sign-in messages after signing in"))
            }
            DirMessage::Challenge { .. }
            | DirMessage::SignedIn { .. }
            | DirMessage::Refused { .. }
            | DirMessage::RoomCreated(_)
            | DirMessage::RoomRefused { .. }
            | DirMessage::RoomFound(_)
            | DirMessage::RoomNotFound(_)
            | DirMessage::Rooms(_)
            | DirMessage::Stats { .. } => {
                return Err(NetError::Malformed("server-only message from a client"))
            }
        }
    }
}

fn reply(shared: &Shared, id: SessionId, msg: &DirMessage) {
    if let Some(session) = shared.state().sessions.get(&id) {
        session.out.send(msg);
    }
}
