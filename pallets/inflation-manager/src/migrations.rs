use super::*;

use frame_support::{pallet_prelude::*, weights::Weight};
use sp_runtime::traits::AccountIdConversion;

pub(crate) fn on_runtime_upgrade<T: Config>() -> Weight {
	upgrade::MigrateToV0::<T>::on_runtime_upgrade()
}

mod upgrade {
	use super::*;

	pub struct MigrateToV0<T>(sp_std::marker::PhantomData<T>);

	impl<T: Config> MigrateToV0<T> {
		fn fund_difference_balances() {
			let account = T::PotId::get().into_account_truncating();
			let now_total_issuance = T::Currency::total_issuance();
			let desired_issuance = T::DefaultTotalIssuanceNum::get();
			if now_total_issuance < desired_issuance {
				let amount = desired_issuance.saturating_sub(now_total_issuance);
				T::Currency::deposit_creating(&account, amount);
				log::info!(
					"Total issuance was increased from {:?} to {:?}, by {:?} tokens.",
					now_total_issuance,
					desired_issuance,
					amount
				);
			}
		}

		pub fn on_runtime_upgrade() -> Weight {
			let mut weight_writes = 0;
			let mut weight_reads = 0;

			let onchain_storage_version = Pallet::<T>::on_chain_storage_version();
			weight_reads += 1;

			let current = Pallet::<T>::current_storage_version();

			if onchain_storage_version < current {
				Self::fund_difference_balances();

				let inflation_configuration = T::DefaultInflationConfiguration::get();
				// install inflation config
				InflationConfiguration::<T>::put(inflation_configuration.clone());
				weight_writes += 1;

				// set current year to 1
				CurrentYear::<T>::put(1);
				weight_writes += 1;

				// calculate inflation parameters for the first year
				let inflation_parameters =
					Pallet::<T>::update_inflation_parameters(&inflation_configuration);
				weight_reads += 1;

				// install inflation parameters for first year
				InflationParameters::<T>::put(inflation_parameters.clone());
				weight_writes += 1;

				// set the flag to calculate inflation parameters after a year(in blocks)
				let racalculation_target_block = frame_system::Pallet::<T>::current_block_number() +
					T::BlockNumber::from(BLOCKS_PER_YEAR);
				weight_reads += 1;

				// Update recalculation flag
				DoRecalculationAt::<T>::put(racalculation_target_block);
				weight_writes += 1;

				let block_rewards = Pallet::<T>::rewards_per_block(&inflation_parameters);
				weight_reads += 1;

				BlockRewards::<T>::put(block_rewards);
				weight_writes += 1;

				// Update storage version
				STORAGE_VERSION.put::<Pallet<T>>();

				log::info!(
					"Inflation Manager storage migration completed from version {:?} to version {:?}", onchain_storage_version, current
				);
			}
			T::DbWeight::get().reads_writes(weight_reads, weight_writes)
		}
	}
}
