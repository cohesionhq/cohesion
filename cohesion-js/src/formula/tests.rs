use std::time::Duration;

use cohesion_core::model::{PropertyDataType, PropertyValue};

use super::{
    CACHE_SIZE, Formula, FormulaOutput, FormulaRow, FormulaRuntime, NOT_A_FUNCTION, TOO_SLOW,
};
use crate::sandbox::SandboxOptions;

use PropertyDataType as T;
use PropertyValue as V;

fn runtime() -> FormulaRuntime {
    FormulaRuntime::new(SandboxOptions::FORMULA).unwrap()
}

fn formula(property_id: i64, data_type: PropertyDataType, source: &str) -> Formula {
    Formula {
        property_id,
        source: source.to_string(),
        data_type,
    }
}

/// A row of `Pages` and `Title`.
fn book(pages: Option<i64>, title: &str) -> FormulaRow {
    vec![
        ("Pages".to_string(), pages.map(V::Integer)),
        ("Title".to_string(), Some(V::Text(title.to_string()))),
    ]
}

/// What `source`, a formula of `data_type`, gives for one book.
fn one(data_type: PropertyDataType, source: &str) -> FormulaOutput {
    let run = runtime().evaluate(&formula(1, data_type, source), &[book(Some(412), "Dune")]);
    assert!(run.stopped.is_none());
    run.outputs.into_iter().next().unwrap()
}

#[test]
fn a_formula_reads_the_row_by_name_and_returns_its_type() {
    let cases = [
        (T::Integer, "(row) => row.Pages * 2", Some(V::Integer(824))),
        (
            T::Text,
            "(row) => row.Title.toUpperCase()",
            Some(V::Text("DUNE".into())),
        ),
        (T::Decimal, "(row) => row.Pages / 8", Some(V::Decimal(51.5))),
        (T::Currency, "(row) => 9.99", Some(V::Currency(9.99))),
        (
            T::Checkbox,
            "(row) => row.Pages > 400",
            Some(V::Checkbox(true)),
        ),
        (
            T::Date,
            "(row) => '2026-11-09'",
            Some(V::Date("2026-11-09".into())),
        ),
        (T::Integer, "(row) => null", None),
        (T::Integer, "(row) => undefined", None),
        (T::Text, "(row) => ''", None),
        // TypeScript is stripped first, as a cue's is.
        (
            T::Integer,
            "(row: { Pages: number }): number => row.Pages",
            Some(V::Integer(412)),
        ),
    ];
    for (data_type, source, expected) in cases {
        assert_eq!(one(data_type, source), Ok(expected), "{source}");
    }
}

#[test]
fn an_empty_value_reads_as_null() {
    let run = runtime().evaluate(
        &formula(1, T::Checkbox, "(row) => row.Pages === null"),
        &[book(None, "Dune")],
    );
    assert_eq!(run.outputs, vec![Ok(Some(V::Checkbox(true)))]);
}

#[test]
fn a_value_of_another_type_fails_its_row() {
    for (data_type, source) in [
        (T::Integer, "(row) => 3.5"),
        (T::Integer, "(row) => '3'"),
        (T::Text, "(row) => 42"),
        (T::Decimal, "(row) => NaN"),
        (T::Checkbox, "(row) => 1"),
        (T::Date, "(row) => '2026-02-30'"),
        (T::Currency, "(row) => ({ amount: 1 })"),
    ] {
        assert!(one(data_type, source).is_err(), "{source}");
    }
}

#[test]
fn a_throw_fails_only_its_row() {
    let run = runtime().evaluate(
        &formula(
            1,
            T::Integer,
            "(row) => { if (row.Pages === 2) throw new TypeError('two'); return row.Pages }",
        ),
        &[book(Some(1), "a"), book(Some(2), "b"), book(Some(3), "c")],
    );
    assert!(run.stopped.is_none());
    assert_eq!(run.outputs[0], Ok(Some(V::Integer(1))));
    assert!(run.outputs[1].as_ref().unwrap_err().contains("two"));
    assert_eq!(run.outputs[2], Ok(Some(V::Integer(3))));
}

#[test]
fn a_source_that_isnt_a_function_fails_every_row() {
    for source in ["42", "(row) => {", "row.Pages"] {
        let run = runtime().evaluate(
            &formula(1, T::Integer, source),
            &[book(Some(1), "a"), book(Some(2), "b")],
        );
        assert!(run.stopped.is_none());
        assert_eq!(run.outputs.len(), 2);
        assert!(run.outputs.iter().all(Result::is_err), "{source}");
    }
    let run = runtime().evaluate(&formula(1, T::Integer, "42"), &[book(Some(1), "a")]);
    assert_eq!(run.outputs, vec![Err(NOT_A_FUNCTION.to_string())]);
}

#[test]
fn a_formula_reaches_nothing_but_the_language() {
    let source = "(row) => [typeof cohesion, typeof network, typeof fetch, typeof storage, \
                  typeof agent, typeof connectors, typeof services, typeof notifications, \
                  typeof intelligence, typeof require, typeof console].join(',')";
    let undefined = ["undefined"; 11].join(",");
    assert_eq!(one(T::Text, source), Ok(Some(V::Text(undefined))));
}

#[test]
fn formulas_dont_share_globals() {
    let mut runtime = runtime();
    let leaking = "(() => { globalThis.leak = 1; return (row) => 1 })()";
    runtime.evaluate(&formula(1, T::Integer, leaking), &[book(None, "a")]);
    let run = runtime.evaluate(
        &formula(2, T::Text, "(row) => typeof leak"),
        &[book(None, "a")],
    );
    assert_eq!(run.outputs, vec![Ok(Some(V::Text("undefined".into())))]);
}

/// A formula counting its calls, so a recompiled one starts again at 1.
const COUNTER: &str = "(() => { let calls = 0; return (row) => ++calls })()";

fn counts(runtime: &mut FormulaRuntime, property_id: i64, source: &str, rows: usize) -> Vec<i64> {
    let rows = vec![book(None, "a"); rows];
    runtime
        .evaluate(&formula(property_id, T::Integer, source), &rows)
        .outputs
        .into_iter()
        .map(|output| match output {
            Ok(Some(V::Integer(n))) => n,
            other => panic!("{other:?}"),
        })
        .collect()
}

#[test]
fn a_formula_is_compiled_once_for_every_row_and_the_next_job() {
    let mut runtime = runtime();
    assert_eq!(counts(&mut runtime, 1, COUNTER, 3), vec![1, 2, 3]);
    assert_eq!(counts(&mut runtime, 1, COUNTER, 1), vec![4]);
}

#[test]
fn a_changed_source_is_compiled_afresh_in_place_of_the_old() {
    let mut runtime = runtime();
    counts(&mut runtime, 1, COUNTER, 2);
    let changed = format!("{COUNTER} ");
    assert_eq!(counts(&mut runtime, 1, &changed, 1), vec![1]);
    assert_eq!(runtime.cached(), 1);
    let run = runtime.evaluate(&formula(1, T::Integer, "(row) => 7"), &[book(None, "a")]);
    assert_eq!(run.outputs, vec![Ok(Some(V::Integer(7)))]);
    // The same source of another property is its own formula.
    assert_eq!(counts(&mut runtime, 2, COUNTER, 1), vec![1]);
}

#[test]
fn the_cache_holds_a_bounded_number_dropping_the_least_recently_used() {
    let mut runtime = runtime();
    for property_id in 0..=CACHE_SIZE as i64 {
        counts(&mut runtime, property_id, COUNTER, 1);
    }
    assert_eq!(runtime.cached(), CACHE_SIZE);
    // Property 0 was dropped, so it starts again; the last one was kept.
    assert_eq!(counts(&mut runtime, CACHE_SIZE as i64, COUNTER, 1), vec![2]);
    assert_eq!(counts(&mut runtime, 0, COUNTER, 1), vec![1]);
}

#[test]
fn a_formula_past_its_time_limit_stops_the_runtime() {
    let options = SandboxOptions {
        time_limit: Duration::from_millis(50),
        ..SandboxOptions::FORMULA
    };
    let mut runtime = FormulaRuntime::new(options).unwrap();
    let run = runtime.evaluate(
        &formula(
            1,
            T::Integer,
            "(row) => { if (row.Pages === 2) { while (true) {} } return row.Pages }",
        ),
        &[book(Some(1), "a"), book(Some(2), "b"), book(Some(3), "c")],
    );
    assert_eq!(run.outputs, vec![Ok(Some(V::Integer(1)))]);
    assert_eq!(run.stopped, Some(TOO_SLOW));
}

#[test]
fn a_formula_past_its_memory_limit_stops_the_runtime() {
    let run = runtime().evaluate(
        &formula(
            1,
            T::Integer,
            "(row) => { const kept = []; while (true) kept.push(new Array(100000).fill(1)) }",
        ),
        &[book(Some(1), "a"), book(Some(2), "b")],
    );
    assert!(run.outputs.is_empty());
    assert_eq!(run.stopped, Some(super::TOO_LARGE));
}
