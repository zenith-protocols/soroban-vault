#![no_std]

mod storage;
mod strategy;
mod contract;
pub use contract::{VaultContract, VaultContractClient};
mod test;
