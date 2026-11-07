use clap::CommandFactory;

use super::*;

#[test]
fn command_tree_is_consistent() {
    CohesionArgs::command().debug_assert();
}
