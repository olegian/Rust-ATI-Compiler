# DATIR: Dynamic Abstract Type Inference in Rust
This repository contains all the code necessary to automatically insert instrumentation into arbitrary Rust source code, to perform abstract type inference (ATI). This project is loosely based on [dynamic inference of abstract types](https://dl.acm.org/doi/10.1145/1146238.1146268).

## Using This Repository
This instrumentation relies on `rustc`'s query system to execute callbacks. This requires linking against `rustc`'s nightly build. To do so, run the following bash [commands](https://rust-lang.github.io/rustup/installation/index.html):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain none -y;

echo ". \"$HOME/.cargo/env\"" >> ~/.bashrc;

rustup toolchain install nightly --allow-downgrade --profile minimal --component clippy;

rustup component add rust-src rustc-dev llvm-tools-preview;
```

At this point, you should be able to compile and run this project with:
```sh
cargo +nightly run [OPTIONAL] -- INPUT [OPTIONAL]
```

For more usage information, run `cargo +nightly run -- --help`.

Note that if this project is built into a binary, and separately executed it requires extra linking with `rustc`'s private libraries to execute, by setting the `LD_LIBRARY_PATH` environment variable to point to the nightly compiler build (e.g. `$REPO_HOME/target/debug/deps:$HOME/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib`). Until this is resolved, it's easiest to stick with the `cargo run` option mentioned above.

If typing `+nightly` becomes tedious, feel free to run `rustup default nightly` to default to the nightly compiler build. After executing that command, you can simply omit the nightly flag. To switch back to the stable build as the default, use `rustup default stable`.

## File Description
The following files make up the majority of the implementation:

1. `src/ati/*`: Contains the ATI runtime library that is used at runtime to dynamically keep track of value interactions. All files within this directory are injected into the target crate.
2. `src/callbacks/*`: Defines the callbacks used by various compiler invocations. DATIR currently relies on being able to perform two compilations, one to generally gather some information (`src/callbacks/gather`), another to perform the actual instrumentation (`src/callbacks/instrument`). Following instrumentation, some extra code has to be generated and inserted into the crate. This is done by code contained within `src/callbacks/codegen`.
3. `src/file_loader/*`: Defines a custom rustc-compatible `FileLoader` which is capable of performing AST-level mutations before the file contents even make it to the compiler parser. This allows instrumentation of all files, not just the crate root.
7. `tests/*`: Unit tests, which invoke the compiler on input files and checks the ATI output against an expected partition.

## Output
DATIR can produce two kinds of output, based on what flags are used to invoke it. If `--release ATI_OUTPUT_DIR` is specified, then the produced target binary will write a file to the output directory every time it is invoked, in the `.ati` format that is compatible with the `decls-merger`.

If `--release` is unspecified, then executing the produced target binary will instead simple print the comparability report to stdout, in the following format:

```
===ATI-ANALYSIS-START===
tests/simple/main.rs::foo:::ENTER
x -> 1
y -> 1
z -> 2
---
tests/simple/main.rs::foo:::EXIT
return -> 0
x -> 0
y -> 0
z -> 2
---
tests/simple/main.rs::main:::ENTER
---
tests/simple/main.rs::main:::EXIT
---
```

This instrumentation only reports the abstract types of formals and return values, ultimately to construct a program specification.
