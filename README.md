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
cargo run [OPTIONAL] -- INPUT [OPTIONAL]
```

Optional arguments passed before the `--` are passed to the `rustc` invocation responsible for building the instrumentation compiler. Optional arguments passed after are forwarded to the compiler invocation when instrumenting `INPUT`.

Note that if this project is built into a binary, it requires extra linking with `rustc`'s private library to execute, by setting the `LD_LIBRARY_PATH` environment variable to point to the nightly compiler build (e.g. `$REPO_HOME/target/debug/deps:$HOME/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib`). Until this is resolved, it's easiest to stick with the `cargo run` option mentioned above.

## File Description
The following files make up the majority of the implementation:

1. `src/main.rs`: Entry point. Defines necessary callbacks (which perform multiple passes over the AST to add necessary statements), then invokes `rustc` passing in these callbacks and forwarding all arguments.
2. `src/ati/ati.rs`: Contains all code used by instrumentation during runtime, to track tagged value interactions. The contents of this file are automatically defined in the root file of the INPUT source file.
3. `src/instrumentation/params.rs`: Defines the first AST pass, responsible for discovering all instrumented functions, updating their signatures to make primitive types into tagged values. Function names are changed to allow for the creation of "stubs". All structs which contain primitive types are also converted to use tagged values instead.
4. `src/instrumentation/statements.rs`: Defines the second AST pass, responsible for converting all literal expressions into tracked values.
5. `src/instrumentation/stubs.rs`: Creates copies of each instrumented function, to run necessary preludes before function execution and epilogues after function execution. This controls the sites, points in the program where we report the discovered abstract types, on function entrance and exit.
6. `src/instrumentation/types.rs`: Responsible for defining all functions and types in `ati.rs` in INPUT.
7. `tests/*`: Unit tests, which invoke the compiler on input files and checks the ATI output against an expected partition.

## Output
The exact form of output is governed by `src/ati/ati.rs::ATI::report()`. This function is invoked right before `main` exits. Currently, the output is just written to stdout, starting with `===ATI-ANALYSIS-START===`. For example, the following output is produced by instrumenting and executing a simple program which has `main` invoke another function `foo`, which accepts three parameters `x`, `y`, and `z` (`tests/simple/input.rs`). The values of `x` and `y` are used to compute the return value, `RET`.
```
===ATI-ANALYSIS-START===
foo::ENTER
x:0
y:1
z:2
---
foo::EXIT
RET:0
x:0
y:0
z:2
---
main::ENTER
---
main::EXIT
---
```
This instrumentation only reports the abstract types of formals and return values, ultimately to construct a program specification.

## Features yet to be implemented:
The following is a list of features still in progress:

1. Instrumenting multi-file projects. The current version only supports instrumentation of a single file.
2. Collections (like `Vec`) are currently unsupported.
    - In general, non-user defined functions which return complex types will break instrumentation.
3. Undoubtedly much more!
