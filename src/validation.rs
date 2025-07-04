use soroban_sdk::{panic_with_error, Address, Env};
use crate::{
    errors::VaultError,
    math::{self, SCALAR_7},
    storage,
};

/// Validates that an amount is positive (greater than zero)
pub fn require_positive_amount(env: &Env, amount: i128) {
    if amount <= 0 {
        panic_with_error!(env, VaultError::InvalidAmount);
    }
}

/// Validates that a strategy is authorized (was registered at deployment)
pub fn require_authorized_strategy(env: &Env, strategy: &Address) {
    let strategies = storage::get_strategies(env);
    if !strategies.contains(strategy) {
        panic_with_error!(env, VaultError::UnauthorizedStrategy);
    }
}

/// Validates that sufficient vault liquidity exists for an operation
pub fn require_sufficient_liquidity(env: &Env, requested_amount: i128, vault_balance: i128) {
    let total_tokens = storage::get_total_tokens(env);
    let min_liquidity_rate = storage::get_min_liquidity_rate(env);

    let required_liquidity = math::calculate_min_liquidity(env, total_tokens, min_liquidity_rate);

    if vault_balance <= required_liquidity {
        panic_with_error!(env, VaultError::InsufficientVaultBalance);
    }

    let available_liquidity = vault_balance - required_liquidity;
    if requested_amount > available_liquidity {
        panic_with_error!(env, VaultError::InsufficientVaultBalance);
    }
}

/// Validates that a user doesn't have an existing redemption request
pub fn require_no_pending_redemption(env: &Env, user: &Address) {
    if storage::has_redemption_request(env, user) {
        panic_with_error!(env, VaultError::RedemptionInProgress);
    }
}

/// Validates rates are within valid range (0-100%)
pub fn validate_rate(env: &Env, rate: i128) {
    if rate < 0 || rate > SCALAR_7 {
        panic_with_error!(env, VaultError::InvalidAmount);
    }
}

/// Validates that a redemption can be executed (unlock time passed)
pub fn require_redemption_unlocked(env: &Env, unlock_time: u64) {
    if env.ledger().timestamp() < unlock_time {
        panic_with_error!(env, VaultError::RedemptionLocked);
    }
}

/// Validates repayment amount doesn't exceed borrowed amount
pub fn require_valid_repayment(env: &Env, amount: i128, borrowed: i128) {
    if amount > borrowed {
        panic_with_error!(env, VaultError::InvalidAmount);
    }
}

/// Validates that a calculated amount is positive
pub fn require_positive_result(env: &Env, amount: i128) {
    if amount <= 0 {
        panic_with_error!(env, VaultError::InvalidAmount);
    }
}