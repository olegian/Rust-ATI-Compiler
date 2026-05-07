//! Provides rustc callback implementations, which specify how
//! each of the two compilations done by DATIR function.
//!
//! DATIR relies on two compilation passes:
//! 1. A "Gather" pass, which collects information from the HIR/MIR.
//! 2. An "Instrument" pass, which transforms the AST, using the gathered information
//!    to insert appropriate instrumentation.
//! 
//! At the end of the instrument pass, the runtime libary is injected, shim functions are defined,
//! and traits are implemented for user-defined types. This is governed by the codegen module!

mod codegen;
pub mod gather;
pub mod instrument;
pub mod parsing;
mod types;
