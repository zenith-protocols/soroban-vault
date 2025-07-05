#![allow(dead_code)]

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, BytesN, Env, String, Vec,
};
use vault::{VaultContract, VaultContractClient};

// Re-export token WASM
mod token_contract {
    soroban_sdk::contractimport!(file = "./token.wasm");
}

// Constants
pub const SCALAR_7: i128 = 10_000_000;
pub const ONE_DAY_LEDGERS: u32 = 17280; // assumes 5s a ledger

// Default configuration values
pub const DEFAULT_LOCK_TIME: u64 = 300; // 5 minutes
pub const DEFAULT_PENALTY_RATE: i128 = SCALAR_7 / 10; // 10%
pub const DEFAULT_MIN_LIQUIDITY_RATE: i128 = SCALAR_7 / 5; // 20%

/// Test environment with all necessary components
pub struct TestEnv<'a> {
    pub env: Env,
    pub token: Address,
    pub vault: VaultContractClient<'a>,
    pub admin: Address,
    pub users: Vec<Address>,
    pub strategies: Vec<Address>,
    pub token_wasm_hash: BytesN<32>,
}

/// Configuration for vault setup
pub struct VaultConfig {
    pub lock_time: u64,
    pub penalty_rate: i128,
    pub min_liquidity_rate: i128,
    pub num_users: u32,
    pub num_strategies: u32,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            lock_time: DEFAULT_LOCK_TIME,
            penalty_rate: DEFAULT_PENALTY_RATE,
            min_liquidity_rate: DEFAULT_MIN_LIQUIDITY_RATE,
            num_users: 2,
            num_strategies: 1,
        }
    }
}

/// Creates a complete test environment with vault, token, users, and strategies
pub fn setup_vault_with_config<'a>(config: VaultConfig) -> TestEnv<'a> {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();
    env.mock_all_auths();

    // Set TTL parameters
    env.ledger().set_min_temp_entry_ttl(ONE_DAY_LEDGERS);
    env.ledger().set_min_persistent_entry_ttl(ONE_DAY_LEDGERS * 30);

    // Create admin
    let admin = Address::generate(&env);

    // Deploy underlying token
    let token = env.register_stellar_asset_contract_v2(admin.clone());

    // Upload token WASM for share token
    let token_wasm_hash = env.deployer().upload_contract_wasm(token_contract::WASM);

    // Create users and fund them with 10k tokens each
    let mut users = Vec::new(&env);
    for _ in 0..config.num_users {
        let user = Address::generate(&env);
        let token_client = StellarAssetClient::new(&env, &token.address());
        token_client.mint(&user, &(10_000 * SCALAR_7));
        users.push_back(user);
    }

    // Create strategies
    let mut strategies = Vec::new(&env);
    for _ in 0..config.num_strategies {
        strategies.push_back(Address::generate(&env));
    }

    // Deploy vault
    let vault_address = env.register(
        VaultContract,
        (
            token.address(),
            token_wasm_hash.clone(),
            String::from_str(&env, "Test Vault Shares"),
            String::from_str(&env, "TVS"),
            strategies.clone(),
            config.lock_time,
            config.penalty_rate,
            config.min_liquidity_rate,
        ),
    );

    let vault = VaultContractClient::new(&env, &vault_address);

    TestEnv {
        env,
        token: token.address(),
        vault,
        admin,
        users,
        strategies,
        token_wasm_hash,
    }
}

/// Creates a basic test environment with default configuration
pub fn setup_vault<'a>() -> TestEnv<'a> {
    setup_vault_with_config(VaultConfig::default())
}

/// Helper functions for TestEnv
impl<'a> TestEnv<'a> {
    /// Get token client for the underlying token
    pub fn token_client(&self) -> TokenClient {
        TokenClient::new(&self.env, &self.token)
    }

    /// Get token client for the share token
    pub fn share_token_client(&self) -> TokenClient {
        TokenClient::new(&self.env, &self.vault.share_token())
    }

    /// Mint tokens to any address (user or strategy)
    pub fn mint_tokens(&self, to: &Address, amount: i128) {
        let token_client = StellarAssetClient::new(&self.env, &self.token);
        token_client.mint(to, &amount);
    }

    /// Fund a user with tokens (alias for mint_tokens)
    pub fn fund_user(&self, user: &Address, amount: i128) {
        self.mint_tokens(user, amount);
    }

    /// Create a new funded user
    pub fn create_funded_user(&self, amount: i128) -> Address {
        let user = Address::generate(&self.env);
        self.mint_tokens(&user, amount);
        user
    }

    /// Get user's token balance
    pub fn token_balance(&self, user: &Address) -> i128 {
        self.token_client().balance(user)
    }

    /// Get user's share balance
    pub fn share_balance(&self, user: &Address) -> i128 {
        self.share_token_client().balance(user)
    }

    /// Get vault's token balance
    pub fn vault_balance(&self) -> i128 {
        self.token_client().balance(&self.vault.address)
    }

    /// Advance time by seconds
    pub fn advance_time(&self, seconds: u64) {
        let current = self.env.ledger().timestamp();
        self.env.ledger().set_timestamp(current + seconds);
    }

    /// Advance time past lock period
    pub fn advance_past_lock(&self) {
        self.advance_time(DEFAULT_LOCK_TIME + 1);
    }

    /// Advance time to exact unlock time
    pub fn advance_to_unlock(&self) {
        self.advance_time(DEFAULT_LOCK_TIME);
    }
}


/// Creates a minimal setup for unit-style tests
pub fn setup_simple<'a>() -> (Env, Address, VaultContractClient<'a>) {
    let test_env = setup_vault_with_config(VaultConfig {
        num_users: 1,
        num_strategies: 0,
        ..Default::default()
    });

    let user = test_env.users.get(0).unwrap();
    (test_env.env, user, test_env.vault)
}

/// Asserts two values are approximately equal (within 0.01%)
pub fn assert_approx_eq(actual: i128, expected: i128, msg: &str) {
    let tolerance = expected.abs() / 10000; // 0.01%
    let diff = (actual - expected).abs();
    assert!(
        diff <= tolerance,
        "{}: expected {} ± {}, got {} (diff: {})",
        msg,
        expected,
        tolerance,
        actual,
        diff
    );
}

/// Asserts actual value is within a range
pub fn assert_in_range(actual: i128, min: i128, max: i128, msg: &str) {
    assert!(
        actual >= min && actual <= max,
        "{}: expected value in range [{}, {}], got {}",
        msg,
        min,
        max,
        actual
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setup_vault() {
        let test_env = setup_vault();

        // Verify setup
        assert_eq!(test_env.users.len(), 2);
        assert_eq!(test_env.strategies.len(), 1);

        // Check user balances - all users get 10k tokens
        let user = test_env.users.get(0).unwrap();
        let user_balance = test_env.token_balance(&user);
        assert_eq!(user_balance, 10_000 * SCALAR_7);

        // Check vault is deployed
        let total_shares = test_env.vault.total_shares();
        assert_eq!(total_shares, 0);
    }

    #[test]
    fn test_custom_setup() {
        let test_env = setup_vault_with_config(VaultConfig {
            num_users: 3,
            num_strategies: 2,
            lock_time: 600,
            penalty_rate: SCALAR_7 / 5, // 20%
            min_liquidity_rate: SCALAR_7 / 10, // 10%
        });

        assert_eq!(test_env.users.len(), 3);
        assert_eq!(test_env.strategies.len(), 2);

        // Verify all users have 10k tokens
        for user in test_env.users.iter() {
            let balance = test_env.token_balance(&user);
            assert_eq!(balance, 10_000 * SCALAR_7);
        }
    }
}