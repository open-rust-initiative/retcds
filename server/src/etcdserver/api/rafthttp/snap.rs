use slog::o;

pub mod message;
mod db;
mod snap_shotter;
use slog::Drain;
#[allow(dead_code)]
fn default_logger() -> slog::Logger {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    slog::Logger::root(drain, o!())
}