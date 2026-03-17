# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - Unreleased

### Removed
- Removed support for features, since the previous setup breaks with [build-dir-layout-v2]
  and is far too fragile to be maintained.

[build-dir-layout-v2]: https://github.com/rust-lang/cargo/issues/15010

## [0.1.1] - 2026-02-28

### Fixed
- Fixed compatibility with [cargo-llvm-cov]

[0.1.1]: https://github.com/mich101mich/err_span_check/releases/tag/0.1.1
[cargo-llvm-cov]: https://github.com/taiki-e/cargo-llvm-cov

## [0.1.0] - 2026-02-23

### Added
- The core functionality as a heavily modified fork of [trybuild@1.0.114]

[0.1.0]: https://github.com/mich101mich/err_span_check/releases/tag/0.1.0
[trybuild@1.0.114]: https://github.com/dtolnay/trybuild/releases/tag/1.0.114
