# Repository Guidelines

## Project Structure & Module Organization
This repository combines embedded, web, FPGA, and documentation work:

- `esp32/`: embedded Rust firmware for the ESP32-S3. Main entry point is `src/bin/main.rs`; shared modules live in `src/`; device tests live in `tests/`; UI assets live in `src/ui/`.
- `webapp/`: Leptos + Axum Rust web app. Application code is in `src/`, static assets in `public/`, styles in `style/`, and browser tests in `end2end/tests/`.
- `fpga/`: FPGA project files plus the `ascon-hardware` hardware tree. HDL sources and testbenches are under `fpga/ascon-hardware/hardware/ascon_lwc/`.
- `doc/` and `cad/`: documentation and CAD/manufacturing assets.

## Build, Test, and Development Commands
- `cd esp32 && cargo build`: build the firmware crate.
- `cd esp32 && cargo test`: run the embedded test target defined in `tests/hello_test.rs`.
- `cd webapp && cargo leptos watch`: run the web app locally with live reload.
- `cd webapp && cargo leptos build --release`: produce the server binary and site bundle.
- `cd webapp && cargo leptos end-to-end`: run Playwright end-to-end tests.
- `cd webapp/end2end && npm install`: install Playwright dependencies before browser tests.
- `cd fpga/ascon-hardware/hardware/ascon_lwc && make v6`: run the VHDL testbench for a hardware variant. Swap `v6` for `v1`, `v2`, etc. as needed.

## Coding Style & Naming Conventions
Use `cargo fmt` for Rust crates before opening a PR. Follow existing Rust naming: snake_case for files, modules, and functions; PascalCase for types. Keep embedded and crypto logic split into focused modules such as `mqtt.rs`, `crypto.rs`, and `processor.rs`. For VHDL, preserve the current variant-based directory naming (`v1`, `v6`, `v1_8bit`).

## Testing Guidelines
Add tests next to the component they exercise: embedded tests in `esp32/tests/*.rs`, browser tests in `webapp/end2end/tests/*.spec.ts`, and HDL verification through the `make <variant>` flows. Test coverage is currently light, so new features should include at least one automated path or a documented validation step.

## Commit & Pull Request Guidelines
Recent history mixes direct summaries (`Add FPGA project...`) with Conventional Commit style (`feat: Add initial project structure...`). Prefer short, imperative subjects and keep them under roughly 72 characters. PRs should include a clear scope, affected area (`esp32`, `webapp`, `fpga`), test evidence, and screenshots for UI changes.
