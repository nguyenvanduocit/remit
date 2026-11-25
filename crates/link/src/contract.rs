//! A contract is a permission set of the one protocol: the calls a peer granted it may make, each with its own answer
//! type, and the pushes it may receive. `contract!` writes one from a list read like `define.ts` was.

use std::marker::PhantomData;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::conn::{CallId, Millis};
use crate::envelope::{ErrorCode, Failure, Payload};

/// The calls of one contract, as the enum `contract!` writes. On the link a call is its method's name beside its arguments
/// in order, so the enum's own serde shape (what a page reads) is not the link's.
pub trait Contract: Sized {
    /// The capability a peer is granted to make these calls.
    const CAPABILITY: &'static str;
    fn method(&self) -> &'static str;
    /// The call's own timeout, when it declared one; else the link's.
    fn timeout_ms(&self) -> Option<Millis>;
    /// The line the call waits its turn in: a change that declared `order = <arg>` names the value of that argument (a
    /// chain's id), so the contract's calls naming the same one run in the order sent whatever their method; none: the
    /// line of its own method.
    fn order(&self) -> Option<&str>;
    /// The call's arguments, in order.
    fn encode_args(&self) -> Result<Payload, Failure>;
    /// The call `method` names, from its arguments; an unknown method or arguments not of its types are
    /// `INVALID_ARGUMENT`.
    fn decode_args(method: &str, args: &Payload) -> Result<Self, Failure>;
}

/// The pushes of one contract, as the enum `contract!` writes; on the link as a call is.
pub trait Pushes: Sized {
    const CAPABILITY: &'static str;
    fn name(&self) -> &'static str;
    fn encode_args(&self) -> Result<Payload, Failure>;
    fn decode_args(name: &str, args: &Payload) -> Result<Self, Failure>;
}

/// What a call is, which decides how a replay of it is safe: a `read` changes nothing, a `change` carries a `Request`
/// the callee acts on once, an `idempotent` call acts once by its own arguments, as `why` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallKind {
    Read { why: &'static str },
    Change,
    Idempotent { why: &'static str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallSpec {
    pub name: &'static str,
    pub kind: CallKind,
}

/// A call ready to send, typed by its answer `A`. Kept and sent again as it is, it retries the same action.
#[derive(Debug)]
pub struct Outgoing<C, A> {
    call: C,
    home: Option<String>,
    answer: PhantomData<fn() -> A>,
}

impl<C: Clone, A> Clone for Outgoing<C, A> {
    fn clone(&self) -> Self {
        Self {
            call: self.call.clone(),
            home: self.home.clone(),
            answer: PhantomData,
        }
    }
}

impl<C, A> Outgoing<C, A> {
    pub fn new(call: C) -> Self {
        Self {
            call,
            home: None,
            answer: PhantomData,
        }
    }

    /// The call is for an entity whose home is that node.
    #[must_use]
    pub fn at(mut self, home: String) -> Self {
        self.home = Some(home);
        self
    }

    pub fn call(&self) -> &C {
        &self.call
    }

    pub(crate) fn into_parts(self) -> (C, Option<String>) {
        (self.call, self.home)
    }
}

/// A call sent: its id, which its `Event::Answer` names, and its answer type.
#[derive(Debug)]
pub struct Pending<A> {
    id: CallId,
    answer: PhantomData<fn() -> A>,
}

impl<A> Pending<A> {
    pub(crate) fn new(id: CallId) -> Self {
        Self {
            id,
            answer: PhantomData,
        }
    }

    pub fn id(&self) -> CallId {
        self.id
    }
}

impl<A: DeserializeOwned> Pending<A> {
    /// The answer the event carried, as the call's own type.
    pub fn decode(&self, result: Result<Payload, Failure>) -> Result<A, Failure> {
        result?
            .decode()
            .map_err(|e| Failure::new(ErrorCode::InternalError, format!("the answer does not decode: {e}")))
    }
}

/// A call as its contract's enum, from its method and arguments.
pub fn decode_call<C: Contract>(method: &str, args: &Payload) -> Result<C, Failure> {
    C::decode_args(method, args)
}

/// A push as its contract's enum, from its name and arguments.
pub fn decode_push<P: Pushes>(name: &str, args: &Payload) -> Result<P, Failure> {
    P::decode_args(name, args)
}

#[doc(hidden)]
pub fn encode_answer<A: Serialize>(answer: Result<A, Failure>) -> Result<Payload, Failure> {
    Payload::encode(&answer?)
}

#[doc(hidden)]
pub fn decode_part<T: DeserializeOwned>(what: &str, args: &Payload) -> Result<T, Failure> {
    args.decode()
        .map_err(|e| Failure::new(ErrorCode::InvalidArgument, format!("the arguments of {what}: {e}")))
}

#[doc(hidden)]
pub fn decode_answer_part<T: DeserializeOwned>(what: &str, answer: &Payload) -> Result<T, Failure> {
    answer.decode().map_err(|e| {
        Failure::new(
            ErrorCode::InternalError,
            format!("the answer of {what} does not decode: {e}"),
        )
    })
}

#[doc(hidden)]
pub fn unknown(kind: &str, name: &str) -> Failure {
    Failure::new(
        ErrorCode::InvalidArgument,
        format!("no {kind} {name:?} in this contract"),
    )
}

#[doc(hidden)]
pub const fn first<T: Copy>(xs: &[T]) -> Option<T> {
    match xs {
        [x, ..] => Some(*x),
        [] => None,
    }
}

/// Writes a contract in the module it is invoked in: `CAPABILITY`; `Call`, one variant per call, tagged by method on the
/// wire; `Handler`, one typed method per call, a `change` taking `request: Request` first, so a changing call without one
/// cannot be written; `serve`, whose exhaustive match decodes a call, runs its handler and encodes that call's own answer
/// type; `client`, one stub per call returning an `Outgoing` typed by its answer; `Push`; and `CALLS` and `PUSHES`, the
/// lists the TypeScript generator reads. A `read` and an `idempotent` call say why a replay is safe. A `change` on one
/// entity says `order = <arg>`, the argument that names it: the peer's changes of that entity then run in the order sent,
/// whatever their method (a resume sent after a pause is done after it).
///
/// ```
/// mod demo {
///     #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
///     pub struct Ack {}
///     link::contract! {
///         capability: "demo";
///         reads {
///             chain_get(id: String) -> String, why = "reads one chain's view";
///             health_check() -> Ack, why = "probes and reads", timeout_ms = 150000;
///         }
///         changes {
///             chain_pause(id: String, why: String) -> Ack, order = id;
///             chain_delete(id: String) -> Ack, order = id, timeout_ms = 300000;
///             note_add(text: String) -> Ack;
///         }
///         idempotent { session_kill(session_id: String) -> Ack, why = "a session ended twice ends once"; }
///         pushes { chain_updated(chain: String); }
///     }
/// }
/// let pause = demo::client::chain_pause(link::Request { id: "r1".into() }, "c1".into(), "lunch".into());
/// assert_eq!(link::Contract::method(pause.call()), "chain_pause");
/// assert_eq!(link::Contract::timeout_ms(pause.call()), None);
/// assert_eq!(link::Contract::order(pause.call()), Some("c1"));
/// assert_eq!(link::Contract::order(demo::client::note_add(link::Request { id: "r2".into() }, "hi".into()).call()), None);
/// assert_eq!(link::Contract::timeout_ms(demo::client::health_check().call()), Some(150_000));
/// ```
#[macro_export]
macro_rules! contract {
    (
        capability: $cap:literal;
        reads { $( $(#[$rm:meta])* $rname:ident ( $($rarg:ident : $rty:ty),* ) -> $rret:ty, why = $rwhy:literal $(, timeout_ms = $rt:literal)?; )* }
        changes { $( $(#[$cm:meta])* $cname:ident ( $($carg:ident : $cty:ty),* ) -> $cret:ty $(, order = $co:ident)? $(, timeout_ms = $ct:literal)?; )* }
        idempotent { $( $(#[$im:meta])* $iname:ident ( $($iarg:ident : $ity:ty),* ) -> $iret:ty, why = $iwhy:literal $(, timeout_ms = $it:literal)?; )* }
        pushes { $( $(#[$pm:meta])* $pname:ident ( $($parg:ident : $pty:ty),* ); )* }
    ) => {
        pub const CAPABILITY: &str = $cap;

        /// Every call of the contract; as JSON (what a page reads) `{"method": <name>, "args": {<camelCase names>}}`.
        #[expect(
            non_camel_case_types,
            reason = "a variant is named as its method is on the wire and in the handler: snake_case"
        )]
        #[derive(Debug, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(tag = "method", content = "args", rename_all_fields = "camelCase")]
        pub enum Call {
            $( $(#[$rm])* $rname { $($rarg: $rty),* }, )*
            $( $(#[$cm])* $cname { request: $crate::Request, $($carg: $cty),* }, )*
            $( $(#[$im])* $iname { $($iarg: $ity),* }, )*
        }

        impl $crate::Contract for Call {
            const CAPABILITY: &'static str = $cap;

            fn method(&self) -> &'static str {
                match self {
                    $( Self::$rname { .. } => stringify!($rname), )*
                    $( Self::$cname { .. } => stringify!($cname), )*
                    $( Self::$iname { .. } => stringify!($iname), )*
                }
            }

            fn timeout_ms(&self) -> Option<$crate::Millis> {
                match self {
                    $( Self::$rname { .. } => $crate::first(&[$($rt)?]), )*
                    $( Self::$cname { .. } => $crate::first(&[$($ct)?]), )*
                    $( Self::$iname { .. } => $crate::first(&[$($it)?]), )*
                }
            }

            fn order(&self) -> Option<&str> {
                match self {
                    $( Self::$rname { .. } => None, )*
                    $( Self::$cname { $($co,)? .. } => $crate::first(&[$( ::core::convert::AsRef::<str>::as_ref($co) )?]), )*
                    $( Self::$iname { .. } => None, )*
                }
            }

            fn encode_args(&self) -> ::core::result::Result<$crate::Payload, $crate::Failure> {
                // `*self` with `ref` bindings, so a contract with no calls matches its empty enum exhaustively.
                match *self {
                    $( Self::$rname { $(ref $rarg),* } => $crate::Payload::encode(&( $($rarg,)* )), )*
                    $( Self::$cname { ref request, $(ref $carg),* } => $crate::Payload::encode(&( request, $($carg,)* )), )*
                    $( Self::$iname { $(ref $iarg),* } => $crate::Payload::encode(&( $($iarg,)* )), )*
                }
            }

            fn decode_args(method: &str, args: &$crate::Payload) -> ::core::result::Result<Self, $crate::Failure> {
                match method {
                    $( stringify!($rname) => {
                        let ( $($rarg,)* ): ( $($rty,)* ) = $crate::decode_part(method, args)?;
                        Ok(Self::$rname { $($rarg),* })
                    } )*
                    $( stringify!($cname) => {
                        let ( request, $($carg,)* ): ( $crate::Request, $($cty,)* ) = $crate::decode_part(method, args)?;
                        Ok(Self::$cname { request, $($carg),* })
                    } )*
                    $( stringify!($iname) => {
                        let ( $($iarg,)* ): ( $($ity,)* ) = $crate::decode_part(method, args)?;
                        Ok(Self::$iname { $($iarg),* })
                    } )*
                    _ => Err($crate::unknown("call", method)),
                }
            }
        }

        /// What answers the contract's calls.
        pub trait Handler: Sync {
            $( $(#[$rm])* fn $rname(&self, $($rarg: $rty),*)
                -> impl ::core::future::Future<Output = ::core::result::Result<$rret, $crate::Failure>> + Send; )*
            $( $(#[$cm])* fn $cname(&self, request: $crate::Request, $($carg: $cty),*)
                -> impl ::core::future::Future<Output = ::core::result::Result<$cret, $crate::Failure>> + Send; )*
            $( $(#[$im])* fn $iname(&self, $($iarg: $ity),*)
                -> impl ::core::future::Future<Output = ::core::result::Result<$iret, $crate::Failure>> + Send; )*
        }

        /// The answer of any call, as its own type: what a caller that names the call only at run time (a CLI) decodes with
        /// `decode_answer`. Serialised, it is the bare answer.
        #[expect(
            non_camel_case_types,
            reason = "a variant is named as its method is on the wire and in the handler: snake_case"
        )]
        #[derive(Debug, ::serde::Serialize)]
        #[serde(untagged)]
        pub enum Answer {
            $( $rname($rret), )*
            $( $cname($cret), )*
            $( $iname($iret), )*
        }

        /// The answer of a call of `method`, as its own type.
        pub fn decode_answer(
            method: &str,
            answer: &$crate::Payload,
        ) -> ::core::result::Result<Answer, $crate::Failure> {
            match method {
                $( stringify!($rname) => $crate::decode_answer_part(method, answer).map(Answer::$rname), )*
                $( stringify!($cname) => $crate::decode_answer_part(method, answer).map(Answer::$cname), )*
                $( stringify!($iname) => $crate::decode_answer_part(method, answer).map(Answer::$iname), )*
                _ => Err($crate::unknown("call", method)),
            }
        }

        /// The answer of a call of `method` read from a readable format (a fake server's JSON), as its own type.
        pub fn read_answer<'de, D: ::serde::Deserializer<'de>>(
            method: &str,
            answer: D,
        ) -> ::core::result::Result<Answer, $crate::Failure> {
            let read = |e: D::Error| $crate::Failure::new(
                $crate::ErrorCode::InvalidArgument,
                ::std::format!("the answer of {method}: {e}"),
            );
            match method {
                $( stringify!($rname) => <$rret as ::serde::Deserialize>::deserialize(answer).map(Answer::$rname).map_err(read), )*
                $( stringify!($cname) => <$cret as ::serde::Deserialize>::deserialize(answer).map(Answer::$cname).map_err(read), )*
                $( stringify!($iname) => <$iret as ::serde::Deserialize>::deserialize(answer).map(Answer::$iname).map_err(read), )*
                _ => Err($crate::unknown("call", method)),
            }
        }

        /// Answers one call with its handler: what does not decode is `INVALID_ARGUMENT`.
        pub async fn serve<H: Handler>(
            handler: &H,
            method: &str,
            args: &$crate::Payload,
        ) -> ::core::result::Result<$crate::Payload, $crate::Failure> {
            match $crate::decode_call::<Call>(method, args)? {
                $( Call::$rname { $($rarg),* } => $crate::encode_answer(handler.$rname($($rarg),*).await), )*
                $( Call::$cname { request, $($carg),* } =>
                    $crate::encode_answer(handler.$cname(request, $($carg),*).await), )*
                $( Call::$iname { $($iarg),* } => $crate::encode_answer(handler.$iname($($iarg),*).await), )*
            }
        }

        /// One stub per call, typed by its answer.
        pub mod client {
            #[expect(
                unused_imports,
                reason = "the stubs name the contract's types as its caller wrote them; only a contract with no calls leaves it unused"
            )]
            use super::*;
            $( pub fn $rname($($rarg: $rty),*) -> $crate::Outgoing<Call, $rret> {
                $crate::Outgoing::new(Call::$rname { $($rarg),* })
            } )*
            $( pub fn $cname(request: $crate::Request, $($carg: $cty),*) -> $crate::Outgoing<Call, $cret> {
                $crate::Outgoing::new(Call::$cname { request, $($carg),* })
            } )*
            $( pub fn $iname($($iarg: $ity),*) -> $crate::Outgoing<Call, $iret> {
                $crate::Outgoing::new(Call::$iname { $($iarg),* })
            } )*
        }

        /// Every push of the contract; as JSON (what a page reads) `{"push": <name>, "args": {<camelCase names>}}`.
        #[expect(
            non_camel_case_types,
            reason = "a variant is named as its method is on the wire and in the handler: snake_case"
        )]
        #[derive(Debug, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(tag = "push", content = "args", rename_all_fields = "camelCase")]
        pub enum Push {
            $( $(#[$pm])* $pname { $($parg: $pty),* }, )*
        }

        impl $crate::Pushes for Push {
            const CAPABILITY: &'static str = $cap;

            fn name(&self) -> &'static str {
                // `*self`, so a contract with no pushes matches its empty enum exhaustively.
                match *self {
                    $( Self::$pname { .. } => stringify!($pname), )*
                }
            }

            fn encode_args(&self) -> ::core::result::Result<$crate::Payload, $crate::Failure> {
                match *self {
                    $( Self::$pname { $(ref $parg),* } => $crate::Payload::encode(&( $($parg,)* )), )*
                }
            }

            fn decode_args(name: &str, args: &$crate::Payload) -> ::core::result::Result<Self, $crate::Failure> {
                match name {
                    $( stringify!($pname) => {
                        let ( $($parg,)* ): ( $($pty,)* ) = $crate::decode_part(name, args)?;
                        Ok(Self::$pname { $($parg),* })
                    } )*
                    _ => Err($crate::unknown("push", name)),
                }
            }
        }

        pub const CALLS: &[$crate::CallSpec] = &[
            $( $crate::CallSpec { name: stringify!($rname), kind: $crate::CallKind::Read { why: $rwhy } }, )*
            $( $crate::CallSpec { name: stringify!($cname), kind: $crate::CallKind::Change }, )*
            $( $crate::CallSpec { name: stringify!($iname), kind: $crate::CallKind::Idempotent { why: $iwhy } }, )*
        ];

        pub const PUSHES: &[&str] = &[ $( stringify!($pname) ),* ];
    };
}
