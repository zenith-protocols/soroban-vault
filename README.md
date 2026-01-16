# Strategy Vault

ERC-4626 compliant tokenized vault built on [OpenZeppelin Stellar Contracts](https://github.com/OpenZeppelin/stellar-contracts) with deposit-based locking and strategy integration.

## Features

### Deposit Lock

Users must wait `lock_time` seconds after their last deposit before withdrawing, redeeming, or transferring shares. This prevents atomic arbitrage attacks by enforcing a time delay between deposits and exits. A short lock time (e.g., 15 minutes) is recommended.

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
