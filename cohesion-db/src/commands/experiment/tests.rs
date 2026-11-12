use crate::test_support::context;
use crate::{ListExperimentsCommand, SetExperimentCommand};

#[test]
fn no_experiments_are_offered_yet() {
    let db = context();
    assert!(
        db.send_blocking(ListExperimentsCommand {})
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_key_no_experiment_has_is_refused() {
    let db = context();
    let refused = db.send_blocking(SetExperimentCommand {
        key: "gone".to_string(),
        enabled: true,
    });
    assert!(refused.is_err());
}
