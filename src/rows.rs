use crate::{value::Record, VM};
use crate::Result;

pub struct Rows<'vm> {
    vm: VM<'vm>,
}

impl <'vm> Rows<'vm> {
    pub fn new(vm: VM) -> Rows {
        Rows { vm }
    }

    pub fn next(&mut self) -> Result<Option<Record>> {
        self.vm.next()
    }
}
