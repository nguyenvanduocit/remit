//! A release is traced as the work it does (ADR-0119): its procedure's run, its going live and its taking back are each a
//! trace of its own, the root span `remit.release` with the release's id, linked (`follows_from`, which the exporter
//! sends as a span link) to the span it was started from. A span link needs the span it points at, and a chain, which
//! lives for days, has none; so the release names its chains the way a chain's records do: a child span for each landing
//! it carries, tagged with that landing's chain. A search on `@remit.chain_id` finds that marker span, whose trace is the
//! release's. The release's own logs and step spans carry `remit.release_id`, not a chain: a release is of many.

use model::Release;
use telemetry::attr;
use tracing::Span;

use crate::release::Own;

/// The span of `action` (`run`, `live`, `take_back`) on rel, with a marker for each landing of `own`. Its version is
/// recorded once the procedure says it (`Span::current().record("version", ..)`).
pub(crate) fn release_span(action: &'static str, rel: &Release, own: &Own) -> Span {
    let caller = Span::current();
    let span = tracing::info_span!(
        parent: None,
        attr::RELEASE_SPAN,
        { attr::RELEASE_ID } = %rel.id,
        repo = %rel.repo,
        action,
        version = tracing::field::Empty
    );
    if caller.id().is_some() {
        span.follows_from(&caller);
    }
    for l in &own.landings {
        // a marker: it ends as it is made, and the exporter sends it with its chain
        drop(tracing::info_span!(
            parent: &span,
            attr::RELEASE_LANDING_SPAN,
            { attr::CHAIN_ID } = %l.chain_id,
            { attr::LANDING_ID } = %l.id
        ));
    }
    span
}
