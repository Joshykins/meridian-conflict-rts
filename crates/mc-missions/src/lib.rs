//! Campaign missions compile Rust async sequences to explicit, serialisable
//! trigger programs. Await records a dependency; the simulation executes the
//! program, so neither a Rust future nor hidden local state belongs in a save.
mod script;
mod vela;
pub use vela::definition;

#[cfg(test)]
mod tests;
