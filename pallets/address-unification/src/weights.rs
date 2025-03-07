
//! Autogenerated weights for `address_unification`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2024-06-19, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `jaypan-peaq`, CPU: `AMD Ryzen 5 5600H with Radeon Graphics`
//! EXECUTION: Some(Native), WASM-EXECUTION: Compiled, CHAIN: Some("dev-local"), DB CACHE: 1024

// Executed Command:
// ./target/release/peaq-node
// benchmark
// pallet
// --chain=dev-local
// --execution=native
// --wasm-execution=compiled
// --pallet=address_unification
// --extrinsic=*
// --steps=50
// --repeat=20
// --output=weight.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::Weight};
use core::marker::PhantomData;

/// Weight functions for `address_unification`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
	/// Storage: AddressUnification EvmAddresses (r:1 w:1)
	/// Proof: AddressUnification EvmAddresses (max_values: None, max_size: Some(60), added: 2535, mode: MaxEncodedLen)
	/// Storage: AddressUnification Accounts (r:1 w:1)
	/// Proof: AddressUnification Accounts (max_values: None, max_size: Some(60), added: 2535, mode: MaxEncodedLen)
	/// Storage: System BlockHash (r:1 w:0)
	/// Proof: System BlockHash (max_values: None, max_size: Some(44), added: 2519, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:0)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn claim_account() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `173`
		//  Estimated: `3593`
		// Minimum execution time: 95_973_000 picoseconds.
		Weight::from_parts(96_814_000, 0)
			.saturating_add(Weight::from_parts(0, 3593))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: AddressUnification EvmAddresses (r:1 w:1)
	/// Proof: AddressUnification EvmAddresses (max_values: None, max_size: Some(60), added: 2535, mode: MaxEncodedLen)
	/// Storage: AddressUnification Accounts (r:1 w:1)
	/// Proof: AddressUnification Accounts (max_values: None, max_size: Some(60), added: 2535, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:0)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn claim_default_account() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `119`
		//  Estimated: `3593`
		// Minimum execution time: 41_669_000 picoseconds.
		Weight::from_parts(42_210_000, 0)
			.saturating_add(Weight::from_parts(0, 3593))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(2))
	}
}
