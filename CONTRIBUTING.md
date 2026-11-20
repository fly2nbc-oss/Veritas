# Contributing

Thank you for helping improve **Veritas**.

## Prerequisites

1. [Rust](https://rustup.rs/) – `rust-toolchain.toml` pins the version (1.99.0); `cargo` installs it when needed.
2. Windows: Visual Studio or the Build Tools with the C++ workload (linker, and `rc.exe` for the exe's icon and version info).
3. Linux: `pkg-config libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev` and OpenGL.

## Getting started

```bash
git clone https://github.com/fly2nbc-oss/Veritas.git
cd Veritas
cargo run --release -- "/path/to/folder"
```

## Checking your build

```bash
scripts/quality.sh all   # like CI: guard check, format, clippy, tests with coverage, osv-scanner
cargo deny check         # vulnerabilities, licenses and sources of the dependencies (deny.toml)
```

`scripts/quality.sh` runs on Linux and on Windows in Git Bash. It needs `cargo-llvm-cov` (`cargo install cargo-llvm-cov --locked`; `llvm-tools-preview` is listed in `rust-toolchain.toml`) and, for the vulnerability check, `osv-scanner` – without it only CI runs that check. Test coverage must not fall below the value in `.quality-coverage`. `scripts/quality.sh lint` checks only format and clippy, `scripts/quality.sh doctor` shows whether the tools fit.

The tests need no display. What the whole window does with a key – confirmation dialog, history card, deleting, filters, the Esc chain – is tested by `ui::tests` with a bare egui context (`VeritasApp::show`) and folders in the temp directory. Nothing there touches your files; the deletion tests use a stub instead of the trash.

The program and the library must not panic: `unwrap`, `expect`, `panic!`, slicing strings (`&s[a..b]`) and anything else that can crash are forbidden outside the tests (`[lints]` in `Cargo.toml`, `#![deny]` in `lib.rs` and `main.rs`, `clippy.toml`). `unsafe` appears only in three Windows calls, every block with a `// SAFETY:` comment.

## User interface

Veritas follows the UI design system of the fly2nbc-oss desktop apps. Colors, sizes and building blocks are in `src/theme.rs`; the interface uses only these tokens, no hex values. The deliberate deviations (among them rows 28 instead of 32 px high) are listed in [`AGENTS.md`](AGENTS.md) under *Product decisions*. Interface texts live only in `src/i18n/`, in five languages.

New icons: add the Lucide name to `tools/make_lucide_subset.py`, run the script (instructions in the script) and copy the code point into `theme::icon`. `theme::tests::every_icon_is_in_the_subset` checks this.

The README picture (`screenshots/veritas-main.png`, dark `veritas-main-dark.png`) is made by a script from the example data in `docs/examples/`: `pwsh tools/screenshot.ps1` builds a release and runs `veritas --screenshot <picture.png> [--theme light|dark] [--lang de|en|fr|es|it] [--verify] [--size WxH] [--scale F] <folder>` (tested on Windows). The picture shows the English interface (`-Lang en`, the script's default); `-Lang de` etc. records in another language under the same file names. It briefly opens a window without focus; it simulates no input and changes no settings. Renew it when the interface changes visibly.

## Packages

The scripts CI uses, after `cargo build --release`:

```bash
pwsh packaging/windows/build.ps1   # dist/windows: portable folder + zip, NSIS installer
packaging/linux/build.sh           # dist/linux: .deb and AppImage – on Ubuntu 24.04
```

`build.ps1` needs `cargo install cargo-packager --locked`; the C runtime is linked statically (`.cargo/config.toml`), so no redistributable DLL goes into the package. `build.sh` needs `cargo-deb`. `packaging/windows/test-installer.ps1` installs and uninstalls in `%LOCALAPPDATA%\Veritas` – meant for CI only.

## Continuous Integration

[`.github/workflows/ci.yml`](.github/workflows/ci.yml) has these jobs:

- **Lint workflows** (always) – `actionlint` (with `shellcheck` over every `run:` block) and `zizmor` for the workflow's security.
- **Lint & test (Linux)** (always) – `scripts/quality.sh all` (guard check against weakened checks, `cargo fmt --check`, `cargo clippy`, `cargo test` with coverage, `osv-scanner`) and `cargo deny check` on Ubuntu 24.04.
- **Test & build (Windows)** – `cargo clippy` and `cargo test`. In a public repository on every push and pull request; in a private one (a private fork, say), where every minute counts (twice as much on Windows), only on `v*` tags and manual runs (*Actions → CI → Run workflow* or `gh workflow run CI`). In that case, test on Windows yourself before pushing.
- **Package** and **Install test** – the Windows installer and zip, the AppImage and the `.deb`, with install tests on Ubuntu 24.04 and Debian 13. Under the same conditions, but never for pull requests.
- **Draft release** – only for a `v*` tag that equals the version in `Cargo.toml`: checksums, the notes from the version's `CHANGELOG.md` section and, in a public repository, a build provenance attestation (`gh attestation verify <file> --repo fly2nbc-oss/Veritas`).

If you change `packaging/`, `build.rs` or the dependencies in a private repository, start a manual run before tagging.

Every action is pinned to a commit, the version follows as a comment. Dependabot (`.github/dependabot.yml`) proposes new actions and dependencies once a month in grouped pull requests; CI checks each one.

## Cross-compilation

Building on the target system is the most reliable way. Windows from Linux works with MinGW and glutin's OpenGL dependencies (`rustup target add x86_64-pc-windows-gnu`, then `cargo build --release --target x86_64-pc-windows-gnu`, or with [cross](https://github.com/cross-rs/cross)); macOS needs the macOS SDK and in practice only works on a Mac. None of these routes is tested.

## Pull requests

- Keep changes focused on one concern where possible.
- Describe **what** changes and **why** in the description.
- For UI changes, attach a screenshot, light and dark if you can.
- `scripts/quality.sh all` must pass; new error cases get a test.
- Add an entry to `CHANGELOG.md` under *Unreleased*.

## Code of conduct

Participants are expected to follow our [`CODE_OF_CONDUCT.md`](./CODE_OF_CONDUCT.md).

## License

By contributing, you agree that your contributions are licensed under the project's [Apache-2.0 license](./LICENSE).
