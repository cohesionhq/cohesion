use std::time::Duration;

use cohesion_core::model::{DatabaseDataType, DatabaseValue};

use super::{FormulaPool, ROWS_PER_JOB};
use crate::formula::{Formula, FormulaOutput, FormulaRow, TOO_SLOW};
use crate::sandbox::SandboxOptions;

fn pool() -> FormulaPool {
    FormulaPool::with_options(
        1,
        SandboxOptions {
            time_limit: Duration::from_millis(50),
            ..SandboxOptions::FORMULA
        },
    )
}

fn compute(pool: &FormulaPool, source: &str, rows: Vec<FormulaRow>) -> Vec<FormulaOutput> {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(pool.evaluate(
            Formula {
                property_id: 1,
                source: source.to_string(),
                data_type: DatabaseDataType::Integer,
            },
            rows,
        ))
}

fn numbered(count: usize) -> Vec<FormulaRow> {
    (0..count)
        .map(|n| vec![("N".to_string(), Some(DatabaseValue::Integer(n as i64)))])
        .collect()
}

#[test]
fn every_row_gets_its_output_across_jobs() {
    let pool = pool();
    let count = ROWS_PER_JOB * 2 + 3;
    let outputs = compute(&pool, "(row) => row.N + 1", numbered(count));
    let expected: Vec<FormulaOutput> = (0..count)
        .map(|n| Ok(Some(DatabaseValue::Integer(n as i64 + 1))))
        .collect();
    assert_eq!(outputs, expected);
}

#[test]
fn a_formula_that_never_returns_fails_the_rest_of_the_pass_and_the_pool_goes_on() {
    let pool = pool();
    let count = ROWS_PER_JOB * 3;
    let outputs = compute(
        &pool,
        "(row) => { if (row.N === 3) { while (true) {} } return row.N }",
        numbered(count),
    );
    assert_eq!(outputs.len(), count);
    assert_eq!(outputs[2], Ok(Some(DatabaseValue::Integer(2))));
    assert!(
        outputs[3..]
            .iter()
            .all(|output| output == &Err(TOO_SLOW.to_string()))
    );
    assert_eq!(
        compute(&pool, "(row) => row.N", numbered(1)),
        vec![Ok(Some(DatabaseValue::Integer(0)))]
    );
}
