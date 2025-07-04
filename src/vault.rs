use soroban_sdk::{token, Address, Env};
use crate::{
    events::VaultEvents,
    math::{self, Converter},
    storage::{self, RedemptionRequest},
    validation,
};

/// Deposits tokens and mints shares
pub fn deposit(env: &Env, tokens: i128, receiver: &Address, owner: &Address) -> i128 {
    validation::require_positive_amount(env, tokens);

    // Get current state
    let total_shares = storage::get_total_shares(env);
    let total_tokens = storage::get_total_tokens(env);

    // Calculate shares
    let converter = Converter::new(env, total_shares, total_tokens);
    let shares = converter.shares_from_tokens(tokens);

    // Execute transfers
    let token_client = token::Client::new(env, &storage::get_token(env));
    token_client.transfer(owner, &env.current_contract_address(), &tokens);

    let share_token = storage::get_share_token(env);
    token::StellarAssetClient::new(env, &share_token).mint(receiver, &shares);

    // Update state
    storage::set_total_shares(env, &(total_shares + shares));
    storage::set_total_tokens(env, &(total_tokens + tokens));

    // Emit event
    VaultEvents::deposit(env, owner.clone(), receiver.clone(), shares, tokens);

    shares
}

/// Mints exact shares by depositing tokens
pub fn mint(env: &Env, shares: i128, receiver: &Address, owner: &Address) -> i128 {
    validation::require_positive_amount(env, shares);

    // Get current state
    let total_shares = storage::get_total_shares(env);
    let total_tokens = storage::get_total_tokens(env);

    // Calculate tokens required
    let converter = Converter::new(env, total_shares, total_tokens);
    let tokens = converter.tokens_from_shares(shares);

    // Execute transfers
    let token_client = token::Client::new(env, &storage::get_token(env));
    token_client.transfer(owner, &env.current_contract_address(), &tokens);

    let share_token = storage::get_share_token(env);
    token::StellarAssetClient::new(env, &share_token).mint(receiver, &shares);

    // Update state
    storage::set_total_shares(env, &(total_shares + shares));
    storage::set_total_tokens(env, &(total_tokens + tokens));

    // Emit event
    VaultEvents::mint(env, owner.clone(), receiver.clone(), shares, tokens);

    tokens
}

/// Queues a redemption request
pub fn request_redeem(env: &Env, shares: i128, owner: &Address) {
    validation::require_positive_amount(env, shares);
    validation::require_no_pending_redemption(env, owner);

    // Lock shares in vault
    let share_token = storage::get_share_token(env);
    let share_client = token::Client::new(env, &share_token);
    share_client.transfer(owner, &env.current_contract_address(), &shares);

    // Create redemption request
    let lock_time = storage::get_lock_time(env);
    let unlock_time = env.ledger().timestamp() + lock_time;
    let request = RedemptionRequest { shares, unlock_time };

    storage::set_redemption_request(env, owner, &request);

    // Emit event
    VaultEvents::request_redeem(env, owner.clone(), shares, unlock_time);
}

/// Executes a redemption after lock period
pub fn redeem(env: &Env, receiver: &Address, owner: &Address) -> i128 {
    // Get and validate request
    let request = storage::get_redemption_request(env, owner);
    validation::require_redemption_unlocked(env, request.unlock_time);

    let shares = request.shares; // Use all shares from the request

    // Calculate redemption value
    let total_shares = storage::get_total_shares(env);
    let total_tokens = storage::get_total_tokens(env);
    let converter = Converter::new(env, total_shares, total_tokens);
    let tokens = converter.redemption_value(shares);

    // Execute transfers
    let share_token = storage::get_share_token(env);
    let share_client = token::Client::new(env, &share_token);
    share_client.burn(&env.current_contract_address(), &shares);

    let token_client = token::Client::new(env, &storage::get_token(env));
    token_client.transfer(&env.current_contract_address(), receiver, &tokens);

    // Update state
    storage::set_total_shares(env, &(total_shares - shares));
    storage::set_total_tokens(env, &(total_tokens - tokens));
    storage::remove_redemption_request(env, owner);

    // Emit event
    VaultEvents::redeem(env, owner.clone(), receiver.clone(), shares, tokens);

    tokens
}

/// Emergency redemption with penalty
pub fn emergency_redeem(env: &Env, receiver: &Address, owner: &Address) -> i128 {
    let request = storage::get_redemption_request(env, owner);

    // Calculate value
    let total_shares = storage::get_total_shares(env);
    let total_tokens = storage::get_total_tokens(env);
    let converter = Converter::new(env, total_shares, total_tokens);
    let current_tokens = converter.redemption_value(request.shares);

    // Calculate penalty
    let lock_time = storage::get_lock_time(env);
    let penalty_rate = storage::get_penalty_rate(env);
    let penalty_amount = math::calculate_penalty(
        env,
        current_tokens,
        request.unlock_time,
        lock_time,
        penalty_rate,
    );

    let withdrawal_amount = current_tokens - penalty_amount;
    validation::require_positive_result(env, withdrawal_amount);

    // Execute withdrawal
    let share_token = storage::get_share_token(env);
    let share_client = token::Client::new(env, &share_token);
    share_client.burn(&env.current_contract_address(), &request.shares);

    let token_client = token::Client::new(env, &storage::get_token(env));
    token_client.transfer(&env.current_contract_address(), receiver, &withdrawal_amount);

    // Update state (penalty stays in vault)
    storage::set_total_shares(env, &(total_shares - request.shares));
    storage::set_total_tokens(env, &(total_tokens - withdrawal_amount));
    storage::remove_redemption_request(env, owner);

    // Emit event
    VaultEvents::emergency_redeem(env, owner.clone(), receiver.clone(), request.shares, withdrawal_amount, penalty_amount);

    withdrawal_amount
}

/// Cancels a redemption request
pub fn cancel_redeem(env: &Env, owner: &Address) {
    let request = storage::get_redemption_request(env, owner);

    // Return shares to owner
    let share_token = storage::get_share_token(env);
    let share_client = token::Client::new(env, &share_token);
    share_client.transfer(&env.current_contract_address(), owner, &request.shares);

    storage::remove_redemption_request(env, owner);

    // Emit event
    VaultEvents::cancel_redeem(env, owner.clone(), request.shares);
}