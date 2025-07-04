use soroban_sdk::{token, Address, Env};
use crate::{
    events::VaultEvents,
    storage,
    validation,
};

/// Helper to get vault's token balance
fn get_vault_balance(env: &Env) -> i128 {
    let token_client = token::Client::new(env, &storage::get_token(env));
    token_client.balance(&env.current_contract_address())
}

/// Helper to transfer tokens
fn transfer_tokens(env: &Env, from: &Address, to: &Address, amount: &i128) {
    let token_client = token::Client::new(env, &storage::get_token(env));
    token_client.transfer(from, to, amount);
}

/// Borrows tokens from vault
pub fn borrow(env: &Env, strategy: &Address, amount: i128) {
    // Validate
    validation::require_positive_amount(env, amount);
    validation::require_authorized_strategy(env, strategy);

    // Check liquidity
    let vault_balance = get_vault_balance(env);
    validation::require_sufficient_liquidity(env, amount, vault_balance);

    // Transfer tokens
    transfer_tokens(env, &env.current_contract_address(), strategy, &amount);

    // Update strategy data
    let mut strategy_data = storage::get_strategy_data(env, strategy);
    strategy_data.borrowed += amount;
    storage::set_strategy_data(env, strategy, &strategy_data);

    // Emit event
    VaultEvents::borrow(env, strategy.clone(), amount, strategy_data.borrowed);
}

/// Repays borrowed tokens
pub fn repay(env: &Env, strategy: &Address, amount: i128) {
    // Validate
    validation::require_positive_amount(env, amount);
    validation::require_authorized_strategy(env, strategy);

    // Get and validate strategy data
    let mut strategy_data = storage::get_strategy_data(env, strategy);
    validation::require_valid_repayment(env, amount, strategy_data.borrowed);

    // Transfer tokens
    transfer_tokens(env, strategy, &env.current_contract_address(), &amount);

    // Update strategy data
    strategy_data.borrowed -= amount;
    storage::set_strategy_data(env, strategy, &strategy_data);

    // Emit event
    VaultEvents::repay(env, strategy.clone(), amount, strategy_data.borrowed);
}

/// Transfers tokens to strategy (affects total_tokens)
pub fn transfer_to(env: &Env, strategy: &Address, amount: i128) {
    // Validate
    validation::require_positive_amount(env, amount);
    validation::require_authorized_strategy(env, strategy);

    // Transfer tokens
    transfer_tokens(env, &env.current_contract_address(), strategy, &amount);

    // Update strategy net impact
    let mut strategy_data = storage::get_strategy_data(env, strategy);
    strategy_data.net_impact -= amount;
    storage::set_strategy_data(env, strategy, &strategy_data);

    // Update total tokens
    let total_tokens = storage::get_total_tokens(env);
    storage::set_total_tokens(env, &(total_tokens - amount));

    // Emit event
    VaultEvents::transfer_to(env, strategy.clone(), amount, strategy_data.net_impact);
}

/// Transfers tokens from strategy (affects total_tokens)
pub fn transfer_from(env: &Env, strategy: &Address, amount: i128) {
    // Validate
    validation::require_positive_amount(env, amount);
    validation::require_authorized_strategy(env, strategy);

    // Transfer tokens
    transfer_tokens(env, strategy, &env.current_contract_address(), &amount);

    // Update strategy net impact
    let mut strategy_data = storage::get_strategy_data(env, strategy);
    strategy_data.net_impact += amount;
    storage::set_strategy_data(env, strategy, &strategy_data);

    // Update total tokens
    let total_tokens = storage::get_total_tokens(env);
    storage::set_total_tokens(env, &(total_tokens + amount));

    // Emit event
    VaultEvents::transfer_from(env, strategy.clone(), amount, strategy_data.net_impact);
}