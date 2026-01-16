//! Strategy integration and custom vault extensions

use soroban_sdk::{contracterror, contractevent, panic_with_error, token, Address, Env};
use stellar_tokens::vault::Vault;

use crate::storage;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum StrategyVaultError {
    InvalidAmount = 420,
    SharesLocked = 421,
    UnauthorizedStrategy = 422,
}
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrategyWithdraw {
    #[topic]
    pub strategy: Address,
    pub amount: i128,
    pub new_net_impact: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrategyDeposit {
    #[topic]
    pub strategy: Address,
    pub amount: i128,
    pub new_net_impact: i128,
}
pub struct StrategyVault;

impl StrategyVault {
    /// Returns true if user's shares are currently locked
    /// Users with no deposit history are NOT locked (they received shares via transfer)
    pub fn is_locked(e: &Env, user: &Address) -> bool {
        let Some(last_deposit_time) = storage::get_last_deposit_time(e, user) else {
            return false; // No deposit history = not locked (received via transfer)
        };
        let unlock_time = last_deposit_time.saturating_add(storage::get_lock_time(e));
        e.ledger().timestamp() < unlock_time
    }

    /// Strategy withdraws tokens from the vault
    /// This decreases total_assets and thus the share price
    pub fn withdraw(env: &Env, strategy: &Address, amount: i128) {
        if amount <= 0 {
            panic_with_error!(env, StrategyVaultError::InvalidAmount);
        }
        if !storage::get_strategies(env).contains(strategy) {
            panic_with_error!(env, StrategyVaultError::UnauthorizedStrategy);
        }

        let asset = Vault::query_asset(env);
        let token_client = token::Client::new(env, &asset);

        // Transfer tokens from vault to strategy
        token_client.transfer(&env.current_contract_address(), strategy, &amount);

        // Update strategy net impact tracking
        let net_impact = storage::get_strategy_net_impact(env, strategy) - amount;
        storage::set_strategy_net_impact(env, strategy, net_impact);

        emit_strategy_withdraw(env, strategy.clone(), amount, net_impact);
    }

    /// Strategy deposits tokens to the vault
    /// This increases total_assets and thus the share price
    pub fn deposit(env: &Env, strategy: &Address, amount: i128) {
        if amount <= 0 {
            panic_with_error!(env, StrategyVaultError::InvalidAmount);
        }
        if !storage::get_strategies(env).contains(strategy) {
            panic_with_error!(env, StrategyVaultError::UnauthorizedStrategy);
        }

        let asset = Vault::query_asset(env);
        let token_client = token::Client::new(env, &asset);

        // Transfer tokens from strategy to vault
        token_client.transfer(strategy, &env.current_contract_address(), &amount);

        // Update strategy net impact tracking
        let net_impact = storage::get_strategy_net_impact(env, strategy) + amount;
        storage::set_strategy_net_impact(env, strategy, net_impact);

        emit_strategy_deposit(env, strategy.clone(), amount, net_impact);
    }
}

fn emit_strategy_withdraw(e: &Env, strategy: Address, amount: i128, new_net_impact: i128) {
    StrategyWithdraw { strategy, amount, new_net_impact }.publish(e);
}

fn emit_strategy_deposit(e: &Env, strategy: Address, amount: i128, new_net_impact: i128) {
    StrategyDeposit { strategy, amount, new_net_impact }.publish(e);
}
