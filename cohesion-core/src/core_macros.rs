//! Every `macro_rules!` of this crate. `#[macro_export]` puts them at the
//! crate root, as `cohesion_core::log_info!` and friends.

/// `log_debug!(Module::X, "fmt", args...)`: logs at Debug through the global logger.
#[macro_export]
macro_rules! log_debug {
    ($module:expr, $($arg:tt)+) => {
        $crate::log::debug($module, ::std::format_args!($($arg)+))
    };
}

/// `log_info!(Module::X, "fmt", args...)`: logs at Info through the global logger.
#[macro_export]
macro_rules! log_info {
    ($module:expr, $($arg:tt)+) => {
        $crate::log::info($module, ::std::format_args!($($arg)+))
    };
}

/// `log_warn!(Module::X, "fmt", args...)`: logs at Warn through the global logger.
#[macro_export]
macro_rules! log_warn {
    ($module:expr, $($arg:tt)+) => {
        $crate::log::warn($module, ::std::format_args!($($arg)+))
    };
}

/// `log_error!(Module::X, "fmt", args...)`: logs at Error through the global logger.
#[macro_export]
macro_rules! log_error {
    ($module:expr, $($arg:tt)+) => {
        $crate::log::error($module, ::std::format_args!($($arg)+))
    };
}
