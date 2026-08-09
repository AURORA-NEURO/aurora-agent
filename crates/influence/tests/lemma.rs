//! The inequality every bound in the crate rests on.
//!
//! If the lemma is wrong, nothing above it is a guarantee, so it is checked against a directly
//! constructed worst case rather than only against its own algebra.

use bioprism_influence::{union_bound, RatioRange};

/// Total variation between a base distribution and its reweighting, computed from first
/// principles so the check does not go through any of the crate's own machinery.
fn reweighted_total_variation(base: &[f64], weights: &[f64]) -> f64 {
    let mass: f64 = base.iter().zip(weights).map(|(p, w)| p * w).sum();
    0.5 * base
        .iter()
        .zip(weights)
        .map(|(p, w)| (p * w / mass - p).abs())
        .sum::<f64>()
}

#[test]
fn the_identity_perturbation_has_a_bound_of_exactly_zero() {
    assert_eq!(RatioRange::identity().total_variation_bound(), 0.0);
    assert_eq!(RatioRange::new(3.5, 3.5).unwrap().total_variation_bound(), 0.0);
}

#[test]
fn a_ratio_range_reaching_zero_gives_the_vacuous_bound_of_one() {
    let range = RatioRange::new(0.0, 4.0).unwrap();
    assert_eq!(range.total_variation_bound(), 1.0);
    assert!(range.spread().is_infinite());
}

#[test]
fn the_bound_never_exceeds_one_however_wide_the_range() {
    for hi in [2.0, 10.0, 1.0e6, 1.0e300] {
        let bound = RatioRange::new(1.0, hi).unwrap().total_variation_bound();
        assert!(
            (0.0..=1.0).contains(&bound),
            "range [1, {hi}] produced {bound}, which is not a total-variation distance"
        );
    }
}

#[test]
fn the_bound_depends_only_on_the_ratio_and_not_on_the_scale() {
    let base = RatioRange::new(0.5, 2.0).unwrap().total_variation_bound();
    for scale in [1.0e-6, 0.1, 7.0, 1.0e6] {
        let scaled = RatioRange::new(0.5 * scale, 2.0 * scale)
            .unwrap()
            .total_variation_bound();
        assert!(
            (scaled - base).abs() < 1e-12,
            "scaling the range by {scale} changed the bound from {base} to {scaled}"
        );
    }
}

#[test]
fn the_bound_is_attained_by_a_two_point_base_distribution() {
    for (lo, hi) in [(1.0, 4.0), (0.5, 2.0), (0.9, 1.1), (1.0, 100.0)] {
        let range = RatioRange::new(lo, hi).unwrap();
        let claimed = range.total_variation_bound();

        // The proof's maximiser: mass q on the high endpoint with q(hi - lo) = sqrt(lo*hi) - lo.
        let q = ((lo * hi).sqrt() - lo) / (hi - lo);
        let attained = reweighted_total_variation(&[1.0 - q, q], &[lo, hi]);
        assert!(
            (attained - claimed).abs() < 1e-12,
            "range [{lo}, {hi}]: the worst case attains {attained} but the lemma claims {claimed}"
        );
    }
}

#[test]
fn no_base_distribution_and_weighting_exceeds_the_bound() {
    let range = RatioRange::new(0.7, 2.3).unwrap();
    let claimed = range.total_variation_bound();
    let mut checked = 0usize;
    for a in 1..20u32 {
        for b in 1..20u32 {
            let base = [a as f64, b as f64];
            let mass = base[0] + base[1];
            let base = [base[0] / mass, base[1] / mass];
            for weights in [[0.7, 2.3], [2.3, 0.7], [0.7, 0.7], [1.4, 2.3], [0.7, 1.9]] {
                let observed = reweighted_total_variation(&base, &weights);
                assert!(
                    observed <= claimed + 1e-12,
                    "base {base:?} with weights {weights:?} moved {observed}, above the claimed {claimed}"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 1_000);
}

#[test]
fn the_naive_ratio_bound_is_never_smaller_than_the_lemma() {
    for (lo, hi) in [(1.0, 1.5), (1.0, 3.0), (0.2, 0.9), (1.0, 50.0)] {
        let lemma = RatioRange::new(lo, hi).unwrap().total_variation_bound();
        let naive = (hi - lo) / (2.0 * lo);
        assert!(
            naive >= lemma - 1e-15,
            "the naive bound {naive} is below the lemma {lemma} on [{lo}, {hi}]"
        );
    }
}

#[test]
fn composing_two_ranges_multiplies_their_spreads() {
    let left = RatioRange::new(0.5, 2.0).unwrap();
    let right = RatioRange::new(0.8, 1.6).unwrap();
    let composed = left.compose(right);
    assert!((composed.spread() - left.spread() * right.spread()).abs() < 1e-12);
    assert!(composed.total_variation_bound() >= left.total_variation_bound());
    assert!(composed.total_variation_bound() >= right.total_variation_bound());
}

#[test]
fn composing_with_the_identity_changes_nothing() {
    let range = RatioRange::new(0.3, 1.7).unwrap();
    let composed = range.compose(RatioRange::identity());
    assert_eq!(composed.lo(), range.lo());
    assert_eq!(composed.hi(), range.hi());
}

#[test]
fn the_multiplicative_rule_beats_the_union_rule_on_equal_spreads() {
    for spread in [1.5f64, 4.0, 25.0] {
        let each = RatioRange::new(1.0, spread).unwrap();
        let multiplicative = each.compose(each).total_variation_bound();
        let additive = union_bound([each.total_variation_bound(), each.total_variation_bound()]);
        assert!(
            multiplicative <= additive + 1e-15,
            "spread {spread}: multiplicative {multiplicative} lost to the union bound {additive}"
        );
    }
}

#[test]
fn the_union_bound_is_capped_at_one() {
    assert_eq!(union_bound([0.9, 0.9, 0.9]), 1.0);
    assert_eq!(union_bound(std::iter::empty()), 0.0);
}

#[test]
fn removal_of_a_constant_factor_has_a_ratio_range_of_one() {
    let range = RatioRange::of_removal(&[0.25, 0.25, 0.25, 0.25]).unwrap();
    assert_eq!(range.spread(), 1.0);
    assert_eq!(range.total_variation_bound(), 0.0);
}

#[test]
fn removal_of_a_factor_with_a_zero_entry_is_vacuous_rather_than_an_error() {
    let range = RatioRange::of_removal(&[0.0, 1.0]).unwrap();
    assert_eq!(range.total_variation_bound(), 1.0);
}

#[test]
fn an_inverted_ratio_range_is_rejected() {
    assert!(RatioRange::new(2.0, 1.0).is_err());
}

#[test]
fn a_non_finite_or_negative_ratio_endpoint_is_rejected() {
    assert!(RatioRange::new(f64::NAN, 1.0).is_err());
    assert!(RatioRange::new(1.0, f64::INFINITY).is_err());
    assert!(RatioRange::new(-0.5, 1.0).is_err());
}
