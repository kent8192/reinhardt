use std::sync::Arc;

/// Whether a streaming handler produces or consumes messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingHandlerKind {
    Producer,
    Consumer,
}

/// A registered streaming handler (producer or consumer).
#[derive(Clone)]
pub struct StreamingHandlerRegistration {
    pub topic: &'static str,
    pub group: Option<&'static str>,
    pub name: &'static str,
    pub kind: StreamingHandlerKind,
    /// Factory for spawning consumer tasks. `None` for producers.
    pub consumer_factory: Option<Arc<dyn ConsumerFactory>>,
}

/// Factory that spawns a Kafka consumer task for a topic.
pub trait ConsumerFactory: Send + Sync {
    fn spawn(&self, brokers: Vec<String>, topic: &'static str, group: &'static str);
}

/// Collects producer and consumer handler registrations.
///
/// Use the `streaming_routes!` macro or builder methods to populate.
#[derive(Default)]
pub struct StreamingRouter {
    pub(crate) handlers: Vec<StreamingHandlerRegistration>,
}

impl StreamingRouter {
    pub fn new() -> Self {
        Self { handlers: Vec::new() }
    }

    /// Register a producer handler by topic and name.
    pub fn producer(mut self, topic: &'static str, name: &'static str) -> Self {
        self.handlers.push(StreamingHandlerRegistration {
            topic,
            group: None,
            name,
            kind: StreamingHandlerKind::Producer,
            consumer_factory: None,
        });
        self
    }

    /// Consume the router and return the registered handlers.
    pub fn into_handlers(self) -> Vec<StreamingHandlerRegistration> {
        self.handlers
    }

    /// Register a consumer handler with a factory.
    pub fn consumer(
        mut self,
        topic: &'static str,
        group: &'static str,
        name: &'static str,
        factory: Arc<dyn ConsumerFactory>,
    ) -> Self {
        self.handlers.push(StreamingHandlerRegistration {
            topic,
            group: Some(group),
            name,
            kind: StreamingHandlerKind::Consumer,
            consumer_factory: Some(factory),
        });
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[rstest]
    fn router_starts_empty() {
        let router = StreamingRouter::new();
        assert!(router.handlers.is_empty());
    }

    #[rstest]
    fn producer_registration_stored() {
        let router = StreamingRouter::new().producer("orders", "create_order");
        assert_eq!(router.handlers.len(), 1);
        assert_eq!(router.handlers[0].topic, "orders");
        assert_eq!(router.handlers[0].kind, StreamingHandlerKind::Producer);
        assert_eq!(router.handlers[0].name, "create_order");
    }

    #[rstest]
    fn multiple_handlers_stored() {
        struct NoopFactory;
        impl ConsumerFactory for NoopFactory {
            fn spawn(&self, _: Vec<String>, _: &'static str, _: &'static str) {}
        }

        let router = StreamingRouter::new()
            .producer("orders", "create_order")
            .consumer("orders", "processor", "handle_order", Arc::new(NoopFactory));

        assert_eq!(router.handlers.len(), 2);
        assert_eq!(router.handlers[1].kind, StreamingHandlerKind::Consumer);
    }
}
