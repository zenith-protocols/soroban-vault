use soroban_sdk::{contract, contractimpl, contractclient, token, panic_with_error, Address, Env, Vec, String, BytesN, };
use soroban_fixed_point_math::SorobanFixedPoint;

use crate::{
    storage::{self, RedemptionRequest, StrategyData},
    errors::VaultError,
    token::create_share_token,
    events::VaultEvents,
};

const SCALAR_7: i128 = 10_000_000;

#[contract]
pub struct VaultContract;

#[contractclient(name = "VaultClient")]
pub trait Vault {
    /// Returns the address of the underlying token managed by this vault
    ///
    /// # Returns
    /// Address of the underlying token contract
    fn token(e: Env) -> Address;

    /// Returns the address of the vault's share token contract
    ///
    /// # Returns
    /// Address of the share token contract
    fn share_token(e: Env) -> Address;

    /// Returns the total number of share tokens in circulation
    ///
    /// # Returns
    /// Total share token supply (with 7 decimal places)
    fn total_shares(e: Env) -> i128;

    /// Returns the total assets under management by the vault
    /// Including borrowed funds by strategies
    ///
    /// # Returns
    /// Total assets under management
    fn total_assets(e: Env) -> i128;

    /// Returns the strategy data for a given strategy address
    ///
    /// # Arguments
    /// * `strategy` - Address of the strategy contract
    ///
    /// # Returns
    /// StrategyData containing borrowed amount and net impact
    fn get_strategy(e: Env, strategy: Address) -> StrategyData;

    /// Deposits underlying tokens and mints share tokens to receiver
    ///
    /// # Arguments
    /// * `tokens` - Amount of underlying tokens to deposit (must be > 0)
    /// * `receiver` - Address to receive the minted share tokens
    ///
    /// # Returns
    /// Amount of share tokens minted to receiver
    ///
    /// # Panics
    /// - `ZeroAmount` if tokens <= 0
    fn deposit(e: Env, tokens: i128, receiver: Address) -> i128;

    /// Mints exact share tokens by depositing underlying tokens
    ///
    /// # Arguments
    /// * `shares` - Exact amount of share tokens to mint (must be > 0)
    /// * `receiver` - Address to receive the minted share tokens
    ///
    /// # Returns
    /// Amount of underlying tokens deposited
    ///
    /// # Panics
    /// - `ZeroAmount` if shares <= 0
    fn mint(e: Env, shares: i128, receiver: Address) -> i128;

    /// Queues a redemption request for exact shares
    ///
    /// # Arguments
    /// * `shares` - Amount of shares to request for redemption (must be > 0)
    /// * `owner` - Address that owns the shares (must authorize transaction)
    ///
    /// # Panics
    /// - `ZeroAmount` if shares <= 0
    /// - `WithdrawalInProgress` if owner already has a pending redemption
    /// - `InsufficientShares` if owner doesn't have enough shares
    fn request_redeem(e: Env, shares: i128, owner: Address);

    /// Executes a queued redemption for exact shares after the delay period
    /// Burns exact shares and returns whatever assets that equals
    ///
    /// # Arguments
    /// * `shares` - Exact amount of shares to redeem
    /// * `receiver` - Address to receive the assets
    /// * `owner` - Address that queued the redemption
    ///
    /// # Returns
    /// Amount of assets transferred
    ///
    /// # Panics
    /// - `WithdrawalLocked` if unlock time hasn't been reached
    fn redeem(e: Env, shares: i128, receiver: Address, owner: Address) -> i128;

    /// Emergency redemption with penalty before delay period ends
    ///
    /// # Arguments
    /// * `owner` - Address that queued the redemption (must authorize transaction)
    ///
    /// # Returns
    /// Amount of underlying assets transferred to owner (after penalty)
    fn emergency_redeem(e: Env, owner: Address) -> i128;

    /// Cancels a pending redemption request
    ///
    /// # Arguments
    /// * `owner` - Address that queued the redemption (must authorize transaction)
    ///
    /// # Panics
    /// - Panics if no redemption request exists for owner
    fn cancel_redeem(e: Env, owner: Address);

    /// Allows a registered strategy to borrow tokens from the vault
    ///
    /// # Arguments
    /// * `strategy` - Address of the strategy contract (must match caller)
    /// * `amount` - Amount of tokens to borrow (must be > 0)
    ///
    /// # Authorization
    /// Requires authorization from the strategy contract
    ///
    /// # Panics
    /// - `ZeroAmount` if amount <= 0
    /// - `UnauthorizedStrategy` if strategy not registered at deployment
    /// - `InsufficientVaultBalance` if vault doesn't have enough liquidity
    fn borrow(e: Env, strategy: Address, amount: i128);

    /// Allows a strategy to repay borrowed tokens
    ///
    /// # Arguments
    /// * `strategy` - Address of the strategy contract (must match caller)
    /// * `amount` - Amount of tokens to repay (must be > 0)
    ///
    /// # Panics
    /// - `ZeroAmount` if amount <= 0
    /// - `UnauthorizedStrategy` if strategy not registered at deployment
    fn repay(e: Env, strategy: Address, amount: i128);

    /// Allows a registered strategy to transfer tokens from the vault
    ///
    /// # Arguments
    /// * `strategy` - Address of the strategy contract (must match caller)
    /// * `amount` - Amount of tokens to transfer (must be > 0)
    ///
    /// # Authorization
    /// Requires authorization from the strategy contract
    ///
    /// # Panics
    /// - `ZeroAmount` if amount <= 0
    /// - `UnauthorizedStrategy` if strategy not registered at deployment
    /// - `InsufficientVaultBalance` if vault doesn't have enough tokens
    fn transfer_to(e: Env, strategy: Address, amount: i128);

    /// Allows a strategy to transfer tokens to the vault
    ///
    /// # Arguments
    /// * `strategy` - Address of the strategy contract (must match caller)
    /// * `amount` - Amount of tokens to transfer (must be > 0)
    ///
    /// # Panics
    /// - `ZeroAmount` if amount <= 0
    /// - `UnauthorizedStrategy` if strategy not registered at deployment
    fn transfer_from(e: Env, strategy: Address, amount: i128);
}

#[contractimpl]
impl VaultContract {
    /// Initializes the immutable vault
    ///
    /// # Arguments
    /// * `token` - Address of the underlying token contract
    /// * `token_wasm_hash` - WASM hash for deploying the share token contract
    /// * `name` - Name for the share token
    /// * `symbol` - Symbol for the share token
    /// * `strategies` - List of strategy contract addresses
    /// * `lock_time` - Delay in seconds before redemptions can be executed
    /// * `penalty_rate` - Penalty rate in SCALAR_7 format (0-100%)
    /// * `min_liquidity_rate` - Minimum liquidity percentage in SCALAR_7 format (0-100%)
    ///
    /// # Panics
    /// - `InvalidAmount` if penalty_rate or min_liquidity_rate < 0 or > 100%
    pub fn __constructor(
        e: Env,
        token: Address,
        token_wasm_hash: BytesN<32>,
        name: String,
        symbol: String,
        strategies: Vec<Address>,
        lock_time: u64,
        penalty_rate: i128,
        min_liquidity_rate: i128,
    ) {
        // Validate penalty rate (0-100% in SCALAR_7)
        if penalty_rate < 0 || penalty_rate > SCALAR_7 {
            panic_with_error!(e, VaultError::InvalidAmount);
        }

        // Validate minimum liquidity rate (0-100% in SCALAR_7)
        if min_liquidity_rate < 0 || min_liquidity_rate > SCALAR_7 {
            panic_with_error!(e, VaultError::InvalidAmount);
        }

        let share_token = create_share_token(&e, token_wasm_hash, &token, &name, &symbol);

        // Store immutable vault configuration
        storage::set_token(&e, &token);
        storage::set_share_token(&e, &share_token);
        storage::set_total_shares(&e, &0);
        storage::set_total_tokens(&e, &0); // Initialize total tokens tracker
        storage::set_lock_time(&e, &lock_time);
        storage::set_penalty_rate(&e, &penalty_rate);
        storage::set_min_liquidity_rate(&e, &min_liquidity_rate);
        storage::set_strategies(&e, &strategies);

        // Initialize all strategies with zero impact and zero borrowed
        for strategy_addr in strategies.iter() {
            let initial_data = StrategyData {
                borrowed: 0,
                net_impact: 0,
            };
            storage::set_strategy_data(&e, &strategy_addr, &initial_data);
        }

        storage::extend_instance(&e);
    }
}

#[contractimpl]
impl Vault for VaultContract {
    fn token(e: Env) -> Address {
        storage::extend_instance(&e);
        storage::get_token(&e)
    }

    fn share_token(e: Env) -> Address {
        storage::extend_instance(&e);
        storage::get_share_token(&e)
    }

    fn total_shares(e: Env) -> i128 {
        storage::extend_instance(&e);
        storage::get_total_shares(&e)
    }

    fn total_assets(e: Env) -> i128 {
        storage::extend_instance(&e);
        storage::get_total_tokens(&e)
    }

    fn get_strategy(e: Env, strategy: Address) -> StrategyData {
        storage::extend_instance(&e);
        storage::get_strategy_data(&e, &strategy)
    }

    fn deposit(e: Env, tokens: i128, receiver: Address) -> i128 {
        receiver.require_auth();
        if tokens <= 0 {
            panic_with_error!(e, VaultError::ZeroAmount);
        }

        let token_addr = storage::get_token(&e);
        let share_token = storage::get_share_token(&e);

        let token_client = token::Client::new(&e, &token_addr);
        let total_shares = storage::get_total_shares(&e);
        let total_tokens = storage::get_total_tokens(&e);

        // Calculate shares to mint
        let shares = if total_shares == 0 || total_tokens == 0 {
            // First deposit gets 1:1 ratio
            tokens
        } else {
            // shares = tokens * (total shares / total tokens)
            tokens.fixed_mul_floor(&e, &total_shares, &total_tokens)
        };

        // Transfer tokens from receiver to vault
        token_client.transfer(&receiver, &e.current_contract_address(), &tokens);

        // Mint shares to receiver
        token::StellarAssetClient::new(&e, &share_token).mint(&receiver, &shares);

        // Update state
        storage::set_total_shares(&e, &(total_shares + shares));
        storage::set_total_tokens(&e, &(total_tokens + tokens));

        // Emit deposit event
        VaultEvents::deposit(&e, receiver.clone(), tokens, shares);

        storage::extend_instance(&e);
        shares
    }

    fn mint(e: Env, shares: i128, receiver: Address) -> i128 {
        receiver.require_auth();
        if shares <= 0 {
            panic_with_error!(e, VaultError::ZeroAmount);
        }

        let token_addr = storage::get_token(&e);
        let share_token = storage::get_share_token(&e);

        let token_client = token::Client::new(&e, &token_addr);
        let total_shares = storage::get_total_shares(&e);
        let total_tokens = storage::get_total_tokens(&e);

        // Calculate tokens required
        let tokens = if total_shares == 0 || total_tokens == 0 {
            // First deposit gets 1:1 ratio
            shares
        } else {
            // tokens = shares * (total tokens / total shares)
            shares.fixed_mul_ceil(&e, &total_tokens, &total_shares)
        };

        // Transfer tokens from receiver to vault
        token_client.transfer(&receiver, &e.current_contract_address(), &tokens);

        // Mint shares to receiver
        token::StellarAssetClient::new(&e, &share_token).mint(&receiver, &shares);

        // Update state
        storage::set_total_shares(&e, &(total_shares + shares));
        storage::set_total_tokens(&e, &(total_tokens + tokens));

        // Emit mint event
        VaultEvents::mint(&e, receiver.clone(), shares, tokens);

        storage::extend_instance(&e);
        tokens
    }

    fn request_redeem(e: Env, shares: i128, owner: Address) {
        owner.require_auth();

        if shares <= 0 {
            panic_with_error!(e, VaultError::ZeroAmount);
        }

        // Check if user already has a pending request
        if storage::has_redemption_request(&e, &owner) {
            panic_with_error!(e, VaultError::WithdrawalInProgress);
        }

        // Verify user has enough shares
        let share_token = storage::get_share_token(&e);
        let share_client = token::Client::new(&e, &share_token);

        // Transfer shares to vault for locking
        share_client.transfer(&owner, &e.current_contract_address(), &shares);

        // Create redemption request
        let lock_time = storage::get_lock_time(&e);
        let unlock_time = e.ledger().timestamp() + lock_time;
        let request = RedemptionRequest {
            shares,
            unlock_time,
        };

        storage::set_redemption_request(&e, &owner, &request);

        // Emit event
        VaultEvents::request_redeem(&e, owner.clone(), shares, unlock_time);

        storage::extend_instance(&e);
    }

    fn redeem(e: Env, shares: i128, receiver: Address, owner: Address) -> i128 {
        owner.require_auth();

        let request = storage::get_redemption_request(&e, &owner);

        // Verify unlock time has passed
        if e.ledger().timestamp() < request.unlock_time {
            panic_with_error!(e, VaultError::WithdrawalLocked);
        }

        // Verify shares match (or are less than) the request
        if shares > request.shares {
            panic_with_error!(e, VaultError::InsufficientShares);
        }

        let token_addr = storage::get_token(&e);
        let token_client = token::Client::new(&e, &token_addr);
        let share_token = storage::get_share_token(&e);
        let share_client = token::Client::new(&e, &share_token);

        let total_shares = storage::get_total_shares(&e);
        let total_tokens = storage::get_total_tokens(&e);

        // Calculate tokens to return
        // tokens = shares * (total tokens / total shares)
        let tokens = shares.fixed_mul_floor(&e, &total_tokens, &total_shares);

        // Burn shares from vault
        share_client.burn(&e.current_contract_address(), &shares);

        // Transfer tokens to receiver
        token_client.transfer(&e.current_contract_address(), &receiver, &tokens);

        // Update state
        storage::set_total_shares(&e, &(total_shares - shares));
        storage::set_total_tokens(&e, &(total_tokens - tokens));

        // Update or remove request
        if shares == request.shares {
            storage::remove_redemption_request(&e, &owner);
        } else {
            // Partial redemption - update request
            let updated_request = RedemptionRequest {
                shares: request.shares - shares,
                unlock_time: request.unlock_time,
            };
            storage::set_redemption_request(&e, &owner, &updated_request);

            // Return remaining locked shares to owner
            share_client.transfer(&e.current_contract_address(), &owner, &(request.shares - shares));
        }

        // Emit redeem event
        VaultEvents::redeem(&e, owner.clone(), receiver.clone(), shares, tokens);

        storage::extend_instance(&e);
        tokens
    }

    fn emergency_redeem(e: Env, owner: Address) -> i128 {
        owner.require_auth();

        let request = storage::get_redemption_request(&e, &owner);

        let token_addr = storage::get_token(&e);
        let token_client = token::Client::new(&e, &token_addr);
        let share_token = storage::get_share_token(&e);
        let share_client = token::Client::new(&e, &share_token);

        let total_shares = storage::get_total_shares(&e);
        let total_tokens = storage::get_total_tokens(&e);

        // Calculate current value of shares
        let current_tokens = request.shares.fixed_mul_floor(&e, &total_tokens, &total_shares);

        // Calculate penalty
        let penalty_amount = if e.ledger().timestamp() >= request.unlock_time {
            0 // No penalty if already unlocked
        } else {
            let lock_time = storage::get_lock_time(&e);
            let time_remaining = request.unlock_time - e.ledger().timestamp();
            let penalty_rate = storage::get_penalty_rate(&e);

            // Linear penalty: penalty = max_penalty * (time_remaining / total_lock_time)
            let current_penalty_rate = penalty_rate.fixed_mul_floor(&e, &(time_remaining as i128), &(lock_time as i128));
            current_tokens.fixed_mul_floor(&e, &current_penalty_rate, &SCALAR_7)
        };

        let withdrawal_amount = current_tokens - penalty_amount;

        if withdrawal_amount <= 0 {
            panic_with_error!(e, VaultError::InvalidAmount);
        }

        // Execute withdrawal (penalty stays in vault)
        share_client.burn(&e.current_contract_address(), &request.shares);
        token_client.transfer(&e.current_contract_address(), &owner, &withdrawal_amount);

        // Update state (only reduce by withdrawal amount, penalty benefits other users)
        storage::set_total_shares(&e, &(total_shares - request.shares));
        storage::set_total_tokens(&e, &(total_tokens - withdrawal_amount));
        storage::remove_redemption_request(&e, &owner);

        // Emit emergency redeem event
        VaultEvents::emergency_redeem(&e, owner.clone(), request.shares, withdrawal_amount, penalty_amount);

        storage::extend_instance(&e);
        withdrawal_amount
    }

    fn cancel_redeem(e: Env, owner: Address) {
        owner.require_auth();

        let request = storage::get_redemption_request(&e, &owner);

        let share_token = storage::get_share_token(&e);
        token::Client::new(&e, &share_token).transfer(&e.current_contract_address(), &owner, &request.shares);

        storage::remove_redemption_request(&e, &owner);

        // Emit cancel event
        VaultEvents::cancel_redeem(&e, owner.clone(), request.shares);

        storage::extend_instance(&e);
    }

    fn borrow(e: Env, strategy: Address, amount: i128) {
        strategy.require_auth();
        if amount <= 0 {
            panic_with_error!(e, VaultError::ZeroAmount);
        }

        // Check if strategy is authorized
        let strategies = storage::get_strategies(&e);
        if !strategies.contains(&strategy) {
            panic_with_error!(e, VaultError::UnauthorizedStrategy);
        }

        // Check liquidity constraint
        let token_addr = storage::get_token(&e);
        let token_client = token::Client::new(&e, &token_addr);
        let vault_balance = token_client.balance(&e.current_contract_address());
        let total_tokens = storage::get_total_tokens(&e);
        let min_liquidity_rate = storage::get_min_liquidity_rate(&e);

        // Calculate required liquidity: min_liquidity = total_tokens * min_liquidity_rate
        let required_liquidity = total_tokens.fixed_mul_ceil(&e, &min_liquidity_rate, &SCALAR_7);

        // Available to borrow = vault_balance - required_liquidity
        if vault_balance <= required_liquidity {
            panic_with_error!(e, VaultError::InsufficientVaultBalance);
        }

        let available_liquidity = vault_balance - required_liquidity;
        if amount > available_liquidity {
            panic_with_error!(e, VaultError::InsufficientVaultBalance);
        }

        // Transfer tokens to strategy
        token_client.transfer(&e.current_contract_address(), &strategy, &amount);

        // Update strategy borrowed amount
        let mut strategy_data = storage::get_strategy_data(&e, &strategy);
        strategy_data.borrowed += amount;
        storage::set_strategy_data(&e, &strategy, &strategy_data);

        // Note: total_tokens doesn't change - tokens are still "ours"

        // Emit borrow event
        VaultEvents::borrow(&e, strategy.clone(), amount, strategy_data.borrowed);

        storage::extend_instance(&e);
    }

    fn repay(e: Env, strategy: Address, amount: i128) {
        strategy.require_auth();
        if amount <= 0 {
            panic_with_error!(e, VaultError::ZeroAmount);
        }

        // Check if strategy is authorized
        let strategies = storage::get_strategies(&e);
        if !strategies.contains(&strategy) {
            panic_with_error!(e, VaultError::UnauthorizedStrategy);
        }

        let mut strategy_data = storage::get_strategy_data(&e, &strategy);
        if amount > strategy_data.borrowed {
            panic_with_error!(e, VaultError::InvalidAmount);
        }

        // Transfer tokens from strategy to vault
        let token_addr = storage::get_token(&e);
        token::Client::new(&e, &token_addr).transfer(&strategy, &e.current_contract_address(), &amount);

        // Update strategy borrowed amount
        strategy_data.borrowed -= amount;
        storage::set_strategy_data(&e, &strategy, &strategy_data);

        // Note: total_tokens doesn't change - just moving tokens back

        // Emit repay event
        VaultEvents::repay(&e, strategy.clone(), amount, strategy_data.borrowed);

        storage::extend_instance(&e);
    }

    fn transfer_to(e: Env, strategy: Address, amount: i128) {
        strategy.require_auth();
        if amount <= 0 {
            panic_with_error!(e, VaultError::ZeroAmount);
        }

        // Check if strategy is authorized
        let strategies = storage::get_strategies(&e);
        if !strategies.contains(&strategy) {
            panic_with_error!(e, VaultError::UnauthorizedStrategy);
        }

        // Transfer tokens to strategy
        let token_addr = storage::get_token(&e);
        token::Client::new(&e, &token_addr).transfer(&e.current_contract_address(), &strategy, &amount);

        // Update strategy net impact (negative = net outflow to strategy)
        let mut strategy_data = storage::get_strategy_data(&e, &strategy);
        strategy_data.net_impact -= amount;
        storage::set_strategy_data(&e, &strategy, &strategy_data);

        // Reduce total_tokens as this is a real transfer out
        let total_tokens = storage::get_total_tokens(&e);
        storage::set_total_tokens(&e, &(total_tokens - amount));

        // Emit transfer to event
        VaultEvents::transfer_to(&e, strategy.clone(), amount, strategy_data.net_impact);

        storage::extend_instance(&e);
    }

    fn transfer_from(e: Env, strategy: Address, amount: i128) {
        strategy.require_auth();
        if amount <= 0 {
            panic_with_error!(e, VaultError::ZeroAmount);
        }

        // Check if strategy is authorized
        let strategies = storage::get_strategies(&e);
        if !strategies.contains(&strategy) {
            panic_with_error!(e, VaultError::UnauthorizedStrategy);
        }

        // Transfer tokens from strategy to vault
        let token_addr = storage::get_token(&e);
        token::Client::new(&e, &token_addr).transfer(&strategy, &e.current_contract_address(), &amount);

        // Update strategy net impact (positive = net inflow from strategy)
        let mut strategy_data = storage::get_strategy_data(&e, &strategy);
        strategy_data.net_impact += amount;
        storage::set_strategy_data(&e, &strategy, &strategy_data);

        // Increase total_tokens as this is a real transfer in
        let total_tokens = storage::get_total_tokens(&e);
        storage::set_total_tokens(&e, &(total_tokens + amount));

        // Emit transfer from event
        VaultEvents::transfer_from(&e, strategy.clone(), amount, strategy_data.net_impact);

        storage::extend_instance(&e);
    }
}