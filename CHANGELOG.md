# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/okkez/aws-assume-role-rs/compare/v0.1.7...v0.2.0) - 2025-01-07

### Added

- use tracing crate for logging
- use cache-vault

### Fixed

- log format
- to update skim v0.15.0
- *(deps)* update rust crate serde to v1.0.217
- *(deps)* update rust crate tokio to v1.42.0

### Other

- Merge pull request [#83](https://github.com/okkez/aws-assume-role-rs/pull/83) from okkez/renovate/aws-sdk-rust-monorepo
- Merge pull request [#84](https://github.com/okkez/aws-assume-role-rs/pull/84) from okkez/use-cache-vault
- use tracing_test
- follow previous changes
- Merge pull request [#81](https://github.com/okkez/aws-assume-role-rs/pull/81) from okkez/renovate/testcontainers
- update testcontainers
- *(deps)* update testcontainers
- Merge pull request [#77](https://github.com/okkez/aws-assume-role-rs/pull/77) from okkez/renovate/aws-sdk-rust-monorepo
- Merge pull request [#73](https://github.com/okkez/aws-assume-role-rs/pull/73) from okkez/renovate/backon-0.x
- Merge pull request [#63](https://github.com/okkez/aws-assume-role-rs/pull/63) from okkez/renovate/aws-sdk-rust-monorepo
- Merge pull request [#64](https://github.com/okkez/aws-assume-role-rs/pull/64) from okkez/renovate/clap-4.x-lockfile
- Merge pull request [#70](https://github.com/okkez/aws-assume-role-rs/pull/70) from okkez/renovate/chrono-0.x-lockfile
- Merge pull request [#62](https://github.com/okkez/aws-assume-role-rs/pull/62) from okkez/renovate/rstest-0.x

## [0.1.7](https://github.com/okkez/aws-assume-role-rs/compare/v0.1.6...v0.1.7) - 2024-08-09

### Other
- Merge pull request [#58](https://github.com/okkez/aws-assume-role-rs/pull/58) from okkez/set-role-arn
- Merge pull request [#51](https://github.com/okkez/aws-assume-role-rs/pull/51) from okkez/renovate/aws-sdk-rust-monorepo
- Merge pull request [#56](https://github.com/okkez/aws-assume-role-rs/pull/56) from okkez/renovate/mockall-0.x

## [0.1.6](https://github.com/okkez/aws-assume-role-rs/compare/v0.1.5...v0.1.6) - 2024-06-28

### Added
- improve arg handling
- improve error handling

### Fixed
- *(deps)* update rust crate clap to v4.5.7
- *(deps)* update rust crate toml to v0.8.14
- *(deps)* update rust crate aws-sdk-sts to v1.28.0
- *(lint)* cargo fmt

### Other
- Merge pull request [#47](https://github.com/okkez/aws-assume-role-rs/pull/47) from okkez/renovate/serde_json-1.x-lockfile
- Merge pull request [#42](https://github.com/okkez/aws-assume-role-rs/pull/42) from okkez/renovate/aws-sdk-rust-monorepo
- Merge pull request [#40](https://github.com/okkez/aws-assume-role-rs/pull/40) from okkez/renovate/clap-4.x-lockfile
- Merge pull request [#39](https://github.com/okkez/aws-assume-role-rs/pull/39) from okkez/renovate/aws-sdk-rust-monorepo
- Merge pull request [#38](https://github.com/okkez/aws-assume-role-rs/pull/38) from okkez/renovate/toml-0.x-lockfile
- Merge pull request [#35](https://github.com/okkez/aws-assume-role-rs/pull/35) from okkez/renovate/tokio-1.x-lockfile
- Merge pull request [#34](https://github.com/okkez/aws-assume-role-rs/pull/34) from okkez/renovate/aws-sdk-rust-monorepo
- *(deps)* update testcontainers-modules to 0.5.0
- *(deps)* update rust crate testcontainers to 0.17.0
- organize

## [0.1.5](https://github.com/okkez/aws-assume-role-rs/compare/v0.1.4...v0.1.5) - 2024-05-27

### Added
- improve arguments validation
- relax totp args to make it easy in shell script

## [0.1.4](https://github.com/okkez/aws-assume-role-rs/compare/v0.1.3...v0.1.4) - 2024-05-26

### Added
- add --aws-profile option

### Fixed
- *(deps)* update rust crate serde to v1.0.203
- *(deps)* update aws-sdk-rust monorepo

### Other
- run cargo fmt
- update README.md
- add env option to --aws-profile
- add more cases
- use set_* instead to call assume_role api

## [0.1.3](https://github.com/okkez/aws-assume-role-rs/compare/v0.1.2...v0.1.3) - 2024-05-19

### Other
- add missing command
- fix grammar
- fix assets name
- update

## [0.1.2](https://github.com/okkez/aws-assume-role-rs/compare/v0.1.1...v0.1.2) - 2024-05-18

### Added
- improve arguments validation
- improve arguments validation
- use PathBuf instead of String for --config
- support ini file ($HOME/.aws/config)
- rename package
- use spawn on Windows instead of exec

### Fixed
- *(deps)* update rust crate aws-sdk-sts to v1.25.0
- *(deps)* update rust crate toml to v0.8.13
- run cargo fmt

### Other
- use cargo-nextest
- ignore integration test
- add more test for command line arguments
- add more integration tests
- add integration test using docker
- format
- add test for assume_role
- add tests
- add simple test
- set STS client as a parameter
- extract cli module
- add simple test
- add rustfmt.toml and run cargo fmt
- Merge pull request [#14](https://github.com/okkez/aws-assume-role-rs/pull/14) from okkez/renovate/toml-0.x-lockfile
- Merge pull request [#10](https://github.com/okkez/aws-assume-role-rs/pull/10) from okkez/support-windows

## [0.1.1](https://github.com/okkez/aws-assume-role-rs/compare/v0.1.0...v0.1.1) - 2024-05-13

### Other
- update
- Merge pull request [#7](https://github.com/okkez/aws-assume-role-rs/pull/7) from okkez/support-retry-api
- add more build targets
- update README.md
- fix branch name
