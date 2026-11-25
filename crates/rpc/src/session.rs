//! One admitted session on `/link`, whatever channel it is: its calls answered, each on a task of its own, and its pushes
//! sent in the order they were queued, each on its home's stream, until it closes, its pushes end or it falls behind.

use std::future::Future;

use link::{Event, Failure, Payload};
use link_quic::Link;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Answers the session's calls with `answer` and sends it `pushes` (`home` says each one's stream) until it ends, then says
/// why. `closed` cancelled closes it: the server's word, for one that cannot keep up (it comes back and reads again rather
/// than miss a push) or may no longer come in. An action runs to its end whatever happens to the session that asked for
/// it.
pub async fn run<P, N, A, F>(
    link: &Link,
    events: &mut mpsc::UnboundedReceiver<Event>,
    pushes: impl Fn() -> N,
    closed: &CancellationToken,
    home: impl Fn(&P) -> Option<String>,
    answer: A,
) -> String
where
    P: link::Pushes + Send + 'static,
    N: Future<Output = Option<P>>,
    A: Fn(String, Payload) -> F,
    F: Future<Output = Result<Payload, Failure>> + Send + 'static,
{
    loop {
        tokio::select! {
            biased;
            () = closed.cancelled() => {
                link.close();
                return "the server closed it".to_owned();
            }
            event = events.recv() => match event {
                Some(Event::Call { id, method, body, .. }) => {
                    let (link, answered) = (link.clone(), answer(method, body));
                    tokio::spawn(async move { link.reply(id, answered.await) });
                }
                Some(Event::Closed(closing)) => return format!("{closing:?}"),
                Some(_) => {}
                None => return "the session ended".to_owned(),
            },
            push = pushes() => {
                let Some(push) = push else {
                    link.close();
                    return "its pushes ended".to_owned();
                };
                if let Err(err) = link.push(home(&push), push).await {
                    return format!("a push was not sent: {err}");
                }
            }
        }
    }
}
