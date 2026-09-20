# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0](https://github.com/water-rs/chart/compare/v0.2.0...v0.3.0) - 2026-09-20

### Fixed

- *(a11y)* own the chart naming scope at the chart's bounds
- *(example)* share compatible framework and theme dependencies
- *(deps)* use corrected list viewport backend
- *(layout)* retain axis child placement proposals
- *(ci)* release with the water-rs release-plz fork and the shared Linux deps

### Other

- adopt the 0.5.0 framework wave (0.3.0) ([#22](https://github.com/water-rs/chart/pull/22))
- publish on the main push, not under workflow_run ([#21](https://github.com/water-rs/chart/pull/21))
- pin hydrolysis at the naming-scope bounds fix (water-rs/hydrolysis#50)
- Revert "test: wrap the chart surface in a stack so its a11y bounds match the chart"
- wrap the chart surface in a stack so its a11y bounds match the chart
- pin the framework at f3f58c48f (0.5.0 release candidate)
- let the PR source gate accept release-plz release branches ([#19](https://github.com/water-rs/chart/pull/19))
- name the requirements the pinned revisions provide
- adopt waterui 0.5.0 and name waterui-chart 0.3.0
- validate chart with the full layout conformance revision
- Merge commit '37b8471a9a593b7787ccc607a83e8fc1a66dbb41' into codex/proposal-aware-placement
- disable incremental builds and trim debuginfo ([#14](https://github.com/water-rs/chart/pull/14))
- run tests with cargo nextest ([#13](https://github.com/water-rs/chart/pull/13))
- Merge pull request #11 from water-rs/build/registry-deps
- consume the framework from crates.io
- publish to crates.io via OIDC trusted publishing ([#7](https://github.com/water-rs/chart/pull/7))
- gate pull requests into main so only dev may merge ([#8](https://github.com/water-rs/chart/pull/8))

## [0.2.0](https://github.com/water-rs/chart/compare/v0.1.0...v0.2.0) - 2026-09-11

### Added

- *(testing)* [**breaking**] quiescence-driven waits, virtual frame clock, orthogonal theme/mode, panicking interactions

### Fixed

- *(ci)* install the same Linux packages for the release preflight
- *(release)* verify registry-only package graph
- clear the pre-existing red on dev CI
- *(chart)* [**breaking**] one reactive ChartAxes and full-precision candle timestamps
- *(layout)* [**breaking**] stop losing a view's stretch axis when it is erased
- *(layout)* [**breaking**] count a column's growing children in its own height
- fix repository rule violations and refresh documentation
- fix chart transition rendering performance

### Other

- link the test graph to the waterui 0.4 release commit
- *(deps)* waterui-graphics 0.4 (and waterui-text/-testing 0.4 where used)
- resolve gpu-allocator against windows 0.62, as wgpu-hal does
- depend on the released waterui crates instead of the monorepo dev branch
- update Linux package matrix and add dxc on Windows
- setup standalone crate files, CI workflows, and release-plz
- [**breaking**] ungate Scene2D from the GPU stack and drop its Vello escape hatches
- ship the licence texts in every published crate
- clear the rustdoc warnings
- *(waterui-chart)* compile every doc example
- format audit-fix files with the workspace rustfmt
- Give the scroll region its content, and the rest of dev's red CI
- Format the workspace
- *(tests)* finish chart macro migration (readout_layout + support helper)
- *(tests)* [**breaking**] migrate hand-written builder tests to #[waterui::test]
- Use fused math in chart geometry
- Make reactivity precise across renderers
- clean up clippy warnings across the workspace
- Lean dependency graph for embedded: gpu/widgets/gestures features
- Fix Hydrolysis example rendering and macOS acceptance
- Fix chart accessibility node duplication
- Restore WaterUI CI gates and reactive map API
- reorganize the project
