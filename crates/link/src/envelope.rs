//! What travels on a link's streams: the frames, the error envelope, the handshake, and how a frame is cut out of a
//! stream's bytes (a 4-byte big-endian length, then that many bytes of postcard). Postcard because both ends of a session
//! are one build (a peer of another bundle is refused at hello), so nothing on the wire names a field: a value is its
//! fields in order. It does not describe itself, so a type on the link has no `flatten`, no internally or adjacently
//! tagged enum and no `serde_json::Value`, and a body writes the fields `skip_serializing_if` leaves out (`written`,
//! ADR-0095).

use std::fmt;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Why a call has no value. The domain codes are what a handler answers (`NOT_FOUND` … `CONFLICT`, the orchestrator's
/// `ErrorKind`); the rest the link itself says: no answer in time, no session, a frame that does not decode, a handler
/// that ended abnormally, a call outside what the peer was granted, or a call for an entity another node is home of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    NotFound,
    Ended,
    Unavailable,
    Failed,
    /// What the call would change moved since the caller read it: nothing was changed, and the failure's detail holds
    /// the record as it is now, for the caller to go on from.
    Conflict,
    Timeout,
    Disposed,
    Disconnected,
    Aborted,
    InternalError,
    InvalidArgument,
    Forbidden,
    NotHere,
}

/// A call's error: the code a channel acts on, the words for the owner (they change; the code does not), and what the
/// channel needs to act on it beyond the code (`detail`): each contract types its own (`model::FailureDetail`), so
/// a channel reads it with `detail_as` and never parses the words. The detail is carried encoded, as a body is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    pub code: ErrorCode,
    pub message: String,
    pub detail: Option<Payload>,
}

impl Failure {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
        }
    }

    /// The failure with its contract's detail; a detail that does not encode is the callee's defect, and is left out.
    #[must_use]
    pub fn with_detail<D: Serialize>(mut self, detail: &D) -> Self {
        self.detail = Payload::encode(detail).ok();
        self
    }

    /// The detail as its contract's type; none when there is none, or it is not of that type.
    pub fn detail_as<D: DeserializeOwned>(&self) -> Option<D> {
        self.detail.as_ref().and_then(|d| d.decode().ok())
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for Failure {}

/// The caller's part of an action: its id, made once per action and sent again with each retry, so the callee acts once
/// and answers each retry as it answered the first. Every `change` call of a contract takes one by type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Request {
    pub id: String,
}

/// A node of the network: its id (the hash of its key) and its Ed25519 public key. Recorded at `hello`, not yet proven
/// (ADR-0072 decision 12, *Revisit when*: proof of possession).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: String,
    pub public_key: String,
}

/// The dialling peer's first frame: who it is, its credential (the owner key, the runtime key, a worker key), the
/// contracts it asks to call (`capabilities`), the contracts it answers calls of (`offers`), and the build of its
/// contracts (`bundle`, none for a development build, which is held to none).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    pub node: Node,
    pub credential: String,
    pub capabilities: Vec<String>,
    pub offers: Vec<String>,
    pub bundle: Option<String>,
}

/// The accepting peer's answer to an admitted hello: who it is and the contracts the dialler may call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Welcome {
    pub node: Node,
    pub granted: Vec<String>,
}

/// Why a hello was turned away: `bad_credential` is final (the shell shows the way in), `new_bundle` sends the shell to
/// load itself again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCode {
    BadCredential,
    NewBundle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    pub code: RefusalCode,
    pub reason: String,
}

/// A body on the link: a call's arguments, an answer, a push's arguments or a failure's detail, encoded once by the side
/// that typed it and decoded once by the side that types it. The link carries it without reading it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Payload(Vec<u8>);

impl Payload {
    pub fn encode<T: Serialize + ?Sized>(value: &T) -> Result<Self, Failure> {
        postcard::to_allocvec(&crate::written::every_field(value))
            .map(Self)
            .map_err(|e| Failure::new(ErrorCode::InternalError, format!("encode a body: {e}")))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// The body as `T`; what is not a `T` says why.
    pub fn decode<T: DeserializeOwned>(&self) -> Result<T, postcard::Error> {
        postcard::from_bytes(&self.0)
    }
}

/// One frame. The control stream carries `hello`, `welcome`, `refused`, `ping`, `pong` and `behind`; a call stream one
/// `call` one way and one `ok` or `err` back; a push stream `push` frames in order. A call names its `method` and a push
/// its `name` beside the body, which holds only the arguments. `seq` counts the caller's calls of one
/// line of one contract from 0 in this session: those naming one `order` (a change of one entity, `Contract::order`), else
/// those of one method; `home` is the node the call is for, none meaning the callee's own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Frame {
    Hello(Hello),
    Welcome(Welcome),
    Refused(Refusal),
    Ping,
    Pong,
    Behind,
    Call {
        seq: u64,
        order: Option<String>,
        contract: String,
        method: String,
        home: Option<String>,
        body: Payload,
    },
    Ok {
        value: Payload,
    },
    Err(Failure),
    Push {
        contract: String,
        name: String,
        body: Payload,
    },
}

/// The bytes of one frame on a stream.
pub fn encode(frame: &Frame) -> Result<Vec<u8>, Failure> {
    // The length goes first, written once the frame is.
    let out = postcard::to_extend(frame, vec![0; 4])
        .map_err(|e| Failure::new(ErrorCode::InternalError, format!("encode a frame: {e}")))?;
    let len = u32::try_from(out.len().saturating_sub(4))
        .map_err(|_| Failure::new(ErrorCode::InternalError, "a frame is larger than 4 GiB"))?;
    let mut out = out;
    if let Some(head) = out.get_mut(..4) {
        head.copy_from_slice(&len.to_be_bytes());
    }
    Ok(out)
}

/// Cuts frames out of one stream's bytes as they arrive, in any chunks.
#[derive(Debug, Default)]
pub struct Reader {
    buf: Vec<u8>,
}

impl Reader {
    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// The next whole frame, or none while it has not all arrived.
    pub fn next_frame(&mut self) -> Option<Result<Frame, postcard::Error>> {
        let head: [u8; 4] = self.buf.get(..4)?.try_into().ok()?;
        let end = usize::try_from(u32::from_be_bytes(head)).ok()?.checked_add(4)?;
        let frame = postcard::from_bytes(self.buf.get(4..end)?);
        self.buf.drain(..end);
        Some(frame)
    }
}
