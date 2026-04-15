mod common;

// incomplete tests
mod collections;
mod loops;
mod type_params;

// test suite
mod array;
mod array_high_dim;
mod multi_file;
mod nested_returns;
mod simple;
mod type_hints;
mod untracked_fns;
mod uses_enum;
mod uses_methods;
mod uses_struct;
mod all_binary_operators;
mod all_assign_operators;
mod generic_struct;
mod unary_operators;
mod ranges;

// FIXME: It's kind of annoying that `cargo test` ends up showing this
// file in the output, but I honestly didn't like any of the quick solutions
// to this, and there are more interesting things to work on rn.
// also the case for the main.rs files...
