use crate::{value::Value, VM};
use crate::Result;

// TODO rename rows?
pub struct Rows<'vm> {
    vm: VM<'vm>,
}

impl <'vm> Rows<'vm> {
    pub fn new(vm: VM) -> Rows {
        Rows { vm }
    }

    pub fn next(&mut self) -> Result<Option<Value>> {
        self.vm.next()
    }
}
