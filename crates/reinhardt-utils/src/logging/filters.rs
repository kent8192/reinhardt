/// A filter that uses a callback function to determine whether a log record should pass.
pub trait CallbackFilter {}
/// A filter that determines whether a log record should be processed.
pub trait Filter {}
/// A filter that only passes log records when the application is not in debug mode.
pub struct RequireDebugFalse;
/// A filter that only passes log records when the application is in debug mode.
pub struct RequireDebugTrue;
