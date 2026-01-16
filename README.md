# Soroban Vault

ERC-4626 compliant tokenized vault built on [OpenZeppelin Stellar Contracts](https://github.com/OpenZeppelin/stellar-contracts) with deposit-based locking and strategy integration.

## Features

### Deposit Lock

Users must wait `lock_time` seconds after their last deposit before withdrawing or redeeming shares. This prevents atomic arbitrage attacks where an attacker could:

1. Deposit into the vault
2. Trigger a profitable strategy action in the same transaction
3. Immediately withdraw with the profits

By enforcing a time delay, arbitrageurs cannot exploit price movements within a single transaction. A short lock time of 15 minutes is recommended - long enough to prevent atomic exploitation while minimizing inconvenience for legitimate users.

### Transfer Restriction

Locked users cannot transfer their shares. This closes an exploit where:

1. Account A deposits funds (becomes locked)
2. Account A transfers shares to Account B
3. Account B (not locked) immediately withdraws

By blocking transfers while locked, users cannot bypass the time-lock by moving shares to another account. Users who receive shares via transfer (without depositing) are not locked and can withdraw immediately.

### Strategy Integration

Authorized strategy contracts can withdraw funds from the vault to deploy in external protocols, and deposit returns back. These operations directly affect `total_assets` and thus the share price. Each strategy's cumulative net impact (P&L) is tracked.

## Structure

```
src/
├── lib.rs        # Module exports
├── contract.rs   # VaultContract with FungibleToken/FungibleVault implementations
├── storage.rs    # Storage keys and persistence functions
├── strategy.rs   # StrategyVault, StrategyVaultError, and events
└── test.rs       # Unit tests
```
