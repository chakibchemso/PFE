# Repository Guidelines

## Project Structure & Module Organization

This repository combines embedded firmware, a Rust web app, FPGA assets, and documentation.

- `esp32/`: ESP32-S3 Rust firmware. Entry point: `src/bin/main.rs`; modules live in `src/`; tests live in `tests/`.
- `webapp/`: Leptos + Axum SSR app. Code is in `src/`, assets in `public/`, styles in `style/`, and Playwright tests in `end2end/tests/`.
- `fpga/`: FPGA files and `ascon-hardware`; HDL/testbenches are under `fpga/ascon-hardware/hardware/ascon_lwc/`.
- `doc/`, `cad/`: documentation, CAD, and manufacturing assets.

## Build, Test, and Development Commands

- `cd esp32 && cargo build`: builds firmware for `xtensa-esp32s3-none-elf`.
- `cd esp32 && cargo test`: runs embedded tests using the configured harness.
- `cd esp32 && cargo build --features plot`: enables serial signal plotting.
- `cd webapp && cargo leptos watch`: runs the web app locally with live reload.
- `cd webapp && cargo leptos build --release`: builds the server and WASM/CSS bundle.
- `cd webapp/end2end && npm install`: installs Playwright dependencies.
- `cd webapp && cargo leptos end-to-end`: runs browser end-to-end tests.
- `cd fpga/ascon-hardware/hardware/ascon_lwc && make v6`: runs the VHDL testbench; replace `v6` as needed.

## Coding Style & Naming Conventions

- Rust follows standard conventions: `snake_case` for files, modules, and functions; `PascalCase` for types and traits.
- Run `cargo fmt` in the relevant Rust crate before submitting changes.
- Keep embedded, DSP, crypto, MQTT, UI, and driver logic in focused modules.
- For VHDL, preserve existing variant directory names such as `v1`, `v6`, and `v1_8bit`.
- Use `cargo clippy` for the ESP32 crate and resolve warnings where practical.

## Testing Guidelines

- Embedded tests use `embedded-test` and live in `esp32/tests/*.rs`.
- Web end-to-end tests use Playwright and live in `webapp/end2end/tests/*.spec.ts`.
- HDL verification uses `make <variant>` inside `fpga/ascon-hardware/hardware/ascon_lwc/`.
- New features should include an automated test or documented manual validation.

## Commit & Pull Request Guidelines

- Use short, imperative commit subjects under about 72 characters.
- Prefer Conventional Commit prefixes: `feat:`, `fix:`, `refactor:`, `docs:`, or `chore:`.
- Pull requests should state scope, changes, rationale, and test evidence.
- Include screenshots for UI changes and link related issues when applicable.

## Security & Configuration Tips

- Never commit Wi-Fi credentials, cryptographic keys, broker passwords, or secrets.
- Use environment variables or ignored local config files for sensitive values.
- After cloning, run `git submodule update --init` to populate FPGA submodules.
- The ESP32 crate requires the pinned Espressif Rust toolchain in `esp32/rust-toolchain.toml`.
