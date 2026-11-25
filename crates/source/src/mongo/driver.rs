//! The MongoDB client on the official driver: six reads and nothing else.

use std::sync::Arc;
use std::time::Duration;

use bson::{Bson, Document};
use mongodb::error::ErrorKind;
use mongodb::options::{ClientOptions, ConnectionString, ReadPreference, SelectionCriteria};
use mongodb::{Client, Cursor};

use super::{MongoClient, MongoCollection, MongoDatabase, MongoError, MongoFind, MongoResult};
use crate::error::BoxFuture;

/// How long the server may work on one operation: it is sent as maxTimeMS, so a scan the server cannot finish stops there and
/// not only on this side of the connection.
pub(crate) const MONGO_OP_TIMEOUT: Duration = Duration::from_secs(15);
pub(crate) const MONGO_CONNECT_WAIT: Duration = Duration::from_secs(8);
const MONGO_APP_NAME: &str = "remit-readonly";
const MONGO_COMMENT: &str = "remit read-only source";
const MONGO_MAX_POOL: u32 = 4;
/// The server's code for an operation that ran past its maxTimeMS.
const MAX_TIME_EXPIRED: i32 = 50;

/// The client options for `uri`: an application name that marks the connection in the server's logs as Remit's read-only
/// reader, secondaryPreferred reads unless the connection string chooses, and short waits. The error never carries the
/// connection string: the driver echoes parts of it.
pub(crate) async fn mongo_options(uri: &str) -> MongoResult<ClientOptions> {
    // Parsing a mongodb+srv:// string resolves its SRV and TXT records: a failure there is also told without the driver's
    // words, which name the hosts.
    let not_a_uri = || {
        MongoError::Other("MONGODB_URI is not a MongoDB connection string (mongodb:// or mongodb+srv://)".to_owned())
    };
    let conn = ConnectionString::parse(uri).map_err(|_| not_a_uri())?;
    let mut opts = ClientOptions::parse(conn).await.map_err(|_| not_a_uri())?;
    if opts.selection_criteria.is_none() {
        opts.selection_criteria = Some(SelectionCriteria::ReadPreference(ReadPreference::secondary_preferred(
            None,
        )));
    }
    opts.app_name = Some(MONGO_APP_NAME.to_owned());
    opts.server_selection_timeout = Some(MONGO_CONNECT_WAIT);
    opts.connect_timeout = Some(MONGO_CONNECT_WAIT);
    opts.max_pool_size = Some(MONGO_MAX_POOL);
    Ok(opts)
}

/// Opens the client of `uri`. Dropping it frees the pool.
pub(crate) fn connect_mongo(uri: String) -> BoxFuture<'static, MongoResult<Arc<dyn MongoClient>>> {
    Box::pin(async move {
        let opts = mongo_options(&uri).await?;
        let c = Client::with_options(opts)
            .map_err(|_| MongoError::Other("the MongoDB client could not be created from MONGODB_URI".to_owned()))?;
        let client: Arc<dyn MongoClient> = Arc::new(Driver(c));
        Ok(client)
    })
}

impl From<mongodb::error::Error> for MongoError {
    fn from(e: mongodb::error::Error) -> Self {
        // The labels the server and driver put on a timeout.
        if e.contains_label("NetworkTimeoutError") || e.contains_label("ExceededTimeLimitError") {
            return Self::TimedOut;
        }
        match e.kind.as_ref() {
            ErrorKind::ServerSelection { .. } => Self::Unreachable,
            ErrorKind::Command(c) if c.code == MAX_TIME_EXPIRED => Self::TimedOut,
            ErrorKind::Io(io) if io.kind() == std::io::ErrorKind::TimedOut => Self::TimedOut,
            _ => Self::Other(e.to_string()),
        }
    }
}

/// A `MongoClient` on the official driver.
struct Driver(Client);

/// `op` with the time one operation may take on this side too: one deadline over the whole of it, the cursor's reading
/// included.
async fn bounded<T>(op: impl Future<Output = MongoResult<T>>) -> MongoResult<T> {
    tokio::time::timeout(MONGO_OP_TIMEOUT, op)
        .await
        .unwrap_or(Err(MongoError::TimedOut))
}

fn comment() -> Bson {
    Bson::String(MONGO_COMMENT.to_owned())
}

impl MongoClient for Driver {
    fn databases(&self) -> BoxFuture<'_, MongoResult<Vec<MongoDatabase>>> {
        Box::pin(async move {
            let specs = bounded(async { Ok(self.0.list_databases().authorized_databases(true).await?) }).await?;
            Ok(specs
                .into_iter()
                .map(|d| MongoDatabase {
                    name: d.name,
                    size_bytes: d.size_on_disk,
                })
                .collect())
        })
    }

    fn collections<'a>(&'a self, db: &'a str) -> BoxFuture<'a, MongoResult<Vec<MongoCollection>>> {
        Box::pin(async move {
            let specs = bounded(async {
                let cur = self
                    .0
                    .database(db)
                    .list_collections()
                    .authorized_collections(true)
                    .await?;
                drain(cur).await
            })
            .await?;
            Ok(specs
                .into_iter()
                .map(|s| MongoCollection {
                    view: matches!(s.collection_type, mongodb::results::CollectionType::View),
                    name: s.name,
                })
                .collect())
        })
    }

    fn estimated_count<'a>(&'a self, db: &'a str, coll: &'a str) -> BoxFuture<'a, MongoResult<i64>> {
        Box::pin(async move {
            let n = bounded(async {
                Ok(self
                    .0
                    .database(db)
                    .collection::<Document>(coll)
                    .estimated_document_count()
                    .max_time(MONGO_OP_TIMEOUT)
                    .comment(comment())
                    .await?)
            })
            .await?;
            Ok(i64::try_from(n).unwrap_or(i64::MAX))
        })
    }

    fn count<'a>(
        &'a self,
        db: &'a str,
        coll: &'a str,
        filter: Document,
        limit: i64,
    ) -> BoxFuture<'a, MongoResult<i64>> {
        Box::pin(async move {
            let n = bounded(async {
                Ok(self
                    .0
                    .database(db)
                    .collection::<Document>(coll)
                    .count_documents(filter)
                    .limit(u64::try_from(limit).unwrap_or(0))
                    .max_time(MONGO_OP_TIMEOUT)
                    .comment(comment())
                    .await?)
            })
            .await?;
            Ok(i64::try_from(n).unwrap_or(i64::MAX))
        })
    }

    fn find<'a>(&'a self, db: &'a str, coll: &'a str, q: MongoFind) -> BoxFuture<'a, MongoResult<Vec<Document>>> {
        Box::pin(async move {
            let collection = self.0.database(db).collection::<Document>(coll);
            let mut find = collection
                .find(q.filter)
                .limit(q.limit)
                .allow_disk_use(false)
                .max_time(MONGO_OP_TIMEOUT)
                .comment(comment());
            if !q.projection.is_empty() {
                find = find.projection(q.projection);
            }
            if !q.sort.is_empty() {
                find = find.sort(q.sort);
            }
            bounded(async { drain(find.await?).await }).await
        })
    }

    fn aggregate<'a>(
        &'a self,
        db: &'a str,
        coll: &'a str,
        pipeline: Vec<Document>,
    ) -> BoxFuture<'a, MongoResult<Vec<Document>>> {
        Box::pin(async move {
            bounded(async {
                let cur = self
                    .0
                    .database(db)
                    .collection::<Document>(coll)
                    .aggregate(pipeline)
                    .allow_disk_use(false)
                    .max_time(MONGO_OP_TIMEOUT)
                    .comment(comment())
                    .await?;
                drain(cur).await
            })
            .await
        })
    }
}

/// A cursor read to its end. Every caller bounds the result with a limit, so the end is near.
async fn drain<T: serde::de::DeserializeOwned + Send + Sync>(mut cur: Cursor<T>) -> MongoResult<Vec<T>> {
    let reading = |e: mongodb::error::Error| match MongoError::from(e) {
        MongoError::Other(m) => MongoError::Other(format!("reading the result: {m}")),
        other => other,
    };
    let mut out = Vec::new();
    while cur.advance().await.map_err(reading)? {
        out.push(cur.deserialize_current().map_err(reading)?);
    }
    Ok(out)
}
