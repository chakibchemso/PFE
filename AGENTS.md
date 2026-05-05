# Repository Guidelines

## Project Structure & Module Organization
This repository combines embedded, web, FPGA, and documentation work:

- `esp32/`: embedded Rust firmware for the ESP32-S3. Main entry point is `src/bin/main.rs`; shared modules live in `src/` (`app/`, `crypto`, `drivers/`, `dsp/`, `system/`, `tasks/`, `ui/`, `utils`); device tests live in `tests/`; signal-processing tools live in `tools/`.
- `webapp/`: Leptos + Axum Rust web app. Application code is in `src/`, static assets in `public/`, styles in `style/`, and browser tests in `end2end/tests/`.
- `fpga/`: FPGA project files plus the `ascon-hardware` hardware submodule. HDL sources and testbenches are under `fpga/ascon-hardware/hardware/ascon_lwc/`.
- `doc/` and `cad/`: documentation and CAD/manufacturing assets.

## Build, Test, and Development Commands

| Command | What it does |
|---|---|
| `cd esp32 && cargo build` | Compile the firmware crate for `xtensa-esp32s3-none-elf`. |
| `cd esp32 && cargo test` | Run the embedded test target defined in `tests/hello_test.rs`. |
| `cd webapp && cargo leptos watch` | Run the web app locally with live reload. |
| `cd webapp && cargo leptos build --release` | Produce the server binary and WASM/CSS site bundle. |
| `cd webapp && cargo leptos end-to-end` | Run Playwright end-to-end tests (requires `npm install` first). |
| `cd webapp/end2end && npm install` | Install Playwright dependencies before browser tests. |
| `cd fpga/ascon-hardware/hardware/ascon_lwc && make v6` | Run the VHDL testbench for a hardware variant. Swap `v6` for `v1`, `v2`, etc. as needed. |

Enable serial signal plotting in the firmware with the `plot` feature flag:
```
cargo build --features plot
```

## Coding Style & Naming Conventions

- **Rust**: run `cargo fmt` in the relevant crate before opening a PR. Follow standard Rust naming: `snake_case` for files, modules, and functions; `PascalCase` for types and traits.
- **Modules**: keep embedded and crypto logic split into focused modules — e.g. `mqtt.rs`, `crypto.rs`, `pipeline.rs`, `filters.rs`. Do not dump unrelated logic into a single file.
- **VHDL**: preserve the current variant-based directory naming (`v1`, `v6`, `v1_8bit`). Do not rename variant directories.
- **Clippy**: a `.clippy.toml` is present in `esp32/`. Run `cargo clippy` and resolve warnings before committing.

## Testing Guidelines

- **Embedded tests** (`esp32/tests/*.rs`): use the `embedded-test` harness with the `xtensa-semihosting` feature. Run with `cargo test` from `esp32/`.
- **Browser tests** (`webapp/end2end/tests/*.spec.ts`): use Playwright. Run with `cargo leptos end-to-end` from `webapp/`.
- **HDL verification**: use the `make <variant>` flow inside `fpga/ascon-hardware/hardware/ascon_lwc/`. Known-answer tests (KAT) live in `KAT/<variant>/`.
- Test coverage is currently light. New features should include at least one automated test path or a documented manual validation step.

## Commit & Pull Request Guidelines

- Use short, imperative commit subjects under ~72 characters.  
  Examples: `Add FPGA project with Ascon submodule`, `feat: Add DSP pipeline`.
- Prefix with a Conventional Commit type (`feat:`, `fix:`, `refactor:`, `docs:`, `chore:`) when the change is non-trivial.
- PR descriptions must include:
  - **Scope**: which sub-project is affected (`esp32`, `webapp`, `fpga`).
  - **What changed** and **why**.
  - **Test evidence**: output of `cargo test`, `make <variant>`, or Playwright results.
  - **Screenshots** for any UI changes.
- Link related issues where applicable.

## Architecture Overview

The system is a health-monitoring wearable built around three interconnected layers:

1. **ESP32-S3 firmware** — reads vital signs (SpO₂/HR via MAX30105), runs a DSP pipeline (FIR filters, metrics), renders a Slint UI on a CO5300 display, and publishes encrypted MQTT telemetry over Wi-Fi using Ascon-AEAD.
2. **Web app** — a Leptos/Axum SSR + WASM app that subscribes to the MQTT broker, decrypts the payload client-side with the same Ascon key, and displays live vitals in the browser.
3. **FPGA** — a hardware Ascon LWC core (VHDL) targeting a Gowin FPGA, used to evaluate the cipher's hardware performance.

## Security & Configuration Tips

- Cryptographic keys and Wi-Fi credentials must **never** be committed. Use environment variables or a local config file excluded by `.gitignore`.
- The `ascon-hardware` FPGA tree is a Git submodule. After cloning, run `git submodule update --init` to populate it.
- The `esp32` crate targets `xtensa-esp32s3-none-elf` and requires the Espressif Rust toolchain. See `esp32/rust-toolchain.toml` for the pinned channel.
