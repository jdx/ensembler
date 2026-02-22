# Changelog

## [1.0.2](https://github.com/jdx/ensembler/compare/v1.0.1...v1.0.2) - 2026-02-22

### Fixed

- make execute() future Send and use process groups for clean kills ([#71](https://github.com/jdx/ensembler/pull/71))

## [1.0.1](https://github.com/jdx/ensembler/compare/v1.0.0...v1.0.1) - 2026-02-21

### Other

- *(deps)* bump clx from <1 to 1 ([#70](https://github.com/jdx/ensembler/pull/70))

## [1.0.0] - 2026-01-31

### Added
- `#[non_exhaustive]` attribute on `Error` enum for future compatibility
- `timeout()` method for setting command timeouts
- `allow_non_zero()` method to allow non-zero exit codes without error
- `Error::Cancelled` variant for cancelled commands
- `Error::TimedOut` variant for timed out commands
- Optional `progress` feature flag (enabled by default) for `clx` progress bar integration
- Comprehensive test suite

### Changed
- Improved error handling throughout the codebase
- Use Aho-Corasick algorithm for efficient multi-pattern secret redaction

### Fixed
- Replaced `unwrap()` calls with proper error handling
