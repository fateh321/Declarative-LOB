### Declarative LOB — Quickstart

This repository contains:
- `LOB/PintLOB/` — a Pint-based declarative orderbook contract (`.pnt`) and its std library
- `LOB/Solver/` — a Rust solver that proposes solution to Pint smart contract with tests against the orderbook logic
- `LOB/SolidityLOB/` - a reference declarative orderbook implementation in solidity
Follow these steps to get set up and run tests.

---

### 1) Install Pint (compiler/CLI)

This project uses the Pint toolchain to build the contract defined in `LOB/PintLOB/orderbook`.

- We use Pint version **0.13.0**. Make sure your `pint --version` reports `0.13.0`.
- Installation (macOS/Linux):
  1. Download the Pint 0.13.0 binary for your OS/architecture from your team’s distribution or the official releases (https://essential-contributions.github.io/pint/book/the-book-of-pint.html). You can also follow the official guide: The Book of Pint → Installation.
  2. Make it executable and place it on your `PATH` (e.g., `/usr/local/bin`):

```bash
chmod +x ./pint          # grant execute permission to the downloaded binary
sudo mv ./pint /usr/local/bin/pint  # move onto PATH so your shell can find it
pint --version           # should print 0.13.0
```

If you run into issues, ensure Pint is on your `PATH` and restart your shell.

---

### 2) Install Rust (via rustup)

Install the Rust toolchain using `rustup`:

```bash
# macOS/Linux
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# After install, ensure your current shell sees cargo/rustc
source "$HOME/.cargo/env"

# Verify
rustc --version
cargo --version
```

This project uses release builds for tests; no nightly components are required.

---

### 3) Build the Pint contract

From the repository root:

```bash
cd LOB/PintLOB/orderbook
pint build
```

Notes:
- Entry point is configured in `pint.toml` as `pt_priority_orderbook.pnt`.
- Ensure the `std` dependency path in `pint.toml` resolves (it points to `../std`). The default layout in this repo already matches that.

If the build succeeds, Pint will output build artifacts and a success message.

---

### 4) Run Solver tests (Rust)

From the repository root:

```bash
cd LOB/Solver
# Run all tests (release, show test output)
cargo test --release -- --nocapture

# Run a single test by name (substring match), e.g. `test_add_limit_order`
cargo test test_add_limit_order --release -- --nocapture
```

- `--release` builds with optimizations (faster).
- `-- --nocapture` prints `println!` output from tests to the console.

---

### Troubleshooting

- Pint not found: Make sure Pint is installed and available on your `PATH` (reopen your terminal or `source` your shell profile).
- Pint build errors: Confirm you are in `LOB/PintLOB/orderbook` when running `pint build`, and that `std` is available at `LOB/PintLOB/std`.
- Rust toolchain: If `cargo` is missing after installing rustup, run `source "$HOME/.cargo/env"` in your shell.

---

### Repository layout (key paths)

- `LOB/PintLOB/orderbook/pint.toml` — Pint package config (entry point + deps)
- `LOB/PintLOB/orderbook/src/pt_priority_orderbook.pnt` — contract entry point
- `LOB/PintLOB/std/` — Pint std library used by the contract
- `LOB/Solver/` — Rust crate with tests (`cargo test`)
