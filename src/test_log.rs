//! Captures the log events a closure emits on the calling thread, for the tests that assert on
//! what is logged: its level, its `kind`, its fields. A scoped subscriber, no global one; the
//! interest cache is rebuilt once it is in place, so a callsite another thread registered just
//! before cannot be left cached as "never".

use std::sync::{Arc, Mutex};

use tracing::field::{Field, Visit};
use tracing::level_filters::LevelFilter;
use tracing::span::{Attributes, Id, Record};
use tracing::subscriber::Interest;
use tracing::{Event, Level, Metadata, Subscriber};

/// One log event.
#[derive(Debug, Clone)]
pub struct Captured {
    pub level: Level,
    pub message: String,
    pub fields: Vec<(String, String)>,
}

impl Captured {
    pub fn field(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn kind(&self) -> Option<&str> {
        self.field("kind")
    }

    /// Whether this is a `level` event of `kind`.
    pub fn is(&self, level: Level, kind: &str) -> bool {
        self.level == level && self.kind() == Some(kind)
    }
}

/// Runs `f`, returning every event it emitted on this thread.
pub fn capture(f: impl FnOnce()) -> Vec<Captured> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let subscriber = Capture {
        events: events.clone(),
    };
    tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        f()
    });
    let events = events.lock().unwrap().clone();
    events
}

struct Capture {
    events: Arc<Mutex<Vec<Captured>>>,
}

impl Subscriber for Capture {
    fn register_callsite(&self, _: &'static Metadata<'static>) -> Interest {
        Interest::always()
    }

    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        Some(LevelFilter::TRACE)
    }

    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _: &Id, _: &Record<'_>) {}

    fn record_follows_from(&self, _: &Id, _: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        self.events.lock().unwrap().push(Captured {
            level: *event.metadata().level(),
            message: fields.message,
            fields: fields.fields,
        });
    }

    fn enter(&self, _: &Id) {}

    fn exit(&self, _: &Id) {}
}

#[derive(Default)]
struct Fields {
    message: String,
    fields: Vec<(String, String)>,
}

impl Fields {
    fn put(&mut self, field: &Field, value: String) {
        if field.name() == "message" {
            self.message = value;
        } else {
            self.fields.push((field.name().to_string(), value));
        }
    }
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.put(field, value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.put(field, value.to_string());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.put(field, value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.put(field, value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.put(field, format!("{value:?}"));
    }
}
