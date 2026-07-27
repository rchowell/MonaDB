/// Runtime options that apply to every query on a [`crate::MonaDB`] handle.
#[derive(Debug, Clone, Default)]
pub struct Options {
    debug: bool,
}

impl Options {
    /// Enables bytecode tracing before execution.
    #[must_use]
    pub fn debug(mut self, enabled: bool) -> Self {
        self.debug = enabled;
        self
    }

    /// Enables or disables bytecode tracing in place.
    pub fn set_debug(&mut self, enabled: bool) {
        self.debug = enabled;
    }

    /// Returns whether bytecode tracing is enabled.
    pub fn debug_enabled(&self) -> bool {
        self.debug
    }
}
