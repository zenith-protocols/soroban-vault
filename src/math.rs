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