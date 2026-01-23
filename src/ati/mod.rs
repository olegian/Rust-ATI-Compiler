// Without including ati.rs in the root crate
// rust-analyzer fails to run for that file.
// Adding this just to make the dev experience better.
pub mod ati;
