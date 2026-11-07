use super::{MODULES, TS_DECLARATIONS, TS_DOCUMENTATION};

#[test]
fn ts_declarations_cover_the_api() {
    let declarations = TS_DECLARATIONS;
    assert_eq!(declarations, TS_DOCUMENTATION.join("\n\n") + "\n");
    assert!(!declarations.contains("#[ts"));
}

#[test]
fn no_type_is_in_two_modules() {
    let mut all = TS_DOCUMENTATION.to_vec();
    all.sort_unstable();
    let before = all.len();
    all.dedup();
    assert_eq!(all.len(), before);
}

/// `source` without its `/** ... */` comments.
fn uncommented(source: &str) -> String {
    let mut rest = source;
    let mut out = String::new();
    while let Some(start) = rest.find("/**") {
        out.push_str(&rest[..start]);
        let end = rest[start..].find("*/").expect("a comment ends") + start + 2;
        rest = rest[end..].trim_start_matches(' ').trim_start_matches('\n');
    }
    out + rest
}

#[test]
fn a_brief_declaration_keeps_every_member_and_less_of_its_comments() {
    for module in MODULES {
        let (full, brief) = (module.source(), module.brief_source());
        assert_eq!(uncommented(&brief), uncommented(&full), "{}", module.name);
        assert!(brief.len() <= full.len(), "{}", module.name);
    }
    let core = MODULES.iter().find(|module| module.name == "core").unwrap();
    assert!(core.brief_source().len() < core.source().len());
}
