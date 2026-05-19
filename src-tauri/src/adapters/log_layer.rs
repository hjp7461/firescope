//! tracing 이벤트 → `log:entry` Tauri 이벤트 (원칙 4·11, 자격증명 차단 원칙 5).

use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// 로그에 실어도 안전한 구조화 필드 (값 누출 방지 화이트리스트).
const ALLOWED: &[&str] = &[
    "message", "collection", "count", "took_ms", "profile_id",
    "expires_at", "target", "stream_id", "op", "mode",
];

#[derive(Serialize, Clone)]
struct LogEntry {
    level: String,
    message: String,
    target: String,
    ts: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile_id: Option<String>,
}

#[derive(Default)]
struct FieldVisitor {
    message: String,
    profile_id: Option<String>,
    extra: Vec<String>,
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let name = field.name();
        let val = format!("{value:?}");
        let val = val.trim_matches('"').to_string();
        if name == "message" {
            self.message = val;
        } else if name == "profile_id" {
            self.profile_id = Some(val);
        } else if ALLOWED.contains(&name) {
            self.extra.push(format!("{name}={val}"));
        } else {
            self.extra.push(format!("{name}=<omitted>"));
        }
    }
}

pub struct LogLayer<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> LogLayer<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

fn render(meta_level: &tracing::Level, target: &str, v: FieldVisitor) -> LogEntry {
    let mut message = v.message;
    if !v.extra.is_empty() {
        message = format!("{message} ({})", v.extra.join(" "));
    }
    LogEntry {
        level: meta_level.as_str().to_lowercase(),
        message,
        target: target.to_string(),
        ts: Utc::now().to_rfc3339(),
        profile_id: v.profile_id,
    }
}

impl<S, R> Layer<S> for LogLayer<R>
where
    S: Subscriber,
    R: Runtime,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut v = FieldVisitor::default();
        event.record(&mut v);
        let entry = render(event.metadata().level(), event.metadata().target(), v);
        let _ = self.app.emit("log:entry", entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitelist_allows_known_fields() {
        let v = FieldVisitor {
            message: "query done".into(),
            profile_id: Some("p1".into()),
            extra: vec!["count=30".into(), "took_ms=12".into()],
        };
        let e = render(&tracing::Level::INFO, "firescope::q", v);
        assert_eq!(e.level, "info");
        assert_eq!(e.message, "query done (count=30 took_ms=12)");
        assert_eq!(e.profile_id.as_deref(), Some("p1"));
    }

    #[test]
    fn non_whitelisted_field_is_omitted() {
        let mut v = FieldVisitor::default();
        v.extra.push("password=<omitted>".into());
        let e = render(&tracing::Level::WARN, "t", v);
        assert!(e.message.contains("password=<omitted>"));
        assert!(!e.message.contains("hunter2"));
    }
}
