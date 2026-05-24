use axum::response::sse::{Event, Sse};
use futures::stream::Stream;
use std::convert::Infallible;

/// Wraps a stream of Event results into an Axum Sse response.
pub fn into_sse_stream<S>(stream: S) -> Sse<S>
where
    S: Stream<Item = Result<Event, Infallible>> + Send + 'static,
{
    Sse::new(stream)
}

/// Creates a standard text Event.
pub fn text_event(text: impl Into<String>) -> Event {
    Event::default().data(text.into())
}

/// Creates a JSON-serialized Event.
pub fn json_event<T: serde::Serialize>(data: &T) -> Result<Event, axum::Error> {
    Event::default().json_data(data)
}
