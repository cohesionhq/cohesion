use super::*;

#[test]
fn a_run_at_the_references_scores_half() {
    for difficulty in Difficulty::ALL {
        let reference = reference(difficulty);
        let score = run_efficiency(
            difficulty,
            reference.tokens,
            reference.model_calls,
            reference.seconds,
        );
        assert!((score - 50.0).abs() < 1e-9, "{difficulty:?}: {score}");
    }
}

#[test]
fn using_less_scores_higher() {
    let lean = run_efficiency(Difficulty::Easy, 30_000.0, 2.0, 10.0);
    let heavy = run_efficiency(Difficulty::Easy, 90_000.0, 6.0, 40.0);
    assert!(
        lean > heavy && lean < 100.0 && heavy > 0.0,
        "{lean} {heavy}"
    );
    // The same work counts as leaner on a harder case.
    assert!(run_efficiency(Difficulty::Hard, 90_000.0, 6.0, 40.0) > heavy);
}

#[test]
fn overall_rises_with_accuracy_and_efficiency() {
    assert_eq!(overall(100.0, 100.0), 100.0);
    assert_eq!(overall(0.0, 80.0), 0.0);
    assert!(overall(80.0, 60.0) > overall(70.0, 60.0));
    assert!(overall(80.0, 60.0) > overall(80.0, 50.0));
    // Accuracy counts for more than efficiency.
    assert!(overall(90.0, 50.0) > overall(50.0, 90.0));
}
