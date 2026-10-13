# Xode Blockchain v0.3.0

**Runtime:** `xode-runtime` spec_version `14` → `15`
**Node:** `xode-node` `0.3.0`

This release adds **Asset Waiting**: governance must approve each new DEX liquidity
pool before anyone can trade on it.

---

## What's added

### Asset Waiting: `pallet-asset-waiting` (pallet index `62`)

New pools no longer go live straight away. Each one waits in a **Pending** state until
the Technical Committee approves it.

| Call | Who can call | What it does |
|---|---|---|
| `AssetWaiting::create_pool(asset1, asset2)` | Any signed account | Creates the pool in `AssetConversion` with status **Pending** |
| `AssetWaiting::approve_pool(asset1, asset2)` | Technical Committee member | Records one approval. After **2 different members** approve, the pool becomes **Active**. |
| `AssetWaiting::reject_pool(asset1, asset2)` | Technical Committee member | One rejection removes the pending pool completely (see below) |

When a pool is rejected:
- Asset deposits held by the pool account are refunded.
- The pool's LP token is destroyed, including its accounts and approvals.
- The pool is removed from both `AssetConversion::Pools` and `AssetWaiting::PoolDetailsById`.

Governance members are read from `TechnicalCommitteeMembership`.

**Events:** `PoolCreatedPending`, `PoolApproved { approvals }`, `PoolActivated`, `PoolRejected`

**Errors:** `InvalidAssetPair`, `PoolNotFound`, `PoolNotPending`, `NotGovernanceMember`,
`AlreadyApproved`, `TooManyApprovals`

### How DEX calls are filtered

Asset Waiting is part of the runtime call filter (`BaseCallFilter = InsideBoth<TxPause, AssetWaiting>`):

| Call | Behaviour |
|---|---|
| `AssetConversion::create_pool` | **Blocked.** Use `AssetWaiting::create_pool` instead. |
| `AssetConversion::add_liquidity` / `remove_liquidity` | Allowed only when the pool is **Active** |
| `AssetConversion::swap_exact_tokens_for_tokens` / `swap_tokens_for_exact_tokens` | Allowed only when **every pool in the swap path** is **Active** |

Pools created before this upgrade have no Asset Waiting record. They are treated as
Active and keep working as before.

---

## What changed

- `spec_version` raised from **14** to **15**.
- Added `pallet-asset-waiting` to the workspace and the runtime, with unit tests and a mock runtime.
- `BaseCallFilter` now includes `AssetWaiting`.

## Impact on integrators

- **Creating pools:** wallets, dApps and scripts that call `AssetConversion::create_pool`
  directly will now be rejected by the call filter. Switch them to
  `AssetWaiting::create_pool`.
- **New pools:** add-liquidity and swap calls on a new pool fail until it has 2 approvals.
- **Metadata:** a new pallet was added at index 62. Clients must refresh metadata.
- **Migrations:** none needed. The pallet starts with empty storage.

---

## Creating the release

1. Merge `xode-freezer-opt-duration` into `main`.
2. Test:
   ```bash
   cargo test -p pallet-asset-waiting
   cargo build --release
   ```
3. Build the artifacts:
   ```bash
   cargo build --release                                  # x86_64 node and runtime Wasm
   cross build --release --target aarch64-unknown-linux-gnu   # aarch64 node
   ```
   For the on-chain upgrade, build a deterministic Wasm with `srtool` and record its Blake2-256 hash.
4. Tag and push:
   ```bash
   git tag -a v0.3.0 -m "Xode v0.3.0 (spec 15): Asset Waiting"
   git push origin v0.3.0
   ```
5. Create the GitHub release from the tag. Use this file as the release body and attach
   the node binaries, `xode_runtime.compact.compressed.wasm`, and its hash.
6. Upgrade the runtime through governance (`system.authorize_upgrade` / `set_code`). Then check:
   - `state_getRuntimeVersion` reports `specVersion: 15`
   - `AssetWaiting` appears in metadata
   - Smoke test: `AssetWaiting::create_pool`, then approve with 2 members (pool becomes Active),
     then `add_liquidity` and a swap. Also create a second pool and reject it, then check
     that it is removed.
