//! Defines how facts about particular expressions within the AST learned in the Gather
//! compilation, are transferred to the Instrument compilation.
//!
//! Facts can be "markers", which determine whether or not a particular expression meets some
//! condition, or "payloads", which carry some data along with the expression. This mechanism
//! is expressed via the `F` generic on the [`SpanFacts<F>`] type. Markers use `F = ()`.
//!
//! [`SpanFacts::record`] and [`SpanFacts::mark`] can be used by the Gather compilation to
//! contribute discovered facts (both payloads and markers, respectively) about a particular span.
//!
//! [`SpanFacts::get`] and [`SpanFacts::contains`] can be used by the Instrument compilation to
//! query for facts about a particular span.

use crate::callbacks::gather::first_pass_info::span_key::SpanKey;

/// Generic span-keyed fact store.
///
/// Importantly, all records/lookups will convert the identifying span to a [`SpanKey`], so that
/// lookups are consistent between the two compilations, which  necessarily use two different parse
/// sessions. See [`SpanKey::from_span`] for more information.
#[derive(Debug)]
pub struct SpanFacts<F> {
    inner: std::collections::HashMap<SpanKey, F>,
}

// Manual Default impl so `F: Default` isn't required.
impl<F> Default for SpanFacts<F> {
    fn default() -> Self {
        Self {
            inner: std::collections::HashMap::new(),
        }
    }
}

impl<F> SpanFacts<F> {
    /// Record a fact at `span`. No-op if the span doesn't map to a stable
    /// `SpanKey` (dummy spans, unmapped expansion sites, etc.), as this means
    /// this fact will be innaccessible during the instrument compilation.
    pub fn record(
        &mut self,
        span: rustc_span::Span,
        sm: &rustc_span::source_map::SourceMap,
        fact: F,
    ) {
        if let Some(key) = SpanKey::from_span(span, sm) {
            self.inner.insert(key, fact);
        }
    }

    /// Look up a fact at `span`.
    pub fn get(
        &self,
        span: rustc_span::Span,
        sm: &rustc_span::source_map::SourceMap,
    ) -> Option<&F> {
        SpanKey::from_span(span, sm)
            .as_ref()
            .and_then(|k| self.inner.get(k))
    }
}

// Sugar for marker-style facts.
impl SpanFacts<()> {
    /// Marks this span as meeting the condition required by the fact.
    pub fn mark(&mut self, span: rustc_span::Span, sm: &rustc_span::source_map::SourceMap) {
        self.record(span, sm, ());
    }

    /// Checks whether this span meets the condition required by the fact.
    pub fn contains(&self, span: rustc_span::Span, sm: &rustc_span::source_map::SourceMap) -> bool {
        self.get(span, sm).is_some()
    }
}
