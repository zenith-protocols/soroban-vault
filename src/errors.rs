use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VaultError {
    InvalidAmount = 4041,
    InsufficientVaultBalance = 4042,
    RedemptionInProgress = 4043,
    RedemptionLocked = 4044,
    UnauthorizedStrategy = 4045,
}