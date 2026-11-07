use cohesion_core::model::{InstalledService, InstalledServiceFunction};

use super::service_declarations;

fn function(name: &str, args: Option<&str>, read_only: bool) -> InstalledServiceFunction {
    InstalledServiceFunction {
        name: name.into(),
        description: format!("Does {name}."),
        args: args.map(str::to_string),
        returns: Some("{ count: number }".into()),
        read_only,
    }
}

fn service(identifier: &str, functions: Vec<InstalledServiceFunction>) -> InstalledService {
    InstalledService {
        extension_id: 1,
        script_identifier: identifier.into(),
        name: identifier.into(),
        description: format!("The {identifier} service."),
        functions,
    }
}

#[test]
fn every_function_is_declared_and_writes_are_marked() {
    let services = [
        service(
            "counter",
            vec![
                function("increment", Some("{ by?: number }"), false),
                function("read", None, true),
            ],
        ),
        service("mailer", vec![function("send", Some("string"), false)]),
    ];

    let rendered = service_declarations(&services);
    assert_eq!(rendered.matches("interface ServiceMap {").count(), 1);
    assert!(rendered.contains("\"counter\": {"));
    assert!(rendered.contains("\"mailer\": {"));
    assert!(rendered.contains("Does increment. Not available in read-only contexts."));
    assert!(rendered.contains("Does send. Not available in read-only contexts."));
    assert!(rendered.contains("/** Does read. */"));
    assert!(rendered.contains("read(): { count: number };"));
    assert!(service_declarations(&[]).is_empty());
}

#[test]
fn extension_text_cannot_escape_its_slot() {
    let escapes = [
        "number): void; declare const leaked: any; function f(x: number",
        "{ a: number } } declare const leaked: any; namespace x {",
        "number, leaked: any",
        "number /* leaked",
        "number // leaked",
        "\"unclosed leaked",
        "Array<number>> leaked",
    ];
    for text in escapes {
        let mut evil = service(
            "evil\": any; leaked: any; \"x",
            vec![function("f", Some(text), false)],
        );
        evil.description = "Closes */ declare const leaked: any; /*".into();
        let rendered = service_declarations(&[evil]);
        assert!(rendered.contains("f(args: unknown)"), "{text}");
        assert!(!rendered.contains("*/ declare"), "{text}");
        assert!(rendered.contains(r#""evil\": any; leaked: any; \"x": {"#));
    }

    let fine = [
        "{ by?: number; label: string }",
        "Record<string, number[]>",
        "(count: number) => void",
        "\"a;b\" | 'c}'",
    ];
    for text in fine {
        let rendered =
            service_declarations(&[service("fine", vec![function("f", Some(text), false)])]);
        assert!(rendered.contains(&format!("f(args: {text})")), "{text}");
    }
}
