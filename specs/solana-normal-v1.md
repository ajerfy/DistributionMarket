# Solana Normal-Only V1 Market Spec

## Scope

This v1 spec is intentionally Normal-only. The goal is to ship the smallest Solana-compatible version of the market with deterministic fixed-point market state, deterministic Gaussian math, explicit account layouts, and a clean offchain-quote/onchain-verify workflow.

This spec treats the fixed-point Normal engine in [`/Users/aaditjerfy/distribution-markets/src/normal_market.rs`](/Users/aaditjerfy/distribution-markets/src/normal_market.rs) and [`/Users/aaditjerfy/distribution-markets/src/normal_math.rs`](/Users/aaditjerfy/distribution-markets/src/normal_math.rs) as the canonical economic core for v1.

## Economic Model

- Outcome family: Normal distributions only, parameterized by `(mu, sigma)`.
- Invariant target: the market tracks a fixed-point Normal distribution scaled by `lambda` implied by the L2 constraint `k`.
- Collateral model: trades post collateral against the deterministic fixed-grid maximum-loss search currently implemented for the Normal path.
- LP model: LPs hold fungible pro rata shares of remaining market value; they do not mint ERC721-style positions.
- Settlement model: market resolves once to a scalar realized outcome; traders settle against stored old/new Normal distributions and LPs settle against remaining funds, with any fixed-point rounding dust tracked explicitly as residual cash.

## Accounts

### `Market` PDA

Represents the canonical market state.

Fields:
- `version`
- `bump`
- `status`
- `collateral_mint`
- `collateral_vault`
- `lp_mint`
- `market_authority`
- `oracle_config`
- `b`
- `k`
- `current_distribution`
- `current_lambda`
- `total_lp_shares`
- `total_trades`
- `resolved_outcome`
- `created_slot`
- `resolved_slot`

### `Position` PDA

Represents one trader or liquidity position.

Fields:
- `version`
- `bump`
- `market`
- `owner`
- `side`
- `id`
- `old_distribution`
- `new_distribution`
- `collateral_posted`
- `lp_shares`
- `settled`
- `payout_claimed`
- `created_slot`
- `settled_slot`

### `Collateral Vault`

SPL token account owned by the market authority PDA. Holds all market backing and trader collateral.

### `LP Mint`

Fungible SPL mint representing LP claims on remaining market funds.

## Instruction Set

### `InitializeMarket`

Creates the Market PDA, collateral vault, LP mint, and initializes the active Normal distribution.

Payload:
- `initial_b`
- `initial_k`
- `initial_distribution`
- `oracle_config`

Checks:
- positive `b` and `k`
- positive `sigma`
- sigma floor consistent with solvency
- peak density does not exceed backing

### `Trade`

Moves the market from the current Normal distribution to a new one.

Payload:
- `quote`

The quote envelope includes:
- expected market version
- new Normal distribution
- collateral required
- max slippage collateral
- search lower/upper bounds
- coarse and refine sample counts
- quote slot
- quote expiry slot

Checks:
- market is active
- quote not expired
- market version matches
- proposed `sigma` respects the floor
- verified deterministic collateral does not exceed user tolerance
- quote search parameters match the program's bounded verifier policy

Effects:
- transfer collateral to vault
- create Position PDA
- advance current market distribution and lambda
- increment trade counter

### `ManageLiquidity(Add)`

Adds backing to the market and mints LP shares proportionally.

Payload:
- `owner`
- `amount_or_shares`

Checks:
- market is active
- amount is positive
- proportional scaling preserves Normal state

Effects:
- transfer backing into vault
- scale `b`, `k`, and `lambda`
- mint LP shares

### `ManageLiquidity(Remove)`

Burns LP shares and withdraws the corresponding backing while the market is still active.

Payload:
- `owner`
- `amount_or_shares`

Checks:
- market is active
- owner has enough LP shares
- resulting market state remains valid

Effects:
- burn LP shares
- transfer backing out of vault
- scale `b`, `k`, and `lambda` downward

### `ResolveMarket`

Resolves the market once using the configured oracle.

Payload:
- `outcome`
- `oracle_observation_slot`

Checks:
- market is active
- authorized oracle path
- single-use resolution

Effects:
- set resolved outcome
- set resolved slot
- freeze further trades and liquidity changes

### `SettlePosition`

Settles one trader position against the resolved outcome.

Payload:
- `position_id`

Checks:
- market resolved
- position belongs to signer
- position not previously settled

Effects:
- recompute fixed-point payout from stored old/new distributions
- transfer payout from vault
- mark position settled

### `SettleLp`

Settles an LP holder’s remaining claim after trader settlements.

Payload:
- `owner`

Checks:
- market resolved
- owner holds LP shares

Effects:
- compute pro rata remaining value
- burn LP shares or checkpoint claim
- transfer payout from vault

## Quote / Verify Model

V1 should not make the program do expensive open-ended search. The intended flow is:

1. Client reads current market state.
2. Client computes candidate Normal trade and required collateral offchain.
3. Client submits a quote envelope with the proposed new distribution and collateral.
4. Program re-runs a deterministic bounded verification search and rejects the trade if the provided collateral is insufficient or stale.

This gives deterministic acceptance while keeping the instruction surface predictable.

The current verifier design in the repo uses:
- a deterministic bounded search window derived from `(mu, sigma)`
- a coarse search pass plus a local refinement pass
- conservative padding on the required collateral

## Mapping From Current Fixed-Point Market Operations

The current fixed-point market engine maps onto Solana instructions as follows:

- `FixedNormalMarket::new` -> `InitializeMarket`
- `FixedNormalMarket::trade` -> `Trade`
- `FixedNormalMarket::add_liquidity` -> `ManageLiquidity(Add)`
- `FixedNormalMarket::remove_liquidity` -> `ManageLiquidity(Remove)`
- `FixedNormalMarket::resolve` (market outcome write) -> `ResolveMarket`
- `FixedNormalMarket::resolve` (trade payout branch) -> `SettlePosition`
- `FixedNormalMarket::resolve` (LP payout branch) -> `SettleLp`

## Immediate Build Target

The next code step after this spec should be to:
- add a fixed-point `remove_liquidity` path to the Normal engine
- add quote-envelope verification helpers for the trade instruction
- implement these account and instruction types in a Solana-facing crate or module
- carry the current bounded verifier policy directly into program-side trade verification logic
