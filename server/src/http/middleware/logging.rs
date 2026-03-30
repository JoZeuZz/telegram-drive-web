use actix_web::middleware::Logger;

/// Create the pre-configured request logger.
///
/// Format: `<ip> "<method> <path> <version>" <status> <size> <duration>ms <request-id>`
pub fn create_logger() -> Logger {
    Logger::new("%a \"%r\" %s %b %Dms %{x-request-id}o")
}
