#![feature(rustc_private)]
// #![feature(box_patterns)]
// #![feature(min_specialization)]
// #![feature(step_trait)]
// #![feature(unsize)]
// #![feature(coerce_unsized)]

mod common;

// incomplete tests
mod collections;
mod loops;
mod type_params;

// test suite
mod all_assign_operators;
mod array_of_struct;
mod all_binary_operators;
mod array;
mod array_high_dim;
mod array_with_slices;
mod assign_compound;
mod assign_tuples;
mod binary_search;
mod generic_struct;
mod longest_increasing_subsequence;
mod multi_file;
mod multi_file_with_submodules;
mod nested_returns;
mod op_through_trait;
mod ranges;
mod references;
mod simple;
mod type_hints;
mod unary_operators;
mod untracked_fns;
mod uses_enum;
mod uses_methods;
mod uses_struct;

// FIXME: It's kind of annoying that `cargo test` ends up showing this
// file in the output, but I honestly didn't like any of the quick solutions
// to this, and there are more interesting things to work on rn.
// also the case for the main.rs files...
