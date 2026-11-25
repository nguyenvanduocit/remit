//! The relay of a TCP port to the jail's unix socket.

use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream, UnixStream};
use tokio::sync::Semaphore;

/// Bounds the connections the relay carries at once: a program that opens thousands must not exhaust the descriptors of
/// the process that relays, which the program's own supervision depends on. The gate on the other end has its own cap.
const MAX_RELAYED: usize = 128;

/// Copies every connection `l` accepts to a new connection on the unix socket, until the task is dropped or `l` fails.
pub(crate) async fn serve_relay(l: TcpListener, socket: String) {
    let slots = Arc::new(Semaphore::new(MAX_RELAYED));
    let socket: Arc<str> = socket.into();
    while let Ok((c, _)) = l.accept().await {
        // A connection beyond the cap is turned away: dropping it closes it.
        let Ok(slot) = Arc::clone(&slots).try_acquire_owned() else {
            continue;
        };
        let socket = Arc::clone(&socket);
        tokio::spawn(async move {
            relay_to(c, &socket).await;
            drop(slot);
        });
    }
}

/// Carries one connection to a new one on the unix socket and back; a direction's end is passed on as a half-close, and
/// an error in either ends both.
async fn relay_to(mut c: TcpStream, socket: &str) {
    let Ok(mut u) = UnixStream::connect(socket).await else {
        return;
    };
    // Both ends are closed on return, whichever way it ended: there is nothing to report.
    tokio::io::copy_bidirectional(&mut c, &mut u).await.ok();
}
