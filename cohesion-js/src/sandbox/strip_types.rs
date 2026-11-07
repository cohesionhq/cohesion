//! Erases TypeScript annotations so QuickJS can run the script. Types are
//! replaced with whitespace rather than reprinted, so line and column numbers
//! in exceptions still point into the source as written. Plain JavaScript
//! passes through unchanged.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use swc_common::SourceMap;
use swc_common::errors::{EmitterWriter, HANDLER, Handler};
use swc_common::sync::Lrc;
use swc_ts_fast_strip::{Options, operate};

use crate::script_error::ScriptError;

/// Strips types from `source`, parsed as a script (not a module) to match
/// how the sandbox evaluates it. Syntax that has runtime behaviour, such as
/// `enum`, a `namespace` with values or constructor parameter properties,
/// is rejected rather than compiled.
pub(crate) fn strip_types(source: String) -> Result<String, ScriptError> {
    let source_map: Lrc<SourceMap> = Default::default();
    let diagnostics = Diagnostics::default();
    let handler = Handler::with_emitter(
        false,
        false,
        Box::new(EmitterWriter::new(
            Box::new(diagnostics.clone()),
            Some(source_map.clone()),
            true,
            false,
        )),
    );
    let options = Options {
        module: Some(false),
        ..Default::default()
    };

    HANDLER
        .set(&handler, || operate(&source_map, &handler, source, options))
        .map(|output| output.code)
        .map_err(|error| {
            let diagnostics = diagnostics.take();
            ScriptError::Exception {
                message: if diagnostics.is_empty() {
                    error.message
                } else {
                    diagnostics
                },
                stack: None,
            }
        })
}

/// Collects the handler's rendered diagnostics.
#[derive(Clone, Default)]
struct Diagnostics(Arc<Mutex<Vec<u8>>>);

impl Diagnostics {
    fn take(&self) -> String {
        let bytes = std::mem::take(&mut *self.0.lock().expect("mutex is not poisoned"));
        String::from_utf8_lossy(&bytes).trim().to_owned()
    }
}

impl Write for Diagnostics {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .expect("mutex is not poisoned")
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
