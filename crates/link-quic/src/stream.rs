//! One stream's task: it opens the stream when this side opened it, passes on every chunk read, and writes, finishes or
//! resets as the session task asks, saying how many bytes each write flushed so the conn can tell a peer that falls
//! behind. It ends once neither side has more to say on the stream.

use link::StreamId;
use tokio::sync::mpsc;
use web_transport_quinn::{RecvStream, SendStream, Session};

use crate::streams::Input;

/// Large enough for a frame's head and a good part of its body in one read.
const CHUNK: usize = 64 * 1024;

/// The stream reset code: the link's frames carry why; the code only says the stream was given up.
const RESET: u32 = 0;

pub(crate) enum Write {
    Bytes(Vec<u8>),
    Finish,
    Reset,
}

pub(crate) enum Ends {
    OpenBi(Session),
    OpenUni(Session),
    Both(SendStream, RecvStream),
    Recv(RecvStream),
}

pub(crate) async fn run(
    id: StreamId,
    ends: Ends,
    writes: mpsc::UnboundedReceiver<Write>,
    inputs: mpsc::UnboundedSender<Input>,
) {
    let (send, recv) = match ends {
        Ends::OpenBi(session) => match session.open_bi().await {
            Ok((send, recv)) => (Some(send), Some(recv)),
            // The session is gone: its closing reaches the conn by the session's own watcher.
            Err(_) => return,
        },
        Ends::OpenUni(session) => match session.open_uni().await {
            Ok(send) => (Some(send), None),
            Err(_) => return,
        },
        Ends::Both(send, recv) => (Some(send), Some(recv)),
        Ends::Recv(recv) => (None, Some(recv)),
    };
    Stream {
        id,
        send,
        recv,
        writes,
        inputs,
    }
    .run()
    .await;
}

struct Stream {
    id: StreamId,
    send: Option<SendStream>,
    recv: Option<RecvStream>,
    writes: mpsc::UnboundedReceiver<Write>,
    inputs: mpsc::UnboundedSender<Input>,
}

impl Stream {
    async fn run(mut self) {
        while self.send.is_some() || self.recv.is_some() {
            let reading = self.recv.is_some();
            tokio::select! {
                write = self.writes.recv() => match write {
                    Some(write) => {
                        if !self.write(write).await {
                            break;
                        }
                    }
                    // The session ended.
                    None => break,
                },
                chunk = read(self.recv.as_mut()), if reading => match chunk {
                    Some(bytes) => {
                        if self.inputs.send(Input::Read(self.id, bytes)).is_err() {
                            break;
                        }
                    }
                    // Finished or reset by the peer: nothing more comes; writing may go on.
                    None => self.recv = None,
                },
            }
        }
        // The session task forgets the stream; a session already ended has nothing to forget.
        self.inputs.send(Input::Ended(self.id)).ok();
    }

    /// Whether the stream is still worth keeping.
    async fn write(&mut self, write: Write) -> bool {
        match write {
            Write::Bytes(bytes) => {
                let Some(send) = self.send.as_mut() else {
                    return true;
                };
                if send.write_all(&bytes).await.is_err() {
                    return false;
                }
                self.inputs.send(Input::Written(self.id, bytes.len())).is_ok()
            }
            Write::Finish => {
                // A finished stream still sends what was written; this side writes nothing more.
                self.send.take().is_some_and(|mut send| send.finish().is_ok())
            }
            Write::Reset => {
                // Both ends are given up; either may have closed already, which is what was wanted.
                if let Some(send) = self.send.as_mut() {
                    send.reset(RESET).ok();
                }
                if let Some(recv) = self.recv.as_mut() {
                    recv.stop(RESET).ok();
                }
                false
            }
        }
    }
}

/// The next chunk the peer sent, none once it finished or reset the stream. Cancel-safe, as quinn's `read_chunk` is.
async fn read(recv: Option<&mut RecvStream>) -> Option<bytes::Bytes> {
    let recv = recv?;
    match recv.read_chunk(CHUNK, true).await {
        Ok(Some(chunk)) => Some(chunk.bytes),
        Ok(None) | Err(_) => None,
    }
}
