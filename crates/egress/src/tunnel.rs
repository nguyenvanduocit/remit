use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use http::{Request, Response, StatusCode};
use http_body_util::{BodyExt, Empty};
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::Instant;

use crate::egress::{Body, Inner, Turn, dial, text};
use crate::policy::parse_target;

/// The bytes a tunnel or an exchange may still carry, shared by both directions.
#[derive(Debug)]
pub(crate) struct Budget {
    left: AtomicI64,
    over: AtomicBool,
}

impl Budget {
    pub(crate) fn new(n: u64) -> Budget {
        Budget {
            left: AtomicI64::new(i64::try_from(n).unwrap_or(i64::MAX)),
            over: AtomicBool::new(false),
        }
    }

    /// Spends n bytes; false, and the budget marked as overrun, when they are not there.
    pub(crate) fn spend(&self, n: u64) -> bool {
        let n = i64::try_from(n).unwrap_or(i64::MAX);
        // Saturating: a budget overrun by more than i64 holds is still overrun.
        if self.left.fetch_sub(n, Ordering::SeqCst).saturating_sub(n) < 0 {
            self.over.store(true, Ordering::SeqCst);
            return false;
        }
        true
    }

    /// Spends n bytes or none: an injector's request that does not fit leaves the budget as it was.
    pub(crate) fn take(&self, n: u64) -> bool {
        let n = i64::try_from(n).unwrap_or(i64::MAX);
        // Saturating: a budget overrun by more than i64 holds is still overrun.
        if self.left.fetch_sub(n, Ordering::SeqCst).saturating_sub(n) < 0 {
            self.left.fetch_add(n, Ordering::SeqCst);
            return false;
        }
        true
    }

    fn left(&self) -> i64 {
        self.left.load(Ordering::SeqCst)
    }

    pub(crate) fn over(&self) -> bool {
        self.over.load(Ordering::SeqCst)
    }
}

impl Inner {
    /// Serves a CONNECT: it checks where the program wants to go, connects there, and relays bytes both ways until
    /// either side ends, the deadline passes, the byte cap is reached or the Gate closes.
    pub(crate) async fn connect(
        self: Arc<Self>,
        mut req: Request<Incoming>,
        deadline: Instant,
        turn: Turn,
    ) -> Response<Body> {
        let authority = req.uri().authority().map(|a| a.to_string()).unwrap_or_default();
        let t = match parse_target(&authority) {
            Ok(t) => t,
            Err(e) => return self.refuse(&authority, StatusCode::FORBIDDEN, &format!("CONNECT target: {e}")),
        };
        let addrs = match self.route(&t).await {
            Ok(addrs) => addrs,
            Err((status, reason)) => return self.refuse(&t.to_string(), status, &reason),
        };
        let Ok(Ok(upstream)) = tokio::time::timeout_at(deadline, dial(&addrs, t.port)).await else {
            return text(
                StatusCode::BAD_GATEWAY,
                &format!("egress gate: cannot connect to {t}\n"),
            );
        };
        let upgrade = hyper::upgrade::on(&mut req);
        let gate = Arc::clone(&self);
        self.tracker.spawn(async move {
            let _turn = turn;
            let tunnel = async {
                // Bytes a client sent before it read the 200 come first out of the upgraded connection.
                let Ok(client) = upgrade.await else { return };
                let budget = Budget::new(gate.cfg.max_bytes);
                if relay(TokioIo::new(client), upstream, &budget).await {
                    gate.denied.add(&t.to_string(), "tunnel ended at the byte cap");
                }
            };
            tokio::select! {
                () = gate.token.cancelled() => {}
                _ = tokio::time::timeout_at(deadline, tunnel) => {}
            }
        });
        Response::new(Empty::new().map_err(|never| match never {}).boxed())
    }
}

/// Why one direction of a tunnel ended other than by its end of stream.
enum Cut {
    OverCap,
    Failed,
}

/// Copies between the two connections until both directions end; true when the byte cap ended it. When one direction
/// ends, the other side is told (shutdown of its write half) so a protocol that half-closes still finishes; an error or
/// the cap on one side ends both.
pub(crate) async fn relay<A, B>(a: A, b: B, budget: &Budget) -> bool
where
    A: AsyncRead + AsyncWrite,
    B: AsyncRead + AsyncWrite,
{
    let (mut ar, mut aw) = tokio::io::split(a);
    let (mut br, mut bw) = tokio::io::split(b);
    let result = tokio::try_join!(pipe(&mut ar, &mut bw, budget), pipe(&mut br, &mut aw, budget));
    matches!(result, Err(Cut::OverCap))
}

/// Copies src to dst while the budget lasts. At zero it reads one byte more to tell a stream that ended exactly at the
/// cap (fine) from one that goes on (over the cap).
async fn pipe<R, W>(src: &mut R, dst: &mut W, budget: &Budget) -> Result<(), Cut>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf = vec![0u8; 32 << 10];
    loop {
        let left = budget.left();
        let want = usize::try_from(left).map_or(1, |l| l.clamp(1, buf.len()));
        // want is clamped to buf's length and read returns at most what it was given: neither get misses.
        let chunk = buf.get_mut(..want).ok_or(Cut::Failed)?;
        let n = src.read(chunk).await.map_err(|_| Cut::Failed)?;
        if n == 0 {
            #[expect(
                clippy::let_underscore_must_use,
                reason = "this direction ended whole; a peer that cannot take the close is seen by the other direction's pipe"
            )]
            let _ = dst.shutdown().await;
            return Ok(());
        }
        if left <= 0 || !budget.spend(u64::try_from(n).unwrap_or(u64::MAX)) {
            return Err(Cut::OverCap);
        }
        let read = chunk.get(..n).ok_or(Cut::Failed)?;
        dst.write_all(read).await.map_err(|_| Cut::Failed)?;
    }
}
