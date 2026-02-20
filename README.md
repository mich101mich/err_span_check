Error Span Checker
========

[<img alt="github" src="https://img.shields.io/badge/github-mich101mich/err_span_check-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/mich101mich/err_span_check)
[<img alt="crates.io" src="https://img.shields.io/crates/v/err_span_check.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/err_span_check)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-err_span_check-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/err_span_check)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/mich101mich/err_span_check/ci.yml?branch=master&style=for-the-badge" height="20">](https://github.com/mich101mich/err_span_check/actions?query=branch%3Amaster)

A test harness for checking and comparing compiler errors with a focus on error spans. Useful mainly for procedural
macros, but can also be used in other contexts.

This crate is a fork of [trybuild] with inline and in-file error message syntax rather than separate `.stderr` files. Passing tests have been removed for simplicity.

[trybuild]: https://crates.io/crates/trybuild

TODO: Things to talk about:
- Update by default, "frozen" for CI
- requires git
- file format
  - contain arbitrary code
  - at least 5 `/////` start a test case, text to give name
  - Line with at least 10 `//////////` and nothing else to end test case
  - test blocks are removed for other tests
- produces error annotations (not written by hand!)
  - `//~~~~~~~~~~~~~~~~~~~~ errors ~~~~~~~~~~~~~~~~~~~~//` to start external errors
  - `//~` marks an inline spanned message
    - line will be removed from actual test, so might appear in an invalid place




Such tests are commonly useful for testing error reporting involving procedural
macros. We would write test cases triggering either errors detected by the macro
or errors detected by the Rust compiler in the resulting expanded code, and
compare against the expected errors to ensure that they remain user-friendly.

This style of testing is sometimes called *ui tests* because they test aspects
of the user's interaction with a library outside of what would be covered by
ordinary API tests.

Nothing here is specific to macros; err_span_check would work equally well for testing
misuse of non-macro APIs.

```toml
[dev-dependencies]
err_span_check = "1.0"
```

<br>

## Compile-fail tests

A minimal err_span_check setup looks like this:

```rust
#[test]
fn ui() {
    let t = err_span_check::TestCases::new();
    t.test("tests/ui/*.rs");
}
```

The test can be run with `cargo test`. It will individually compile each of the
source files matching the glob pattern, expect them to fail to compile, and
assert that the compiler's error message matches an adjacently named _*.stderr_
file containing the expected output (same file name as the test except with a
different extension). If it matches, the test case is considered to succeed.

Dependencies listed under `[dev-dependencies]` in the project's Cargo.toml are
accessible from within the test cases.

Failing tests display the expected vs actual compiler output inline.

<p align="center">
<a href="#compile-fail-tests">
<img src="https://user-images.githubusercontent.com/1940490/57186575-79418e80-6e96-11e9-9478-c9b3dc10327f.png" width="600">
</a>
</p>

A compile\_fail test that fails to fail to compile is also a failure.

<p align="center">
<a href="#compile-fail-tests">
<img src="https://user-images.githubusercontent.com/1940490/57186576-7b0b5200-6e96-11e9-8bfd-2de705125108.png" width="600">
</a>
</p>

To test just one source file, use:
```
cargo test -- ui err_span_check=example.rs
```
where `ui` is the name of the `#[test]` function that invokes `err_span_check`, and
`example.rs` is the name of the file to test.

<br>

## Details

That's the entire API.

<br>

## Workflow

There are two ways to update the _*.stderr_ files as you iterate on your test
cases or your library; handwriting them is not recommended.

First, if a test case is being run as compile\_fail but a corresponding
_*.stderr_ file does not exist, the test runner will save the actual compiler
output with the right filename into a directory called *wip* within the
directory containing Cargo.toml. So you can update these files by deleting them,
running `cargo test`, and moving all the files from *wip* into your testcase
directory.

<p align="center">
<a href="#workflow">
<img src="https://user-images.githubusercontent.com/1940490/57186579-7cd51580-6e96-11e9-9f19-54dcecc9fbba.png" width="600">
</a>
</p>

Alternatively, run `cargo test` with the environment variable
`ERR_SPAN_CHECK=overwrite` to skip the *wip* directory and write all compiler output
directly in place. You'll want to check `git diff` afterward to be sure the
compiler's output is what you had in mind.

<br>

## What to test

When it comes to compile-fail tests, write tests for anything for which you care
to find out when there are changes in the user-facing compiler output. As a
negative example, please don't write compile-fail tests simply calling all of
your public APIs with arguments of the wrong type; there would be no benefit.

A common use would be for testing specific targeted error messages emitted by a
procedural macro. For example the derive macro from the [`ref-cast`] crate is
required to be placed on a type that has either `#[repr(C)]` or
`#[repr(transparent)]` in order for the expansion to be free of undefined
behavior, which it enforces at compile time:

[`ref-cast`]: https://github.com/dtolnay/ref-cast

```console
error: RefCast trait requires #[repr(C)] or #[repr(transparent)]
 --> $DIR/missing-repr.rs:3:10
  |
3 | #[derive(RefCast)]
  |          ^^^^^^^
```

Macros that consume helper attributes will want to check that unrecognized
content within those attributes is properly indicated to the caller. Is the
error message correctly placed under the erroneous tokens, not on a useless
call\_site span?

```console
error: unknown serde field attribute `qqq`
 --> $DIR/unknown-attribute.rs:5:13
  |
5 |     #[serde(qqq = "...")]
  |             ^^^
```

Declarative macros can benefit from compile-fail tests too. The [`json!`] macro
from serde\_json is just a great big macro\_rules macro but makes an effort to
have error messages from broken JSON in the input always appear on the most
appropriate token:

[`json!`]: https://docs.rs/serde_json/1.0/serde_json/macro.json.html

```console
error: no rules expected the token `,`
 --> $DIR/double-comma.rs:4:38
  |
4 |     println!("{}", json!({ "k": null,, }));
  |                                      ^ no rules expected this token in macro call
```

Sometimes we may have a macro that expands successfully but we count on it to
trigger particular compiler errors at some point beyond macro expansion. For
example the [`readonly`] crate introduces struct fields that are public but
readable only, even if the caller has a &mut reference to the surrounding
struct. If someone writes to a readonly field, we need to be sure that it
wouldn't compile:

[`readonly`]: https://github.com/dtolnay/readonly

```console
error[E0594]: cannot assign to data in a `&` reference
  --> $DIR/write-a-readonly.rs:17:26
   |
17 |     println!("{}", s.n); s.n += 1;
   |                          ^^^^^^^^ cannot assign
```

In all of these cases, the compiler's output can change because our crate or one
of our dependencies broke something, or as a consequence of changes in the Rust
compiler. Both are good reasons to have well conceived compile-fail tests. If we
refactor and mistakenly cause an error that used to be correct to now no longer
be emitted or be emitted in the wrong place, that is important for a test suite
to catch. If the compiler changes something that makes error messages that we
care about substantially worse, it is also important to catch and report as a
compiler issue.

<br>

## Troubleshooting

The Rust compiler's diagnostic output can vary as a function of whether the
`rust-src` Rustup component is installed. The compiler will render source
snippets from the standard library if the standard library source is available
locally, and will simply omit snippets if not. This can account for differences
between CI and local development.

If you have compile_fail tests pertaining to standard library traits or types,
you can ensure a consistent environment by adding a rust-toolchain.toml file
with the following content.

```toml
[toolchain]
components = ["rust-src"]
```

<br>

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
