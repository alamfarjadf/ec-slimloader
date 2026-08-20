#![cfg_attr(not(test), no_std)]

pub mod board;
mod jump;
pub mod lifecycle;
pub mod rom_api;
mod verification;

pub use board::{ExternalStorage, Mcxa, McxaConfig, Partitions, SlotPartition, StatePartition};
