use cohesion_js::script_api::{Load, MODULES};

use super::declarations;

// A module loaded on demand comes brief unless asked for in full, since
// every load is written to the thread's cache; one the code tool already
// declares in brief always comes whole, or asking for it would repeat it.
#[test]
fn only_an_on_demand_module_comes_brief() {
    let actors = MODULES.iter().find(|m| m.name == "actors").unwrap();
    assert!(actors.load == Load::OnDemand);
    assert_eq!(declarations(actors, false), actors.brief_source());
    assert_eq!(declarations(actors, true), actors.source());
    assert!(actors.brief_source().len() < actors.source().len());

    let core = MODULES.iter().find(|m| m.name == "core").unwrap();
    assert!(core.load == Load::Upfront);
    assert_eq!(declarations(core, false), core.source());
}
