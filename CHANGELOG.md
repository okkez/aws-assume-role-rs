# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
