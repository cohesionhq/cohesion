//! Table formulas: a folder property's JavaScript function computing its
//! value from each row (a document of the folder) ([`PropertyFormula`](cohesion_core::model::PropertyFormula)).
//! A formula runs in a QuickJS runtime of its own with nothing mounted, like
//! a Mermaid diagram: only the language's built-ins, no `cohesion`, network,
//! storage, agent or connector, and no database handle at all. The row goes
//! in as plain values and one value comes out.
//!
//! A [`FormulaRuntime`] keeps the last [`CACHE_SIZE`] formulas it compiled,
//! each in a context of its own so one formula can't see another's globals,
//! keyed by property and exact source, so a changed source is compiled
//! afresh and never served stale.

use std::time::Instant;

use cohesion_core::model::{PropertyDataType, PropertyValue};
use rquickjs::{
    CatchResultExt, CaughtError, Context, Ctx, Function, IntoJs, Object, Persistent, Runtime, Value,
};

use crate::sandbox::{INTERRUPTED, SandboxOptions, caught, strip_types};
use crate::script_error::ScriptError;

/// How many compiled formulas a runtime keeps; the least recently used goes
/// first. Each is a context, tens of kilobytes, under the runtime's memory
/// limit.
pub(crate) const CACHE_SIZE: usize = 32;

/// The longest error a formula leaves, in characters; a longer message is
/// cut short.
const ERROR_LENGTH: usize = 300;

/// A formula to compute: whose it is, its source, and the type its outputs
/// must be.
#[derive(Clone, Debug)]
pub struct Formula {
    pub property_id: i64,
    pub source: String,
    pub data_type: PropertyDataType,
}

/// One row as a formula reads it: each property's name with the row's value
/// for it, or `None` when empty. A choice comes as its option's name, as
/// text; an option's id alone means nothing to a formula.
pub type FormulaRow = Vec<(String, Option<PropertyValue>)>;

/// What a formula gave for one row: a value of its type, `None` for an empty
/// cell, or why it failed, in words for the user.
pub type FormulaOutput = Result<Option<PropertyValue>, String>;

pub(crate) const NOT_A_FUNCTION: &str =
    "A formula must be a function of the row, such as `(row) => row.Price * row.Quantity`.";
pub(crate) const TOO_SLOW: &str = "This formula took too long for a row and was stopped.";
const TOO_LARGE: &str = "This formula ran out of memory.";
pub(crate) const FAILED: &str = "This formula couldn’t be computed.";

/// Why computing a formula for a row failed.
enum Failure {
    /// The formula threw, didn't compile, or returned something else than
    /// its type: that row's cell is empty.
    Row(String),
    /// The runtime stopped it, for time or memory: that row and the rest
    /// of the job fail with this, and the runtime is replaced.
    Stop(&'static str),
}

/// What computing a formula for some rows gave.
pub(crate) struct FormulaRun {
    /// One output per row computed, in order.
    pub(crate) outputs: Vec<FormulaOutput>,
    /// Why the runtime stopped before the remaining rows, which fail with
    /// it; the runtime is no longer used.
    pub(crate) stopped: Option<&'static str>,
}

/// A compiled formula: its function, in the context it was compiled in.
/// The function is dropped before its context.
struct Compiled {
    property_id: i64,
    source: String,
    function: Persistent<Function<'static>>,
    context: Context,
}

/// One runtime and the formulas it compiled, most recently used last. It
/// lives on one worker's thread. The compiled formulas are dropped before
/// the runtime.
pub(crate) struct FormulaRuntime {
    compiled: Vec<Compiled>,
    runtime: Runtime,
    options: SandboxOptions,
}

impl FormulaRuntime {
    pub(crate) fn new(options: SandboxOptions) -> Result<Self, rquickjs::Error> {
        let runtime = Runtime::new()?;
        runtime.set_memory_limit(options.memory_limit_bytes);
        Ok(Self {
            compiled: Vec::new(),
            runtime,
            options,
        })
    }

    /// Computes `formula` for each of `rows`, each call under the time
    /// limit. Stops at a row the runtime stopped, which [`FormulaRun`] says.
    pub(crate) fn evaluate(&mut self, formula: &Formula, rows: &[FormulaRow]) -> FormulaRun {
        match self.compile_cached(formula) {
            Ok(()) => {}
            Err(Failure::Row(error)) => {
                return FormulaRun {
                    outputs: rows.iter().map(|_| Err(error.clone())).collect(),
                    stopped: None,
                };
            }
            Err(Failure::Stop(error)) => {
                return FormulaRun {
                    outputs: Vec::new(),
                    stopped: Some(error),
                };
            }
        }
        let compiled = self.compiled.last().expect("just compiled");
        let mut outputs = Vec::with_capacity(rows.len());
        for row in rows {
            arm_deadline(&self.runtime, self.options);
            let output = compiled.context.with(|ctx| {
                let call = || -> rquickjs::Result<Value<'_>> {
                    let function = compiled.function.clone().restore(&ctx)?;
                    function.call((row_object(&ctx, row)?,))
                };
                match call().catch(&ctx) {
                    Ok(value) => Ok(output(&value, formula.data_type)),
                    Err(error) => Err(failure(&ctx, error)),
                }
            });
            match output {
                Ok(output) => outputs.push(output),
                Err(Failure::Row(error)) => outputs.push(Err(error)),
                Err(Failure::Stop(error)) => {
                    return FormulaRun {
                        outputs,
                        stopped: Some(error),
                    };
                }
            }
        }
        FormulaRun {
            outputs,
            stopped: None,
        }
    }

    /// Makes `formula` the most recently used compiled formula: the cached
    /// one for its property and exact source, or compiled now and cached in
    /// place of the property's earlier source, dropping the least recently
    /// used past [`CACHE_SIZE`]. A source that doesn't compile to a function
    /// isn't cached.
    fn compile_cached(&mut self, formula: &Formula) -> Result<(), Failure> {
        let cached = self.compiled.iter().position(|compiled| {
            compiled.property_id == formula.property_id && compiled.source == formula.source
        });
        let compiled = match cached {
            Some(index) => self.compiled.remove(index),
            None => {
                let compiled = self.compile(formula)?;
                // The property's earlier sources are never asked for again.
                self.compiled
                    .retain(|cached| cached.property_id != formula.property_id);
                compiled
            }
        };
        if self.compiled.len() == CACHE_SIZE {
            self.compiled.remove(0);
        }
        self.compiled.push(compiled);
        Ok(())
    }

    fn compile(&self, formula: &Formula) -> Result<Compiled, Failure> {
        let code = strip_types(formula.source.clone()).map_err(|error| {
            Failure::Row(bounded(match error {
                ScriptError::Exception { message, .. } => message,
                other => other.to_string(),
            }))
        })?;
        let context = Context::full(&self.runtime).map_err(|_| Failure::Stop(TOO_LARGE))?;
        arm_deadline(&self.runtime, self.options);
        let function = context.with(|ctx| {
            let value: Value = ctx.eval(code).catch(&ctx).map_err(|e| failure(&ctx, e))?;
            let function = value
                .into_function()
                .ok_or_else(|| Failure::Row(NOT_A_FUNCTION.to_string()))?;
            Ok(Persistent::save(&ctx, function))
        })?;
        Ok(Compiled {
            property_id: formula.property_id,
            source: formula.source.clone(),
            function,
            context,
        })
    }

    /// How many compiled formulas this runtime holds.
    #[cfg(test)]
    pub(crate) fn cached(&self) -> usize {
        self.compiled.len()
    }
}

/// Starts the time limit for the next call.
fn arm_deadline(runtime: &Runtime, options: SandboxOptions) {
    let deadline = Instant::now() + options.time_limit;
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline)));
}

/// `row` as a plain object, each value as a formula reads it: a string, a
/// number, a boolean, or `null`.
pub(crate) fn row_object<'js>(ctx: &Ctx<'js>, row: &FormulaRow) -> rquickjs::Result<Object<'js>> {
    let object = Object::new(ctx.clone())?;
    for (name, value) in row {
        let value = match value {
            None => Value::new_null(ctx.clone()),
            Some(PropertyValue::Text(text) | PropertyValue::Date(text)) => {
                text.as_str().into_js(ctx)?
            }
            Some(PropertyValue::Integer(number)) => (*number as f64).into_js(ctx)?,
            Some(PropertyValue::Decimal(number) | PropertyValue::Currency(number)) => {
                number.into_js(ctx)?
            }
            Some(PropertyValue::Checkbox(checked)) => checked.into_js(ctx)?,
            // Rows carry a choice as its name; an id alone reads as empty.
            Some(PropertyValue::Choice(_)) => Value::new_null(ctx.clone()),
        };
        object.set(name.as_str(), value)?;
    }
    Ok(object)
}

/// The largest whole number a JavaScript number holds exactly.
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// What a formula's function returned, read as a value of `data_type`:
/// `null` and `undefined` are empty, and anything else must be of the type
/// and [checked](PropertyValue::checked).
fn output(value: &Value<'_>, data_type: PropertyDataType) -> FormulaOutput {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    let read = typed_value(value, data_type).ok_or_else(|| expected(data_type).to_string())?;
    read.checked().map_err(str::to_string)
}

/// `value`, which isn't `null` or `undefined`, as a value of `data_type`
/// before it is [checked](PropertyValue::checked): a string for text and
/// dates, a whole number for an integer, a number for a decimal or an
/// amount, a boolean for a checkbox. `None` when it is of another type, and
/// always for a choice, whose options aren't known here.
pub(crate) fn typed_value(value: &Value<'_>, data_type: PropertyDataType) -> Option<PropertyValue> {
    let text = || value.as_string().and_then(|text| text.to_string().ok());
    match data_type {
        PropertyDataType::Text => text().map(PropertyValue::Text),
        PropertyDataType::Date => text().map(PropertyValue::Date),
        PropertyDataType::Integer => value
            .as_number()
            .filter(|number| number.fract() == 0.0 && number.abs() <= MAX_SAFE_INTEGER)
            .map(|number| PropertyValue::Integer(number as i64)),
        PropertyDataType::Decimal => value.as_number().map(PropertyValue::Decimal),
        PropertyDataType::Currency => value.as_number().map(PropertyValue::Currency),
        PropertyDataType::Checkbox => value.as_bool().map(PropertyValue::Checkbox),
        PropertyDataType::Choice => None,
    }
}

/// What a formula of `data_type` must return, for a formula that returned
/// something else.
fn expected(data_type: PropertyDataType) -> &'static str {
    match data_type {
        PropertyDataType::Text => "This formula must return text.",
        PropertyDataType::Integer => "This formula must return a whole number.",
        PropertyDataType::Decimal => "This formula must return a number.",
        PropertyDataType::Currency => "This formula must return an amount, as a number.",
        PropertyDataType::Checkbox => "This formula must return true or false.",
        PropertyDataType::Date => "This formula must return a date, as YYYY-MM-DD.",
        PropertyDataType::Choice => "A formula can’t be a choice.",
    }
}

/// What a throw means: the runtime stopping the formula, for time or memory
/// (out of memory, QuickJS can throw `null`, having no room left for an
/// error), or the formula's own failure, with its message.
fn failure<'js>(ctx: &Ctx<'js>, error: CaughtError<'js>) -> Failure {
    match &error {
        CaughtError::Value(value) if value.is_null() => return Failure::Stop(TOO_LARGE),
        CaughtError::Exception(exception) => {
            let name = exception.get::<_, String>("name").unwrap_or_default();
            let message = exception.message().unwrap_or_default();
            match (name.as_str(), message.as_str()) {
                thrown if thrown == INTERRUPTED => return Failure::Stop(TOO_SLOW),
                ("InternalError", "out of memory") => return Failure::Stop(TOO_LARGE),
                _ => {}
            }
        }
        _ => {}
    }
    Failure::Row(bounded(match caught(ctx, error) {
        ScriptError::Exception { message, .. } => message,
        _ => FAILED.to_string(),
    }))
}

/// `message`, cut to [`ERROR_LENGTH`] characters.
fn bounded(message: String) -> String {
    match message.char_indices().nth(ERROR_LENGTH) {
        Some((end, _)) => format!("{}…", &message[..end]),
        None => message,
    }
}

#[cfg(test)]
mod tests;
