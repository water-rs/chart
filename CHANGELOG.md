# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0](https://github.com/water-rs/chart/compare/v0.3.0...v0.4.0) - 2026-11-14

### Added

- *(chart)* [**breaking**] draw charts with the waterui Canvas API
- [**breaking**] implement the response-geometry layout query
- port to the render-target-neutral recording contract ([#33](https://github.com/water-rs/chart/pull/33))

### Fixed

- *(lint)* satisfy clippy 1.99 suboptimal_flops
- repin hydrolysis to 14d5e231, m3 to fcb8b9e8; patch all registry waterkit crates to 123b0581
- *(ci)* pin the toolchain without disarming the MSRV leg ([#26](https://github.com/water-rs/chart/pull/26))

### Other

- *(deps)* point migrated Batch 2 git dependencies to main ([#58](https://github.com/water-rs/chart/pull/58))
- update release-plz rev for Rust 1.100 compatibility
- release from main and retire dev ([#53](https://github.com/water-rs/chart/pull/53))
- restore the Hydrolysis test fonts from the Actions cache
- *(deps)* point migrated Batch 1 git dependencies to main ([#49](https://github.com/water-rs/chart/pull/49))
- *(chart)* derive each chart's frame from a CanvasSize
- follow the split repositories' dev again
- authenticated font downloads and cached dxc
- follow the framework's dev for response geometry
- lock the response-geometry framework branch
- follow waterui dev's toolchain floor and windows resolution
- read waterui structs through their accessors
- consume the framework and canvas from their dev branches
- keep gpu-allocator on windows 0.62 to match wgpu-hal
- lock the framework at its rebased nami 0.13 head
- lock the sibling components at their nami 0.13 heads
- [**breaking**] move to nami 0.13 and the migrated framework
- pass the release pull request's gate without CI
- [**breaking**] move the patch table to nami 0.12
- require nami 0.12
- *(deps)* pin waterui to dev at 46552dd30 ([#34](https://github.com/water-rs/chart/pull/34))
- *(deps)* pin hydrolysis, hydrolysis-m3 and video-gpu at the repinned heads
- pin cherenkov to 52236561
- install SPIRV-Tools through the shared Linux deps action
- install SPIRV-Tools for cherenkov-gpu's shader build on non-Apple legs
- repin to the Cherenkov cutover base
- repin the vello fork to the glifo canonical-location fix
- repin water-rs dependencies to merged dev heads
- repin water-rs dependencies to unified migration heads
- repin hydrolysis to accessibility import fix
- refresh lockfile for final branch revs
- repin migrated dependency branch heads
- rustfmt on migrated files
- [**breaking**] migrate to nami snapshot API
- consume the 0.5.0 framework wave from crates.io ([#25](https://github.com/water-rs/chart/pull/25))

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
