use soroban_sdk::{contract, contractimpl, contractclient, Address, Env, Vec, String, BytesN};

use crate::{
    math::SCALAR_7,
    storage::{self, StrategyData},
    strategy,
    token::create_share_token,
    validation,
    vault,
};

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

    /// Returns the net impact of a strategy (profit/loss from transfers)
    ///
    /// # Arguments
    /// * `strategy` - Address of the strategy contract
    ///
    /// # Returns
    /// Net impact: positive = profit, negative = loss
    fn net_impact(e: Env, strategy: Address) -> i128;

    /// Deposits underlying tokens and mints share tokens to receiver
    ///
    /// # Arguments
    /// * `tokens` - Amount of underlying tokens to deposit (must be > 0)
    /// * `receiver` - Address to receive the minted share tokens
    /// * `owner` - Address providing the tokens (must authorize transaction)
    ///
    /// # Returns
    /// Amount of share tokens minted to receiver
    ///
    /// # Panics
    /// - `ZeroAmount` if tokens <= 0
    fn deposit(e: Env, tokens: i128, receiver: Address, owner: Address) -> i128;

    /// Mints exact share tokens by depositing underlying tokens
    ///
    /// # Arguments
    /// * `shares` - Exact amount of share tokens to mint (must be > 0)
    /// * `receiver` - Address to receive the minted share tokens
    /// * `owner` - Address providing the tokens (must authorize transaction)
    ///
    /// # Returns
    /// Amount of underlying tokens deposited
    ///
    /// # Panics
    /// - `ZeroAmount` if shares <= 0
    fn mint(e: Env, shares: i128, receiver: Address, owner: Address) -> i128;

    /// Requests a redemption for exact shares
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

    /// Executes a redemption request after the delay period
    /// Burns all queued shares and returns the corresponding assets
    ///
    /// # Arguments
    /// * `receiver` - Address to receive the underlying assets
    /// * `owner` - Address that requested the redemption (must authorize transaction)
    ///
    /// # Returns
    /// Amount of underlying assets transferred to receiver
    ///
    /// # Panics
    /// - `WithdrawalLocked` if unlock time hasn't been reached
    fn redeem(e: Env, receiver: Address, owner: Address) -> i128;

    /// Emergency redemption with penalty before delay period ends
    ///
    /// # Arguments
    /// * `receiver` - Address to receive the underlying assets
    /// * `owner` - Address that requested the redemption (must authorize transaction)
    ///
    /// # Returns
    /// Amount of underlying assets transferred to receiver (after penalty)
    fn emergency_redeem(e: Env, receiver: Address, owner: Address) -> i128;

    /// Cancels a pending redemption request
    ///
    /// # Arguments
    /// * `owner` - Address that requested the redemption (must authorize transaction)
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
        // Validate rates
        validation::validate_rate(&e, penalty_rate);
        validation::validate_rate(&e, min_liquidity_rate);

        // Deploy share token
        let share_token = create_share_token(&e, token_wasm_hash, &token, &name, &symbol);

        // Initialize storage
        storage::set_token(&e, &token);
        storage::set_share_token(&e, &share_token);
        storage::set_total_shares(&e, &0);
        storage::set_total_tokens(&e, &0);
        storage::set_lock_time(&e, &lock_time);
        storage::set_penalty_rate(&e, &penalty_rate);
        storage::set_min_liquidity_rate(&e, &min_liquidity_rate);
        storage::set_strategies(&e, &strategies);

        // Initialize strategies
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

    fn net_impact(e: Env, strategy: Address) -> i128 {
        storage::extend_instance(&e);
        storage::get_strategy_data(&e, &strategy).net_impact
    }

    fn deposit(e: Env, tokens: i128, receiver: Address, owner: Address) -> i128 {
        owner.require_auth();
        let result = vault::deposit(&e, tokens, &receiver, &owner);
        storage::extend_instance(&e);
        result
    }

    fn mint(e: Env, shares: i128, receiver: Address, owner: Address) -> i128 {
        owner.require_auth();
        let result = vault::mint(&e, shares, &receiver, &owner);
        storage::extend_instance(&e);
        result
    }

    fn request_redeem(e: Env, shares: i128, owner: Address) {
        owner.require_auth();
        vault::request_redeem(&e, shares, &owner);
        storage::extend_instance(&e);
    }

    fn redeem(e: Env, receiver: Address, owner: Address) -> i128 {
        owner.require_auth();
        let result = vault::redeem(&e, &receiver, &owner);
        storage::extend_instance(&e);
        result
    }

    fn emergency_redeem(e: Env, receiver: Address, owner: Address) -> i128 {
        owner.require_auth();
        let result = vault::emergency_redeem(&e, &receiver, &owner);
        storage::extend_instance(&e);
        result
    }

    fn cancel_redeem(e: Env, owner: Address) {
        owner.require_auth();
        vault::cancel_redeem(&e, &owner);
        storage::extend_instance(&e);
    }

    fn borrow(e: Env, strategy: Address, amount: i128) {
        strategy.require_auth();
        strategy::borrow(&e, &strategy, amount);
        storage::extend_instance(&e);
    }

    fn repay(e: Env, strategy: Address, amount: i128) {
        strategy.require_auth();
        strategy::repay(&e, &strategy, amount);
        storage::extend_instance(&e);
    }

    fn transfer_to(e: Env, strategy: Address, amount: i128) {
        strategy.require_auth();
        strategy::transfer_to(&e, &strategy, amount);
        storage::extend_instance(&e);
    }

    fn transfer_from(e: Env, strategy: Address, amount: i128) {
        strategy.require_auth();
        strategy::transfer_from(&e, &strategy, amount);
        storage::extend_instance(&e);
    }
}