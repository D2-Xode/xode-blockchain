# orml-vesting (vendored)

Source: https://github.com/open-web3-stack/open-runtime-module-library
Branch: `polkadot-stable2512`
Commit: `f389cbdb1d37f4113a3784d72542aa080beb299c`
License: Apache-2.0 (see `LICENSE`)

## Why vendored

The ORML repository has no root `Cargo.toml` (only `Cargo.dev.toml`), and its crates
declare dependencies with `workspace = true`. A plain git dependency therefore fails to
resolve. The crate on crates.io (`orml-vesting 1.5.0`) is built against polkadot-sdk
stable2506 (`frame-support 41`), not the stable2512 versions this workspace uses.

## Local changes

- `src/*.rs`: unchanged from the commit above.
- `Cargo.toml`: dependencies point at this workspace instead of ORML's
  `Cargo.dev.toml`; `publish = false` added.

## Updating

When the workspace moves to a new polkadot-sdk release, copy `vesting/src/*` from the
matching ORML `polkadot-stableXXXX` branch, update the commit above, and re-check the
`Config` trait against `runtime/src/configs/mod.rs`.
