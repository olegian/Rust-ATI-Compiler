//! Cross-compilation stable key for rustc `Span`s.
//!
//! `FirstPassInfo` is built in compilation pass 1 and consulted in pass 2.
//! Raw `Span` values carry a `SyntaxContext` and reference the active
//! `SourceMap`'s `BytePos` interning, both of which are recreated each
//! invocation. Most times, this will not cause a problem, as long as files are
//! loaded in a consistent manner (which as of right now, they are). Before that
//! was fixed however, span inconsistency caused problems which SpanKey fixed.
//! For purely defensive reasons, this stable-keying functionality is left.
//!
//! `SpanKey` is a `(file, lo, hi)` triple computed by resolving
//! the span through the `SourceMap` to the *file-local* byte offsets,
//! which are a pure function of the source bytes.
//!
//! Spans inside macro expansions / desugarings are normalized via
//! `source_callsite()` so a query on a syntactic AST node still hits the
//! appropriate entry.

use std::path::PathBuf;

/// A representation of a `rustc_span::Span` that is consistent between separate compilations.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpanKey {
    /// Path-string identifier for the source file.
    file: FileKey,
    /// Low byte offset.
    lo: u32,
    /// High byte offset.
    hi: u32,
}

/// A representation of a loaded file, whihc is consistent between separate compilations.
///
/// Within rustc, a FileName can be represented in a variety of different ways, from
/// `FileName::Real(_)`, which represents a real on-disk file, with a path to
/// `FileName::Anon(_)`, a source string with no associated file, to `FileName::MacroExpansion(_)`
/// which is the result of a generated macro body, which also doesn't have an associated file.
///
/// `FileKey` is a consistent, simplified representation of all different file sources, capable
/// of being used through DATIR.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum FileKey {
    Real(PathBuf),
    Other(String),
}

impl FileKey {
    /// Constructs a FileKey from a rustc `FileName`.
    fn from_filename(name: &rustc_span::FileName) -> Self {
        match name {
            rustc_span::FileName::Real(rfn) => FileKey::Real(
                rfn.path(rustc_span::RemapPathScopeComponents::MACRO)
                    .to_path_buf(),
            ),
            other => FileKey::Other(format!("{other:?}")),
        }
    }
}

impl SpanKey {
    /// Resolve `span` to a stable `(file, lo, hi)` key.
    pub fn from_span(
        span: rustc_span::Span,
        sm: &rustc_span::source_map::SourceMap,
    ) -> Option<Self> {
        let span = span.source_callsite();
        if span.is_dummy() {
            return None;
        }
        let lo = sm.lookup_byte_offset(span.lo());
        let hi = sm.lookup_byte_offset(span.hi());
        Some(Self {
            file: FileKey::from_filename(&lo.sf.name),
            lo: lo.pos.0,
            hi: hi.pos.0,
        })
    }
}
