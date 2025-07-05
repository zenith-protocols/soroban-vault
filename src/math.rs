use soroban_sdk::Env;
use soroban_fixed_point_math::SorobanFixedPoint;

pub const SCALAR_7: i128 = 10_000_000;

/// Calculates share/token conversions based on current ratios
pub struct Converter<'a> {
    env: &'a Env,
    total_shares: i128,
    total_tokens: i128,
}

impl<'a> Converter<'a> {
    pub fn new(env: &'a Env, total_shares: i128, total_tokens: i128) -> Self {
        Self {
            env,
            total_shares,
            total_tokens,
        }
    }

    /// Calculates shares to mint for a given token deposit
    pub fn shares_from_tokens(&self, tokens: i128) -> i128 {
        if self.total_shares == 0 || self.total_tokens == 0 {
            tokens // First deposit gets 1:1 ratio
        } else {
            // shares = tokens * (total shares / total tokens)
            tokens.fixed_mul_floor(self.env, &self.total_shares, &self.total_tokens)
        }
    }

    /// Calculates tokens required to mint exact shares
    pub fn tokens_from_shares(&self, shares: i128) -> i128 {
        if self.total_shares == 0 || self.total_tokens == 0 {
            shares // First deposit gets 1:1 ratio
        } else {
            // tokens = shares * (total tokens / total shares)
            shares.fixed_mul_ceil(self.env, &self.total_tokens, &self.total_shares)
        }
    }

    /// Calculates tokens to return for share redemption
    pub fn redemption_value(&self, shares: i128) -> i128 {
        if self.total_shares == 0 {
            0
        } else {
            // tokens = shares * (total tokens / total shares)
            shares.fixed_mul_floor(self.env, &self.total_tokens, &self.total_shares)
        }
    }
}

/// Calculates penalty amount for emergency redemption
pub fn calculate_penalty(
    env: &Env,
    token_value: i128,
    unlock_time: u64,
    lock_time: u64,
    penalty_rate: i128,
) -> i128 {
    let current_time = env.ledger().timestamp();

    if current_time >= unlock_time {
        0 // No penalty if already unlocked
    } else {
        let time_remaining = unlock_time - current_time;

        // Linear penalty: penalty = max_penalty * (time_remaining / total_lock_time)
        let current_penalty_rate = penalty_rate.fixed_mul_floor(
            env,
            &(time_remaining as i128),
            &(lock_time as i128),
        );

        token_value.fixed_mul_floor(env, &current_penalty_rate, &SCALAR_7)
    }
}

/// Calculates minimum required liquidity in the vault
pub fn calculate_min_liquidity(env: &Env, total_tokens: i128, min_liquidity_rate: i128) -> i128 {
    if total_tokens == 0 {
        0
    } else {
        // Use ceiling division to ensure we always maintain minimum
        total_tokens.fixed_mul_ceil(env, &min_liquidity_rate, &SCALAR_7)
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use super::*;
    use soroban_sdk::testutils::{Ledger, LedgerInfo};

    #[test]
    fn test_converter_first_deposit() {
        let env = Env::default();

        // When vault is empty (0 shares, 0 tokens)
        let converter = Converter::new(&env, 0, 0);

        // First deposit should get 1:1 ratio
        assert_eq!(converter.shares_from_tokens(1000 * SCALAR_7), 1000 * SCALAR_7);
        assert_eq!(converter.shares_from_tokens(1), 1); // Even 1 stroop
        assert_eq!(converter.shares_from_tokens(999_999 * SCALAR_7), 999_999 * SCALAR_7);

        // Reverse calculation should also be 1:1
        assert_eq!(converter.tokens_from_shares(1000 * SCALAR_7), 1000 * SCALAR_7);
        assert_eq!(converter.tokens_from_shares(1), 1);
    }

    #[test]
    fn test_converter_share_price_appreciation() {
        let env = Env::default();

        // Vault has 1000 shares and 1200 tokens (20% appreciation)
        // Share price = 1.2 tokens per share
        let converter = Converter::new(&env, 1000 * SCALAR_7, 1200 * SCALAR_7);

        // Depositing 600 tokens should give 500 shares (600 / 1.2)
        assert_eq!(converter.shares_from_tokens(600 * SCALAR_7), 500 * SCALAR_7);

        // To mint 100 shares, need 120 tokens (100 * 1.2)
        assert_eq!(converter.tokens_from_shares(100 * SCALAR_7), 120 * SCALAR_7);

        // Redeeming 500 shares should give 600 tokens
        assert_eq!(converter.redemption_value(500 * SCALAR_7), 600 * SCALAR_7);
    }

    #[test]
    fn test_converter_share_price_depreciation() {
        let env = Env::default();

        // Vault has 1000 shares and 800 tokens (20% loss)
        // Share price = 0.8 tokens per share
        let converter = Converter::new(&env, 1000 * SCALAR_7, 800 * SCALAR_7);

        // Depositing 400 tokens should give 500 shares (400 / 0.8)
        assert_eq!(converter.shares_from_tokens(400 * SCALAR_7), 500 * SCALAR_7);

        // To mint 100 shares, need 80 tokens (100 * 0.8)
        assert_eq!(converter.tokens_from_shares(100 * SCALAR_7), 80 * SCALAR_7);

        // Redeeming 500 shares should give 400 tokens
        assert_eq!(converter.redemption_value(500 * SCALAR_7), 400 * SCALAR_7);
    }

    #[test]
    fn test_converter_tokens_from_shares_exact() {
        let env = Env::default();

        // Test various share prices
        let test_cases = vec![
            (1000 * SCALAR_7, 1000 * SCALAR_7), // 1:1 ratio
            (1000 * SCALAR_7, 2000 * SCALAR_7), // 2:1 ratio
            (1000 * SCALAR_7, 500 * SCALAR_7),  // 0.5:1 ratio
            (1000 * SCALAR_7, 1500 * SCALAR_7), // 1.5:1 ratio
        ];

        for (total_shares, total_tokens) in test_cases {
            let converter = Converter::new(&env, total_shares, total_tokens);

            // Test that tokens_from_shares is inverse of shares_from_tokens
            let shares = 100 * SCALAR_7;
            let tokens_needed = converter.tokens_from_shares(shares);

            // If we deposit tokens_needed, we should get at least the shares we wanted
            let converter2 = Converter::new(&env, total_shares, total_tokens);
            let shares_received = converter2.shares_from_tokens(tokens_needed);

            assert!(shares_received >= shares,
                    "Should receive at least requested shares. Got {} wanted {}",
                    shares_received, shares);
        }
    }

    #[test]
    fn test_converter_rounding_behavior() {
        let env = Env::default();

        // Test with prime numbers to force rounding
        let converter = Converter::new(&env, 1000 * SCALAR_7, 1337 * SCALAR_7);

        // shares_from_tokens uses floor (favorable to vault)
        // Depositing 100 tokens when share price is 1.337
        let shares = converter.shares_from_tokens(100 * SCALAR_7);
        // Should get less than 100 shares due to appreciation
        assert_eq!(shares, 747943156); // ~74.79 shares with 7 decimals

        // tokens_from_shares uses ceil (favorable to vault)
        // Minting exactly 100 shares when share price is 1.337
        let tokens = converter.tokens_from_shares(100 * SCALAR_7);
        assert_eq!(tokens, 1337000000); // Need 133.7 tokens

        // redemption_value uses floor (favorable to vault)
        // Redeeming 100 shares when share price is 1.337
        let redemption = converter.redemption_value(100 * SCALAR_7);
        assert_eq!(redemption, 1337000000); // Get 133.7 tokens

        // Verify rounding direction by checking small amounts
        let one_share_redemption = converter.redemption_value(1);
        let one_share_cost = converter.tokens_from_shares(1);
        // Cost to mint should be >= redemption value (vault favorable)
        assert!(one_share_cost >= one_share_redemption);
    }

    #[test]
    fn test_converter_edge_cases() {
        let env = Env::default();

        // Test with 1 stroop
        let converter = Converter::new(&env, SCALAR_7, SCALAR_7);
        assert_eq!(converter.shares_from_tokens(1), 1);
        assert_eq!(converter.tokens_from_shares(1), 1);
        assert_eq!(converter.redemption_value(1), 1);

        // Test with very large numbers
        let large_amount = i128::MAX / 100; // Avoid overflow
        let converter = Converter::new(&env, large_amount, large_amount);
        assert_eq!(converter.shares_from_tokens(SCALAR_7), SCALAR_7);

        // Test redemption when total_shares is 0 (should return 0)
        let converter = Converter::new(&env, 0, 1000 * SCALAR_7);
        assert_eq!(converter.redemption_value(100 * SCALAR_7), 0);
    }

    #[test]
    fn test_penalty_calculation_linear_decay() {
        let env = Env::default();
        let lock_time = 300; // 5 minutes
        let penalty_rate = SCALAR_7 / 10; // 10% max penalty
        let token_value = 1000 * SCALAR_7;

        // Set current time
        env.ledger().set(LedgerInfo {
            timestamp: 1000,
            protocol_version: 22,
            sequence_number: 100,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 10,
            min_persistent_entry_ttl: 10,
            max_entry_ttl: 3110400,
        });

        let unlock_time = 1000 + lock_time;

        // At start (full penalty)
        let penalty = calculate_penalty(&env, token_value, unlock_time, lock_time, penalty_rate);
        assert_eq!(penalty, 100 * SCALAR_7); // 10% of 1000

        // Halfway through (half penalty)
        env.ledger().set(LedgerInfo {
            timestamp: 1000 + 150,
            protocol_version: 22,
            sequence_number: 100,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 10,
            min_persistent_entry_ttl: 10,
            max_entry_ttl: 3110400,
        });

        let penalty = calculate_penalty(&env, token_value, unlock_time, lock_time, penalty_rate);
        assert_eq!(penalty, 50 * SCALAR_7); // 5% of 1000

        // 90% through (10% penalty remaining)
        env.ledger().set(LedgerInfo {
            timestamp: 1000 + 270,
            protocol_version: 22,
            sequence_number: 100,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 10,
            min_persistent_entry_ttl: 10,
            max_entry_ttl: 3110400,
        });

        let penalty = calculate_penalty(&env, token_value, unlock_time, lock_time, penalty_rate);
        assert_eq!(penalty, 10 * SCALAR_7); // 1% of 1000
    }

    #[test]
    fn test_penalty_calculation_edge_cases() {
        let env = Env::default();

        env.ledger().set(LedgerInfo {
            timestamp: 1000,
            protocol_version: 22,
            sequence_number: 100,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 10,
            min_persistent_entry_ttl: 10,
            max_entry_ttl: 3110400,
        });

        // Test no penalty after unlock time
        let penalty = calculate_penalty(&env, 1000 * SCALAR_7, 999, 300, SCALAR_7 / 10);
        assert_eq!(penalty, 0);

        // Test 100% penalty rate
        let penalty = calculate_penalty(&env, 1000 * SCALAR_7, 1300, 300, SCALAR_7);
        assert_eq!(penalty, 1000 * SCALAR_7); // All tokens as penalty

        // Test 0% penalty rate
        let penalty = calculate_penalty(&env, 1000 * SCALAR_7, 1300, 300, 0);
        assert_eq!(penalty, 0);

        // Test with 1 stroop value
        let penalty = calculate_penalty(&env, 1, 1300, 300, SCALAR_7 / 2);
        assert_eq!(penalty, 0); // Rounds down to 0

        // Test at exact unlock time
        env.ledger().set(LedgerInfo {
            timestamp: 1300,
            protocol_version: 22,
            sequence_number: 100,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 10,
            min_persistent_entry_ttl: 10,
            max_entry_ttl: 3110400,
        });

        let penalty = calculate_penalty(&env, 1000 * SCALAR_7, 1300, 300, SCALAR_7 / 10);
        assert_eq!(penalty, 0); // No penalty at exact unlock time
    }

    #[test]
    fn test_min_liquidity_calculation() {
        let env = Env::default();

        // Test with 20% minimum liquidity
        let min_liquidity_rate = SCALAR_7 / 5; // 20%

        // Normal case
        let min_liq = calculate_min_liquidity(&env, 1000 * SCALAR_7, min_liquidity_rate);
        assert_eq!(min_liq, 200 * SCALAR_7); // 20% of 1000

        // Test with 0 total tokens
        let min_liq = calculate_min_liquidity(&env, 0, min_liquidity_rate);
        assert_eq!(min_liq, 0);

        // Test with 100% minimum liquidity
        let min_liq = calculate_min_liquidity(&env, 1000 * SCALAR_7, SCALAR_7);
        assert_eq!(min_liq, 1000 * SCALAR_7);

        // Test with 0% minimum liquidity
        let min_liq = calculate_min_liquidity(&env, 1000 * SCALAR_7, 0);
        assert_eq!(min_liq, 0);

        // Test rounding up (favorable to safety)
        // If we have 1000 tokens and want 33.33...% minimum
        let min_liquidity_rate = SCALAR_7 / 3; // ~33.33%
        let min_liq = calculate_min_liquidity(&env, 1000 * SCALAR_7, min_liquidity_rate);
        // Should round up to ensure minimum is maintained
        assert!(min_liq >= 333 * SCALAR_7);

        // Test with small amounts
        let min_liq = calculate_min_liquidity(&env, 10, SCALAR_7 / 2); // 50% of 10 stroops
        assert_eq!(min_liq, 5);
    }
}