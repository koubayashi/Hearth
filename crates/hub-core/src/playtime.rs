//! Merging store-reported playtime with sessions Hearth recorded itself.

/// Displayed total for one store entry.
///
/// `store_total` is what the store reports now, `baseline` is what it reported
/// the first time Hearth synced, and `hub_secs` is everything Hearth timed
/// since. Stores that track playtime already include our sessions in their
/// total, so we take the larger figure rather than adding them.
pub fn merged_secs(store_total: Option<i64>, baseline: Option<i64>, hub_secs: i64) -> i64 {
    let ours = baseline.unwrap_or(0) + hub_secs;
    store_total.map_or(ours, |s| s.max(ours))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_store_data_uses_hub_sessions() {
        assert_eq!(merged_secs(None, None, 600), 600);
    }

    #[test]
    fn store_total_includes_our_sessions_so_no_double_count() {
        // Baseline 1h, we timed 30m, store now says 1h30m.
        assert_eq!(merged_secs(Some(5400), Some(3600), 1800), 5400);
    }

    #[test]
    fn store_lagging_behind_falls_back_to_ours() {
        assert_eq!(merged_secs(Some(3600), Some(3600), 1800), 5400);
    }
}
