# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/water-rs/chart/releases/tag/v0.1.0) - 2026-09-02

### Added

- *(testing)* [**breaking**] quiescence-driven waits, virtual frame clock, orthogonal theme/mode, panicking interactions

### Fixed

- *(release)* verify registry-only package graph
- clear the pre-existing red on dev CI
- *(chart)* [**breaking**] one reactive ChartAxes and full-precision candle timestamps
- *(layout)* [**breaking**] stop losing a view's stretch axis when it is erased
- *(layout)* [**breaking**] count a column's growing children in its own height
- fix repository rule violations and refresh documentation
- fix chart transition rendering performance

### Other

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
