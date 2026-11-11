use super::strip_types;
use crate::script_error::ScriptError;

/// Strips `source`, checking that nothing moved: the output is as long as
/// the source, and differs from it only by blanks and the `;` or `)` that
/// stripping writes.
fn strip(source: &str) -> String {
    let stripped = strip_types(source.to_string()).unwrap_or_else(|error| {
        panic!("{source}\nwas refused: {error}");
    });
    let before: Vec<u16> = source.encode_utf16().collect();
    let after: Vec<u16> = stripped.encode_utf16().collect();
    assert_eq!(before.len(), after.len(), "{source}\nchanged length");
    for (index, (before, after)) in before.iter().zip(&after).enumerate() {
        let written = [' ', ';', ')'].map(|character| character as u16);
        assert!(
            before == after || written.contains(after),
            "{source}\nmoved at {index}:\n{stripped}"
        );
    }
    stripped
}

/// `text` with each run of whitespace as one space, so a case can say what
/// is left without counting blanks.
fn collapsed(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn refused(source: &str) -> bool {
    matches!(
        strip_types(source.to_string()),
        Err(ScriptError::Exception { .. })
    )
}

#[test]
fn types_are_blanked_in_place() {
    assert_eq!(
        strip("let a: number = 1;\nconst b = (c as string)!;"),
        "let a         = 1;\nconst b = (c          ) ;"
    );
    // Tabs and line breaks inside a type stay, and a character outside the
    // BMP is two columns wide.
    assert_eq!(
        strip("let a: {\n\tb: \"😀\"\n} = c;"),
        "let a   \n\t       \n  = c;"
    );
}

#[test]
fn what_is_left_is_the_javascript() {
    let cases = [
        (
            "let a: number = 1, b!: string, [c, d]: number[] = e;",
            "let a = 1, b , [c, d] = e;",
        ),
        (
            "function f<T extends object>(this: Window, a: T, b?: number, ...c: string[]): \
             asserts a is T {}",
            "function f ( a , b , ...c ) {}",
        ),
        (
            "const f = async <T,>(a: T, { b }: { b?: string } = {}): Promise<T> => a;",
            "const f = async (a , { b } = {}) => a;",
        ),
        (
            "const a = b as unknown as string[], c = d satisfies E, f = g!.h![0]!;",
            "const a = b , c = d , f = g .h [0] ;",
        ),
        (
            "const a = f<string>(b), c = new Map<string, Array<number>>(), d = <any>e;",
            "const a = f (b), c = new Map (), d = e;",
        ),
        (
            "class A<T> extends B<T> implements C, D {\n\
             private readonly a: number = 1;\n\
             static b?: string;\n\
             declare c: number;\n\
             [key: string]: unknown;\n\
             m(): void;\n\
             m(a?: number): void {}\n\
             abstract n(): void;\n\
             }",
            "class A extends B { a = 1; static b ; ; ; ; m(a ) {} ; }",
        ),
        (
            "type A = { a: string };\n\
             interface B extends A { b(): void }\n\
             declare const c: number;\n\
             declare function d(): void;\n\
             declare namespace D { const a: number }\n\
             function e(a: string): void;\n\
             function e(a: unknown) {}\n\
             namespace N { export type T = number }",
            "; ; ; ; ; ; function e(a ) {} ;",
        ),
        (
            "try {} catch (e: unknown) {}\n\
             for (const [k, v] of a as [string, number][]) {}",
            "try {} catch (e ) {} for (const [k, v] of a ) {}",
        ),
        (
            "type T = `a${string}`;\nconst s = `a${b as string}${`c${d!}`}`;",
            "; const s = `a${b }${`c${d }`}`;",
        ),
        (
            "type F = <T>(a: T) => T extends string ? { [K in keyof T]?: T[K] } : [a: T, b?: T];\n\
             let f: F;",
            "; let f ;",
        ),
        (
            "const o = { m<T>(a: T): T { return a; }, get g(): number { return 1; } };",
            "const o = { m (a ) { return a; }, get g() { return 1; } };",
        ),
    ];
    for (source, expected) in cases {
        assert_eq!(collapsed(&strip(source)), expected, "{source}");
    }
}

#[test]
fn javascript_passes_through_unchanged() {
    let sources = [
        // Comparisons that look like type arguments.
        "const a = b < c, d = e > (f); if (a < b && c > (d)) {}",
        "let a = b < c >= d, e = f < g >> h, i = j < k > l;",
        "for (let i = 0; i < n; i++) { x = i << 2 >> 1 >>> 0; }",
        // Words TypeScript reserves only in context.
        "let type = 1; type = 2; type\nfoo;",
        "let declare = 1, namespace = 2, abstract = 3, as = 4, async = 5; if (as) as;",
        "declare\nfoo;\nasync(x);\nx = async + 1;",
        "class A { static = 1; get = 2; set() {} declare() {} public() {} readonly = 3; }",
        // A colon that is no type annotation.
        "label: for (;;) { break label; }",
        "const a = b ? (c) : d => e, f = { g: h ? i : j, k: (l) => l };",
        "switch (a) { case b ? c : d: break; default: e; }",
        // A `!` that is no assertion.
        "const a = b != c, d = e !== f;\na\n!b",
        // Slashes.
        "const a = b / c / d, e = /[/\\]]+/g.test(f) ? g / 2 : `h${i / j}`;",
        "if (a) /b/.exec(c);\nconst d = e++ / 2;",
        "const a = 0x1Fn + 1_000 + 1e-5 + .5 + 1..toString().length;",
        "const a = 'it\\'s' + \"b\\\"c\" + `d\\`e${\"}\"}`;",
        "const { a, b: { c = 1 }, ...d } = e, [f, , g = 2, ...h] = i;",
        "async function* a() { for await (const b of c) yield* b; yield; }",
        "#!/usr/bin/env cohesion\nconst a = 1;",
    ];
    for source in sources {
        assert_eq!(strip(source), source);
    }
}

#[test]
fn an_arrow_function_is_told_from_parentheses() {
    assert_eq!(
        collapsed(&strip("const a = b ? (c): d => e : f;")),
        "const a = b ? (c) => e : f;"
    );
    assert_eq!(
        collapsed(&strip(
            "const a = <T>(b: T) => b, c = (d, e) => d, f = (g = (h: H) => h) => g;"
        )),
        "const a = (b ) => b, c = (d, e) => d, f = (g = (h ) => h) => g;"
    );
}

#[test]
fn erasing_a_statement_does_not_join_its_neighbours() {
    assert_eq!(
        strip("const a = b\ntype T = number\n(c)"),
        "const a = b\n;              \n(c)"
    );
}

#[test]
fn a_return_type_on_its_own_line_keeps_the_arrow_on_the_parenthesis_line() {
    assert_eq!(
        strip("const a = (b: number):\n  number => b;"),
        "const a = (b          \n       ) => b;"
    );
}

#[test]
fn typescript_that_needs_compiling_is_refused() {
    assert!(refused("enum E { A }"));
    assert!(refused("const enum E { A }"));
    assert!(refused("namespace N { export const a = 1; }"));
    assert!(refused("class A { constructor(private a: number) {} }"));
    // A declaration has nothing to compile.
    assert_eq!(
        collapsed(&strip("declare enum E { A }\nlet a: E;")),
        "; let a ;"
    );
}

#[test]
fn source_the_parser_cannot_follow_is_left_for_quickjs() {
    for source in [
        "let a = ;",
        "const a = (b: number;",
        "a = `b${c",
        "@d class E {}",
    ] {
        assert_eq!(strip(source), source);
    }
    // What was stripped before that point still is.
    assert_eq!(strip("let a: A = 1; let b = ;"), "let a    = 1; let b = ;");
}

#[test]
fn deep_nesting_is_left_for_quickjs() {
    let nested =
        |open: &str, close: &str| format!("a = {}1{}", open.repeat(50_000), close.repeat(50_000));
    for source in [
        nested("[", "]"),
        nested("(b = ", ")"),
        nested("async(b = ", ")"),
        nested("<T>(b = ", ")"),
        nested("b < (", ")"),
        nested("`${", "}`"),
        nested("{ a: ", "}"),
    ] {
        assert_eq!(strip_types(source.clone()).ok(), Some(source));
    }
}
