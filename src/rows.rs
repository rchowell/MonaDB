use crate::{value::Value, VM};
use crate::Result;

// TODO rename rows?
pub struct Rows<'vm> {
    vm: VM<'vm>,
}

impl  Rows<'_> {
    pub fn new(vm: VM) -> Rows {
        Rows { vm }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<Option<Value>> {
        self.vm.next()
    }
}
