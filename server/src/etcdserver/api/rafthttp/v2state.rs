use chrono::{DateTime, Local, TimeZone};
use serde::{Deserialize, Deserializer, Serializer};
use slog::o;

pub mod queue;
pub mod leader;
pub(crate) mod server;
use slog::Drain;
#[allow(dead_code)]
pub fn default_logger() -> slog::Logger {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    slog::Logger::root(drain, o!())
}
pub fn serialize_datetime<S>(datetime: &DateTime<Local>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
{
    let timestamp = datetime.timestamp_nanos();
    serializer.serialize_i64(timestamp)
}

pub fn deserialize_datetime<'de, D>(deserializer: D) -> Result<DateTime<Local>, D::Error>
    where
        D: Deserializer<'de>,
{
    let timestamp = i64::deserialize(deserializer)?;
    Ok(Local.timestamp_nanos(timestamp))
}
