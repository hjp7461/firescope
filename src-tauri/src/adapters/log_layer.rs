//! tracing 이벤트 → `log:entry` Tauri 이벤트 (원칙 4·11, 자격증명 차단 원칙 5).

use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// 로그에 실어도 안전한 구조화 필드 (값 누출 방지 화이트리스트).
/// NOTE: "message", "profile_id", "session_id"는 record_debug에서 먼저 특별 처리되므로 여기 없어도 됨.
const ALLOWED: &[&str] = &[
    "message", "collection", "count", "took_ms",
    "expires_at", "target", "stream_id", "session_id", "op", "mode",
];

#[derive(Serialize, Clone)]
struct LogEntry {
    level: String,
    message: String,
    target: String,
    ts: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
}

#[derive(Default)]
struct FieldVisitor {
    message: String,
    profile_id: Option<String>,
    session_id: Option<String>,
    extra: Vec<String>,
}

impl FieldVisitor {
    /// 필드 이름과 Debug 값을 받아 분류·저장한다.
    /// 비허용 필드는 `value`를 절대 포맷하지 않고 `<omitted>` 리터럴만 기록한다.
    fn put_field(&mut self, name: &str, value: &dyn std::fmt::Debug) {
        if name == "message" {
            let v = format!("{value:?}");
            self.message = v.trim_matches('"').to_string();
        } else if name == "profile_id" {
            let v = format!("{value:?}");
            self.profile_id = Some(v.trim_matches('"').to_string());
        } else if name == "session_id" {
            let v = format!("{value:?}");
            self.session_id = Some(v.trim_matches('"').to_string());
        } else if ALLOWED.contains(&name) {
            let v = format!("{value:?}");
            self.extra.push(format!("{name}={}", v.trim_matches('"')));
        } else {
            // 비허용 필드: value에 절대 접근하지 않는다 (원칙 5).
            self.extra.push(format!("{name}=<omitted>"));
        }
    }
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.put_field(field.name(), value);
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
        session_id: v.session_id,
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
            session_id: Some("s-1".into()),
            extra: vec!["count=30".into(), "took_ms=12".into()],
        };
        let e = render(&tracing::Level::INFO, "firescope::q", v);
        assert_eq!(e.level, "info");
        assert_eq!(e.message, "query done (count=30 took_ms=12)");
        assert_eq!(e.profile_id.as_deref(), Some("p1"));
        assert_eq!(e.session_id.as_deref(), Some("s-1"));
    }

    #[test]
    fn session_id_field_is_captured() {
        let mut v = FieldVisitor::default();
        v.put_field("session_id", &"s-123");
        assert_eq!(v.session_id.as_deref(), Some("s-123"));
        // session_id must NOT appear in extra (it's promoted to a struct field)
        assert!(v.extra.iter().all(|s| !s.contains("s-123")));
    }

    #[test]
    fn non_whitelisted_field_is_omitted() {
        let mut v = FieldVisitor::default();
        v.extra.push("password=<omitted>".into());
        let e = render(&tracing::Level::WARN, "t", v);
        assert!(e.message.contains("password=<omitted>"));
        assert!(!e.message.contains("hunter2"));
    }

    /// put_field 실제 분류 경로를 검증:
    /// 비허용 필드의 값은 FieldVisitor 어디에도 나타나지 않아야 한다 (원칙 5).
    #[test]
    fn put_field_non_whitelisted_never_materializes_value() {
        let mut v = FieldVisitor::default();
        // "password"는 ALLOWED에 없으므로 값("hunter2")은 절대 포맷되어선 안 된다.
        v.put_field("password", &"hunter2");

        // (a) extra 엔트리는 정확히 "password=<omitted>" 이어야 한다.
        assert_eq!(v.extra, vec!["password=<omitted>".to_string()]);

        // (b) visitor 전체 상태에 비밀값이 없어야 한다.
        assert!(!v.message.contains("hunter2"));
        assert!(v.profile_id.as_deref().unwrap_or("").is_empty() || !v.profile_id.as_deref().unwrap().contains("hunter2"));
        for entry in &v.extra {
            assert!(!entry.contains("hunter2"), "extra entry leaked secret: {entry}");
        }

        // (c) render 이후 최종 출력에도 비밀값이 없어야 한다.
        v.message = "baseline".into();
        let e = render(&tracing::Level::ERROR, "t", v);
        assert!(!e.message.contains("hunter2"), "rendered message leaked secret: {}", e.message);
        assert!(e.message.contains("password=<omitted>"));
    }
}
