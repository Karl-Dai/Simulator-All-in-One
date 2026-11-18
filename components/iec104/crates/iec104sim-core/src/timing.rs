//! IEC 60870-5-104 protocol-timing relationship constraints.
//!
//! The standard (§9.6) mandates `t2 < t1`; engineering practice adds
//! `t3 > t1` and `w ≤ ⌊2·k/3⌋`. [`correct_timing`] is the single,
//! orientation-agnostic ("t1/k authoritative") normalization that every
//! backend ingestion point runs so that no configuration can ever take
//! effect while violating these invariants. The frontend mirrors an
//! edit-aware variant for instant feedback, but its output already
//! satisfies the invariants — so this function is a no-op on it.

use serde::{Deserialize, Serialize};

/// The six timing parameters as a single value object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimingParams {
    pub t0: u32,
    pub t1: u32,
    pub t2: u32,
    pub t3: u32,
    pub k: u16,
    pub w: u16,
}

/// One field that [`correct_timing`] changed, for surfacing to the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimingCorrection {
    /// Field name: "t0" | "t1" | "t2" | "t3" | "k" | "w".
    pub field: String,
    pub from: u32,
    pub to: u32,
}

/// Normalize timing parameters so that `t2 < t1 < t3` and `w ≤ ⌊2k/3⌋`,
/// treating `t1`/`k` as the authoritative anchors and clamping the rest.
///
/// `t1` is clamped to `[2,254]` and `k` to `[2,32767]` to leave room for
/// their neighbors (`t2 ≥ 1`, `t3 ≤ 255`, `w ≥ 1`). `t0` is independent
/// and only range-clamped to `[1,255]`.
///
/// Returns the corrected params and the list of fields that changed
/// (empty ⇒ input was already valid).
pub fn correct_timing(input: TimingParams) -> (TimingParams, Vec<TimingCorrection>) {
    let t0 = input.t0.clamp(1, 255);
    let t1 = input.t1.clamp(2, 254);
    let t2 = input.t2.clamp(1, t1 - 1);
    let t3 = input.t3.clamp(t1 + 1, 255);
    let k = input.k.clamp(2, 32767);
    // floor(2k/3); k ≥ 2 ⇒ bound ≥ 1, so the range [1, bound] is valid.
    let w_max = ((2u32 * k as u32) / 3).max(1) as u16;
    let w = input.w.clamp(1, w_max);

    let out = TimingParams { t0, t1, t2, t3, k, w };

    let mut changes = Vec::new();
    if t0 != input.t0 { changes.push(TimingCorrection { field: "t0".into(), from: input.t0, to: t0 }); }
    if t1 != input.t1 { changes.push(TimingCorrection { field: "t1".into(), from: input.t1, to: t1 }); }
    if t2 != input.t2 { changes.push(TimingCorrection { field: "t2".into(), from: input.t2, to: t2 }); }
    if t3 != input.t3 { changes.push(TimingCorrection { field: "t3".into(), from: input.t3, to: t3 }); }
    if k != input.k { changes.push(TimingCorrection { field: "k".into(), from: input.k as u32, to: k as u32 }); }
    if w != input.w { changes.push(TimingCorrection { field: "w".into(), from: input.w as u32, to: w as u32 }); }

    (out, changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(t0: u32, t1: u32, t2: u32, t3: u32, k: u16, w: u16) -> TimingParams {
        TimingParams { t0, t1, t2, t3, k, w }
    }

    #[test]
    fn valid_config_is_noop() {
        let input = p(30, 15, 10, 20, 12, 8); // defaults; 8 == floor(2*12/3)
        let (out, changes) = correct_timing(input);
        assert_eq!(out, input);
        assert!(changes.is_empty());
    }

    #[test]
    fn t2_ge_t1_is_clamped_down() {
        let (out, changes) = correct_timing(p(30, 15, 20, 20, 12, 8));
        assert_eq!(out.t2, 14);
        assert_eq!(out.t1, 15); // anchor untouched
        assert_eq!(changes, vec![TimingCorrection { field: "t2".into(), from: 20, to: 14 }]);
    }

    #[test]
    fn t3_le_t1_is_pushed_up() {
        let (out, changes) = correct_timing(p(30, 15, 10, 8, 12, 8));
        assert_eq!(out.t3, 16);
        assert_eq!(out.t1, 15);
        assert_eq!(changes, vec![TimingCorrection { field: "t3".into(), from: 8, to: 16 }]);
    }

    #[test]
    fn w_over_two_thirds_k_is_clamped() {
        // k=10 ⇒ floor(20/3)=6
        let (out, _) = correct_timing(p(30, 15, 10, 20, 10, 9));
        assert_eq!(out.w, 6);
        assert_eq!(out.k, 10);
    }

    #[test]
    fn import_double_violation() {
        // t2 ≥ t1 and t3 ≤ t1 simultaneously (spec scenario).
        let (out, _) = correct_timing(p(30, 15, 20, 8, 12, 8));
        assert_eq!((out.t1, out.t2, out.t3), (15, 14, 16));
    }

    #[test]
    fn anchor_clamped_at_low_rail() {
        // t1=1 cannot host t2 ≥ 1 → t1 forced to 2, t2 to 1.
        let (out, changes) = correct_timing(p(30, 1, 10, 20, 12, 8));
        assert_eq!(out.t1, 2);
        assert_eq!(out.t2, 1);
        assert!(changes.iter().any(|c| c.field == "t1" && c.to == 2));
        assert!(changes.iter().any(|c| c.field == "t2" && c.to == 1));
    }

    #[test]
    fn anchor_clamped_at_high_rail() {
        // t1=255 cannot host t3 ≤ 255 → t1 forced to 254, t3 to 255.
        let (out, _) = correct_timing(p(30, 255, 10, 20, 12, 8));
        assert_eq!(out.t1, 254);
        assert_eq!(out.t3, 255);
    }

    #[test]
    fn k_low_rail_keeps_w_valid() {
        let (out, _) = correct_timing(p(30, 15, 10, 20, 1, 5));
        assert_eq!(out.k, 2);
        assert_eq!(out.w, 1); // floor(2*2/3)=1
    }

    #[test]
    fn t0_clamped_independently_without_touching_triple() {
        let (out, changes) = correct_timing(p(0, 15, 10, 20, 12, 8));
        assert_eq!(out.t0, 1);
        assert_eq!((out.t1, out.t2, out.t3), (15, 10, 20));
        assert_eq!(changes, vec![TimingCorrection { field: "t0".into(), from: 0, to: 1 }]);
    }

    #[test]
    fn editing_t1_touches_at_most_one_neighbor() {
        // Lower t1 below t2: only t2 moves, t3 stays (no cascade).
        let (out, changes) = correct_timing(p(30, 5, 10, 20, 12, 8));
        assert_eq!(out.t1, 5);
        assert_eq!(out.t2, 4);
        assert_eq!(out.t3, 20);
        assert_eq!(changes.iter().filter(|c| c.field == "t2" || c.field == "t3").count(), 1);
    }

    #[test]
    fn master_config_normalize_timing_corrects_and_reports() {
        let mut cfg = crate::master::MasterConfig {
            t1: 15, t2: 20, t3: 8, ..Default::default()
        };
        let changes = cfg.normalize_timing();
        assert_eq!((cfg.t1, cfg.t2, cfg.t3), (15, 14, 16));
        assert!(changes.iter().any(|c| c.field == "t2"));
        assert!(changes.iter().any(|c| c.field == "t3"));
    }

    #[test]
    fn slave_timing_config_normalize_corrects_and_reports() {
        let mut t = crate::slave::ProtocolTimingConfig { t1: 15, t2: 20, t3: 20, k: 12, w: 8, t0: 30 };
        let changes = t.normalize();
        assert_eq!(t.t2, 14);
        assert_eq!(changes, vec![TimingCorrection { field: "t2".into(), from: 20, to: 14 }]);
    }

    #[test]
    fn slave_timing_config_valid_is_noop() {
        let mut t = crate::slave::ProtocolTimingConfig::default();
        assert!(t.normalize().is_empty());
    }
}
