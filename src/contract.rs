//! Vault Contract - ERC-4626 compliant tokenized vault with deposit-based locking
//!
//! This contract implements the OpenZeppelin FungibleVault trait with a simple
//! deposit lock mechanism: users must wait lock_time seconds after their last
//! deposit before they can withdraw or redeem.

use soroban_sdk::{contract, contractimpl, panic_with_error, Address, Env, MuxedAddress, Vec, String};
use stellar_tokens::{
    fungible::{Base, FungibleToken},
    vault::{FungibleVault, Vault},
};

use crate::{
    storage,
    strategy::{StrategyVault, StrategyVaultError},
};

#[contract]
pub struct VaultContract;

#[contractimpl]
impl VaultContract {
    /// Initializes the vault
    ///
    /// # Arguments
    /// * `name` - Name for the vault share token
    /// * `symbol` - Symbol for the vault share token
    /// * `asset` - Address of the underlying token contract
    /// * `decimals_offset` - Virtual offset for inflation attack protection (0-10)
    /// * `strategies` - List of authorized strategy contract addresses
    /// * `lock_time` - Delay in seconds before redemptions/withdrawals can be executed
    pub fn __constructor(
        e: Env,
        name: String,
        symbol: String,
        asset: Address,
        decimals_offset: u32,
        strategies: Vec<Address>,
        lock_time: u64,
    ) {
        Vault::set_asset(&e, asset);
        Vault::set_decimals_offset(&e, decimals_offset);
        Base::set_metadata(&e, Vault::decimals(&e), name, symbol);

        // Initialize custom storage
        storage::set_lock_time(&e, &lock_time);
        storage::set_strategies(&e, &strategies);

        // Initialize strategies
        for strategy_addr in strategies.iter() {
            storage::set_strategy_net_impact(&e, &strategy_addr, 0);
        }
    }

    /// Returns the net impact (cumulative P&L) for a strategy
    pub fn net_impact(e: Env, strategy: Address) -> i128 {
        storage::extend_instance(&e);
        storage::get_strategy_net_impact(&e, &strategy)
    }

    /// Returns the lock time in seconds
    pub fn lock_time(e: Env) -> u64 {
        storage::extend_instance(&e);
        storage::get_lock_time(&e)
    }

    /// Returns true if user's shares are currently locked
    /// Users with no deposit history are considered locked (prevents transfer exploit)
    pub fn is_locked(e: Env, user: Address) -> bool {
        storage::extend_instance(&e);
        StrategyVault::is_locked(&e, &user)
    }

    /// Strategy withdraws tokens from the vault (decreases total_assets and share price)
    pub fn strategy_withdraw(e: Env, strategy: Address, amount: i128) {
        strategy.require_auth();
        StrategyVault::withdraw(&e, &strategy, amount);
        storage::extend_instance(&e);
    }

    /// Strategy deposits tokens to the vault (increases total_assets and share price)
    pub fn strategy_deposit(e: Env, strategy: Address, amount: i128) {
        strategy.require_auth();
        StrategyVault::deposit(&e, &strategy, amount);
        storage::extend_instance(&e);
    }
}

// Implement FungibleToken trait for share token functionality
#[contractimpl(contracttrait)]
impl FungibleToken for VaultContract {
    type ContractType = Vault;

    /// Override: Block transfer if sender is locked
    fn transfer(e: &Env, from: Address, to: MuxedAddress, amount: i128) {
        if StrategyVault::is_locked(e, &from) {
            panic_with_error!(e, StrategyVaultError::SharesLocked);
        }
        Base::transfer(e, &from, &to, amount);
    }

    /// Override: Block transfer_from if sender is locked
    fn transfer_from(e: &Env, spender: Address, from: Address, to: Address, amount: i128) {
        if StrategyVault::is_locked(e, &from) {
            panic_with_error!(e, StrategyVaultError::SharesLocked);
        }
        Base::transfer_from(e, &spender, &from, &to, amount);
    }
}

// Implement FungibleVault trait for ERC-4626 functionality
// Override deposit/mint to track timestamps, and redeem/withdraw to check lock
#[contractimpl(contracttrait)]
impl FungibleVault for VaultContract {
    /// Override: Track deposit timestamp for the receiver (who gets the shares)
    fn deposit(e: &Env, assets: i128, receiver: Address, from: Address, operator: Address) -> i128 {
        let shares = Vault::deposit(e, assets, receiver.clone(), from, operator);
        storage::set_last_deposit_time(e, &receiver, e.ledger().timestamp());
        storage::extend_instance(e);
        shares
    }

    /// Override: Track mint timestamp for the receiver (who gets the shares)
    fn mint(e: &Env, shares: i128, receiver: Address, from: Address, operator: Address) -> i128 {
        let assets = Vault::mint(e, shares, receiver.clone(), from, operator);
        storage::set_last_deposit_time(e, &receiver, e.ledger().timestamp());
        storage::extend_instance(e);
        assets
    }

    /// Override: Validate lock expired before redemption
    fn redeem(e: &Env, shares: i128, receiver: Address, owner: Address, operator: Address) -> i128 {
        if StrategyVault::is_locked(e, &owner) {
            panic_with_error!(e, StrategyVaultError::SharesLocked);
        }
        let assets = Vault::redeem(e, shares, receiver, owner, operator);
        storage::extend_instance(e);
        assets
    }

    /// Override: Validate lock expired before withdrawal
    fn withdraw(e: &Env, assets: i128, receiver: Address, owner: Address, operator: Address) -> i128 {
        if StrategyVault::is_locked(e, &owner) {
            panic_with_error!(e, StrategyVaultError::SharesLocked);
        }
        let shares = Vault::withdraw(e, assets, receiver, owner, operator);
        storage::extend_instance(e);
        shares
    }

    /// Override: Returns 0 if locked, otherwise returns user's full redeemable balance
    fn max_redeem(e: &Env, owner: Address) -> i128 {
        if StrategyVault::is_locked(e, &owner) {
            0
        } else {
            Vault::max_redeem(e, owner)
        }
    }

    /// Override: Returns 0 if locked, otherwise returns value of user's shares
    fn max_withdraw(e: &Env, owner: Address) -> i128 {
        if StrategyVault::is_locked(e, &owner) {
            0
        } else {
            Vault::max_withdraw(e, owner)
        }
    }
}
