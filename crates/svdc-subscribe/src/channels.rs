//! Channel selector for a [`Subscription`](crate::Subscription).
//!
//! Per SDD §8.2 a subscriber may pick a specific channel set so it
//! only sees the values it cares about. Phase 0 stores the spec but
//! does not filter — every read returns the full `TickRecord` and the
//! subscriber walks `samples[..n_channels]` itself. Phase 1 enables
//! the filter once the channel registry (`mu_id, channel_idx →
//! channel_id`) is wired into the aligner.

/// What channels a subscription wants to see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelSet {
    /// All populated channels in every record (`samples[..n_channels]`).
    All,
    /// Specific channel IDs (dense `u16` indices per SDD §7.2). Phase 1
    /// will use this list to mask out unwanted entries.
    Specific(Vec<u16>),
}

impl ChannelSet {
    /// Convenience: "everything currently populated."
    pub fn all() -> Self {
        ChannelSet::All
    }

    /// Convenience: "these channel IDs only."
    pub fn specific<I: IntoIterator<Item = u16>>(ids: I) -> Self {
        ChannelSet::Specific(ids.into_iter().collect())
    }

    /// Whether a specific channel ID is included in the set.
    /// Returns `true` for [`ChannelSet::All`].
    pub fn contains(&self, channel_id: u16) -> bool {
        match self {
            ChannelSet::All => true,
            ChannelSet::Specific(ids) => ids.contains(&channel_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_contains_every_id() {
        let s = ChannelSet::all();
        assert!(s.contains(0));
        assert!(s.contains(63));
        assert!(s.contains(u16::MAX));
    }

    #[test]
    fn specific_contains_only_listed_ids() {
        let s = ChannelSet::specific([4, 5, 6]);
        assert!(s.contains(4));
        assert!(s.contains(5));
        assert!(s.contains(6));
        assert!(!s.contains(0));
        assert!(!s.contains(7));
    }

    #[test]
    fn specific_empty_list_matches_nothing() {
        let s = ChannelSet::specific(std::iter::empty());
        assert!(!s.contains(0));
        assert!(!s.contains(u16::MAX));
    }
}
