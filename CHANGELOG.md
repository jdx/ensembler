# Changelog

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
