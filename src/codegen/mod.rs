use crate::parser::Program;

pub mod zig;

pub trait Backend {
    fn generate(&mut self, program: &Program) -> String;
}

pub fn generate(program: &Program) -> String {
    zig::ZigBackend::new().generate(program)
}
