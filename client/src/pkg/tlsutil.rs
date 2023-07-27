use slog::o;
use slog::Drain;
pub(crate) mod tlsutil;
mod cipher_suites;

#[allow(dead_code)]
pub fn default_logger() -> slog::Logger {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    slog::Logger::root(drain, o!())
}