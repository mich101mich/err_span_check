#![deny(
    missing_docs,
    missing_debug_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links,
    rustdoc::missing_crate_level_docs,
    rustdoc::invalid_codeblock_attributes,
    rustdoc::invalid_html_tags,
    rustdoc::bare_urls,
    rustdoc::redundant_explicit_links,
    rustdoc::unescaped_backticks
)]
#![allow(clippy::test_attr_in_doctest)] // false positive on the #[test] in the doc comment for run_on_fail_dir

//! A test harness for checking and comparing compiler errors with a focus on error spans.
//!
//! This crate is useful mainly for procedural macros, but can also be used in other contexts where
//! compiler errors need to be verified.
//!
//! It provides a way to verify that your macro underlines the correct part of the code with its errors,
//! using in-file error annotations.
//!
//! # Usage
//!
//! 1. Add `err_span_check` to your `dev-dependencies`.
//! 2. Create a `tests/fail` directory in your project.
//! 3. Add `.rs` files with test cases in `tests/fail`.
//! 4. Create a test file (e.g. `tests/test.rs`) with the following content:
//!
//! ```rust
//! #[test]
//! fn compile_error_tests() {
//!     err_span_check::run_on_fail_dir();
//! }
//! ```
//!
//! 5. Run `cargo test`.
//!
//! # Workflow
//!
//! When running tests, `err_span_check` will automatically update your test files in `tests/fail` with the actual
//! compiler errors. You inspect these changes using `git diff` and commit them if they are correct.
//!
//! **Note:** All test cases in `tests/fail` must be at least staged in git.
//!
//! See the [README](https://github.com/mich101mich/err_span_check) for more details.

#[macro_use]
mod message;

mod cargo;
mod fail_dir;
mod git;
mod manifest;
mod normalize;
mod project;
mod runner;
mod rustflags;
mod util {
    pub(crate) mod env;
    pub(crate) mod extensions;
    pub(crate) mod features;
}

pub(crate) use fail_dir::TestFile;
pub(crate) use project::Project;
pub(crate) use util::extensions::*;

pub(crate) use anyhow::{Context, Error, Result, bail};
pub(crate) use serde::{Deserialize, Serialize, de::Deserializer, ser::Serializer};

pub(crate) use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
};

/// The entry point for this crate. Call this once inside of a test function.
///
/// ```
/// #[test]
/// fn test() {
///     err_span_check::run_on_fail_dir();
/// }
/// ```
///
/// And that's it. All `*.rs` files anywhere within the `tests/fail` directory will be tested as compile-fail tests.
pub fn run_on_fail_dir() {
    match runner::run() {
        Ok(()) => {}
        Err(err) => {
            message::fail(err);
            panic!("err_span_check failed");
        }
    }
}
