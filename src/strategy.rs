use soroban_sdk::{token, Address, Env};
use crate::{
    events::VaultEvents,
    storage,
    validation,
};

pub fn borrow(env: &Env, strategy: &Address, amount: i128) {
    validation::require_positive_amount(env, amount);
    validation::require_authorized_strategy(env, strategy);

    // Get token client once
    let token = storage::get_token(env);
    let token_client = token::Client::new(env, &token);
    let vault_address = env.current_contract_address();

    // Check liquidity
    let vault_balance = token_client.balance(&vault_address);
    validation::require_sufficient_liquidity(env, amount, vault_balance);

    // Transfer tokens
    token_client.transfer(&vault_address, strategy, &amount);

    // Update strategy data
    let mut strategy_data = storage::get_strategy_data(env, strategy);
    strategy_data.borrowed += amount;
    storage::set_strategy_data(env, strategy, &strategy_data);

    // Emit event
    VaultEvents::borrow(env, strategy.clone(), amount, strategy_data.borrowed);
}

pub fn repay(env: &Env, strategy: &Address, amount: i128) {
    validation::require_positive_amount(env, amount);
    validation::require_authorized_strategy(env, strategy);

    // Get and validate strategy data
    let mut strategy_data = storage::get_strategy_data(env, strategy);
    validation::require_valid_repayment(env, amount, strategy_data.borrowed);

    // Transfer tokens
    let token = storage::get_token(env);
    let token_client = token::Client::new(env, &token);
    token_client.transfer(strategy, &env.current_contract_address(), &amount);

    // Update strategy data
    strategy_data.borrowed -= amount;
    storage::set_strategy_data(env, strategy, &strategy_data);

    // Emit event
    VaultEvents::repay(env, strategy.clone(), amount, strategy_data.borrowed);
}

pub fn transfer_to(env: &Env, strategy: &Address, amount: i128) {
    // Validate
    validation::require_positive_amount(env, amount);
    validation::require_authorized_strategy(env, strategy);

    // Transfer tokens
    let token = storage::get_token(env);
    let token_client = token::Client::new(env, &token);
    token_client.transfer(&env.current_contract_address(), strategy, &amount);

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

pub fn transfer_from(env: &Env, strategy: &Address, amount: i128) {
    // Validate
    validation::require_positive_amount(env, amount);
    validation::require_authorized_strategy(env, strategy);

    // Transfer tokens
    let token = storage::get_token(env);
    let token_client = token::Client::new(env, &token);
    token_client.transfer(strategy, &env.current_contract_address(), &amount);

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