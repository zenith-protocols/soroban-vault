use soroban_sdk::{contracttype, unwrap::UnwrapOptimized, Address, Env, Vec as SorobanVec};
use stellar_tokens::fungible::{
    BALANCE_EXTEND_AMOUNT, BALANCE_TTL_THRESHOLD, INSTANCE_EXTEND_AMOUNT, INSTANCE_TTL_THRESHOLD,
};

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum StorageKey {
    LockTime,
    Strategies,
    Strategy(Address),
    LastDepositTime(Address),
}

pub fn extend_instance(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_EXTEND_AMOUNT);
}

pub fn get_lock_time(e: &Env) -> u64 {
    e.storage()
        .instance()
        .get::<StorageKey, u64>(&StorageKey::LockTime)
        .unwrap_optimized()
}

pub fn set_lock_time(e: &Env, lock_time: &u64) {
    e.storage()
        .instance()
        .set::<StorageKey, u64>(&StorageKey::LockTime, lock_time);
}

pub fn get_strategies(e: &Env) -> SorobanVec<Address> {
    e.storage()
        .instance()
        .get::<StorageKey, SorobanVec<Address>>(&StorageKey::Strategies)
        .unwrap_optimized()
}

pub fn set_strategies(e: &Env, strategies: &SorobanVec<Address>) {
    e.storage()
        .instance()
        .set::<StorageKey, SorobanVec<Address>>(&StorageKey::Strategies, strategies);
}

pub fn get_strategy_net_impact(e: &Env, strategy: &Address) -> i128 {
    let key = StorageKey::Strategy(strategy.clone());
    e.storage()
        .persistent()
        .extend_ttl(&key, BALANCE_TTL_THRESHOLD, BALANCE_EXTEND_AMOUNT);
    e.storage()
        .persistent()
        .get::<StorageKey, i128>(&key)
        .unwrap_or(0)
}

pub fn set_strategy_net_impact(e: &Env, strategy: &Address, net_impact: i128) {
    let key = StorageKey::Strategy(strategy.clone());
    e.storage()
        .persistent()
        .set::<StorageKey, i128>(&key, &net_impact);
    e.storage()
        .persistent()
        .extend_ttl(&key, BALANCE_TTL_THRESHOLD, BALANCE_EXTEND_AMOUNT);
}

pub fn get_last_deposit_time(e: &Env, user: &Address) -> Option<u64> {
    let key = StorageKey::LastDepositTime(user.clone());
    let result = e.storage().persistent().get::<StorageKey, u64>(&key);
    if result.is_some() {
        e.storage()
            .persistent()
            .extend_ttl(&key, BALANCE_TTL_THRESHOLD, BALANCE_EXTEND_AMOUNT);
    }
    result
}

pub fn set_last_deposit_time(e: &Env, user: &Address, timestamp: u64) {
    let key = StorageKey::LastDepositTime(user.clone());
    e.storage()
        .persistent()
        .set::<StorageKey, u64>(&key, &timestamp);
    e.storage()
        .persistent()
        .extend_ttl(&key, BALANCE_TTL_THRESHOLD, BALANCE_EXTEND_AMOUNT);
}
