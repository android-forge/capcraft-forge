// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah

//! The FFmpeg filter-chain builder for audio filters. Pictures are drawn by
//! their packages' shaders on the GPU; sound is still FFmpeg's.
//!
//! The chains come from the effect catalogue (`concat-effects`): every
//! built-in audio filter is a package whose manifest carries its template,
//! and whose fixtures pin the exact string at its default, minimum and
//! maximum settings. What this module owns is the seam the exporter calls
//! through.

use concat_effects::Catalogue;
use concat_project::model::AppliedFilter;

/// The complete FFmpeg *audio* filter string for a clip's filters, or the
/// empty string if it has none. Filters apply in the order they were added:
/// EQ before a limiter is a different sound from the reverse.
pub fn audio_filter_chain(filters: &[AppliedFilter]) -> String {
    let filters: Vec<concat_effects::AppliedFilter> = filters.iter().map(engine_filter).collect();
    Catalogue::builtin().audio_chain(&filters)
}

/// A clip's saved link to a package, as the effects engine reads it.
pub fn engine_filter(applied: &AppliedFilter) -> concat_effects::AppliedFilter {
    concat_effects::AppliedFilter {
        id: applied.id.clone(),
        params: applied.params.clone(),
        enabled: applied.enabled,
        keys: applied
            .keys
            .iter()
            .map(|(name, keys)| {
                let keys = keys
                    .iter()
                    .map(|key| concat_effects::ParamKey { at: key.at, value: key.value, ease: key.ease.into() })
                    .collect();
                (name.clone(), keys)
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn applied(id: &str) -> AppliedFilter {
        AppliedFilter::new(id)
    }

    #[test]
    fn the_exporter_reaches_the_built_in_catalogue() {
        assert_eq!(
            audio_filter_chain(&[applied("echo")]),
            "aecho=0.8:0.85:250:0.40"
        );
        assert_eq!(audio_filter_chain(&[]), "");
    }
}
