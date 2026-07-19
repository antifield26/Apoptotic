// Allow structural clippy warnings that would require invasive refactoring
#![allow(clippy::type_complexity)]
#![allow(unreachable_patterns)]

pub mod anvil;
pub mod chunk;
pub mod chunk_store;
pub mod crops;
pub mod fluid;
pub mod generator;
pub mod lighting;
pub mod paletted;
pub mod physics;
pub mod redstone;
pub mod world;
