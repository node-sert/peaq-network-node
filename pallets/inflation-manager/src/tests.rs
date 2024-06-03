use super::*;
use frame_support::assert_noop;
use frame_system::RawOrigin;
use mock::*;
use peaq_primitives_xcm::BlockNumber;
use sp_runtime::{
	traits::{AccountIdConversion, BadOrigin, Saturating},
	Perbill,
};

#[test]
fn sanity_check_genesis_delayed_tge() {
	ExternalityBuilder::default().build().execute_with(|| {
		let snapshot = InflationManagerSnapshot::take_snapshot_at(0);
		// Delayed TGE sets inflation parameters as their default falue
		let expected_inflation_parameters = InflationParametersT::default();

		assert_eq!(snapshot.inflation_configuration, InflationConfigurationT::default());
		assert_eq!(snapshot.inflation_parameters, expected_inflation_parameters);
		assert_eq!(
			snapshot.do_recalculation_at as u64,
			<TestRuntime as Config>::DoInitializeAt::get()
		);
		// inflation has not kicked off, so current year should be 0
		assert_eq!(snapshot.current_year, 0u128);
		assert_eq!(
			snapshot.block_rewards,
			<TestRuntime as Config>::BlockRewardBeforeInitialize::get()
		);
	})
}

#[test]
fn check_fund_enough_token_at_delayed_tge_kickoff() {
	ExternalityBuilder::default()
		.with_balances(vec![(1, 20)])
		.build()
		.execute_with(|| {
			let do_initialize_at = <TestRuntime as Config>::DoInitializeAt::get();
			// set current block to DoInitializeAt
			System::set_block_number(do_initialize_at);
			// run on_finalize
			InflationManager::on_finalize(do_initialize_at);

			assert_eq!(
				<TestRuntime as Config>::Currency::total_issuance(),
				DefaultTotalIssuanceNum::get()
			);
			let account: AccountId =
				<TestRuntime as Config>::PotId::get().into_account_truncating();
			assert_eq!(Balances::usable_balance(account), DefaultTotalIssuanceNum::get() - 20);

			assert_noop!(
				InflationManager::transfer_all_pot(RuntimeOrigin::signed(1), 2),
				BadOrigin
			);

			InflationManager::transfer_all_pot(RawOrigin::Root.into(), 2).unwrap();
			assert_eq!(Balances::usable_balance(account), 0);
			assert_eq!(Balances::usable_balance(2), DefaultTotalIssuanceNum::get() - 20);
		})
}

#[test]
fn check_not_fund_token_at_delayed_tge_kickoff() {
	ExternalityBuilder::default()
		.with_balances(vec![(1, DefaultTotalIssuanceNum::get() + 50)])
		.build()
		.execute_with(|| {
			let do_initialize_at = <TestRuntime as Config>::DoInitializeAt::get();
			// set current block to DoInitializeAt
			System::set_block_number(do_initialize_at);
			// run on_finalize
			InflationManager::on_finalize(do_initialize_at);

			assert_eq!(
				<TestRuntime as Config>::Currency::total_issuance(),
				DefaultTotalIssuanceNum::get() + 50
			);
		})
}

#[test]
fn sanity_check_storage_migration_for_delayed_tge() {
	ExternalityBuilder::default().build().execute_with(|| {
		InflationManager::on_runtime_upgrade();
		let current_block = System::block_number() as u32;

		let total_issuance_before_upgrade = <TestRuntime as Config>::Currency::total_issuance();
		// Delayed TGE is set so this should have no affect
		InflationManager::on_runtime_upgrade();
		let snapshot = InflationManagerSnapshot::take_snapshot_at(current_block);
		// Delayed TGE sets inflation parameters as their default value
		let expected_inflation_parameters = InflationParametersT::default();

		assert_eq!(
			total_issuance_before_upgrade,
			<TestRuntime as Config>::Currency::total_issuance()
		);
		assert_eq!(snapshot.inflation_configuration, InflationConfigurationT::default());
		assert_eq!(snapshot.inflation_parameters, expected_inflation_parameters);
		assert_eq!(
			snapshot.do_recalculation_at as u64,
			<TestRuntime as Config>::DoInitializeAt::get()
		);
		assert_eq!(snapshot.current_year, 0u128);
		assert_eq!(
			snapshot.block_rewards,
			<TestRuntime as Config>::BlockRewardBeforeInitialize::get()
		);
	})
}

// In the DoRecalculationAt block,
// Block rewards are distributed first and then block rewards are updated
#[test]
fn parameters_update_as_expected_at_tge() {
	ExternalityBuilder::default().build().execute_with(|| {
		let do_initialize_at = <TestRuntime as Config>::DoInitializeAt::get() as u32;
		let target_block_at_genesis = do_initialize_at;

		let snapshots_before_tge = vec![
			InflationManagerSnapshot::take_snapshot_at(target_block_at_genesis - 2),
			InflationManagerSnapshot::take_snapshot_at(target_block_at_genesis - 1),
		];

		let snapshots_after_tge = vec![
			InflationManagerSnapshot::take_snapshot_at(target_block_at_genesis),
			InflationManagerSnapshot::take_snapshot_at(target_block_at_genesis + 1),
		];

		// Check that the snapshots before the TGE are consistent
		assert_eq!(snapshots_before_tge[0], snapshots_before_tge[1]);

		// check that the snapshots after the TGE are consistent
		assert_eq!(snapshots_after_tge[0], snapshots_after_tge[1]);

		// check that the snapshots before and after the new year are different
		assert_ne!(snapshots_before_tge[1], snapshots_after_tge[0]);

		// check that the snapshots after the TGE are consistent with the expected values
		assert_eq!(snapshots_after_tge[0].current_year, 1);
		assert_eq!(
			snapshots_after_tge[0].do_recalculation_at,
			snapshots_before_tge[0].do_recalculation_at + BLOCKS_PER_YEAR
		);
		assert_ne!(snapshots_after_tge[0].block_rewards, snapshots_before_tge[0].block_rewards);
	})
}

#[test]
fn stagnation_reached_as_expected_with_delayed_tge() {
	ExternalityBuilder::default().build().execute_with(|| {
		let inflation_configuration = InflationManager::inflation_configuration();
		let stagnation_snapshot_year = inflation_configuration.inflation_stagnation_year as usize;
		let last_snapshot_year = stagnation_snapshot_year + 1;
		let do_recalculation_at = InflationManager::do_recalculation_at() as u32;

		let yearly_snapshots: Vec<InflationManagerSnapshot> = (0..=last_snapshot_year)
			.map(|i| {
				InflationManagerSnapshot::take_snapshot_at(
					do_recalculation_at + (BLOCKS_PER_YEAR * i as u32),
				)
			})
			.collect();

		// verify snapshot inflation parameters - stagnation year index is (year - 1)
		assert_eq!(
			yearly_snapshots[stagnation_snapshot_year - 1]
				.inflation_parameters
				.inflation_rate,
			inflation_configuration.inflation_stagnation_rate
		);
		assert_eq!(
			yearly_snapshots[stagnation_snapshot_year - 1].current_year,
			inflation_configuration.inflation_stagnation_year
		);

		// ensure stagnation continues after stagnation year
		assert_eq!(
			yearly_snapshots[stagnation_snapshot_year].inflation_parameters,
			yearly_snapshots[last_snapshot_year].inflation_parameters
		);
	})
}

#[test]
fn inflation_parameters_correctness_as_expected() {
	ExternalityBuilder::default().build().execute_with(|| {
		let inflation_configuration: InflationConfigurationT =
			InflationManager::inflation_configuration();
		let last_snapshot_year = inflation_configuration.inflation_stagnation_year as usize - 1;
		let disinflation =
			Perbill::one() - inflation_configuration.inflation_parameters.disinflation_rate;
		let inflation = inflation_configuration.inflation_parameters.inflation_rate;
		let mut expected_yearly_inflation_parameters: Vec<InflationParametersT> = vec![];
		let do_recalculation_at = InflationManager::do_recalculation_at() as u32;

		// verify correct parameters before TGE
		let snapshot_before_tge =
			InflationManagerSnapshot::take_snapshot_at(do_recalculation_at - 1);
		assert_eq!(snapshot_before_tge.inflation_parameters, InflationParametersT::default());
		assert_eq!(snapshot_before_tge.current_year, 0);
		assert_eq!(
			snapshot_before_tge.do_recalculation_at as u64,
			<TestRuntime as Config>::DoInitializeAt::get()
		);

		let yearly_snapshots: Vec<InflationManagerSnapshot> = (0..last_snapshot_year)
			.map(|i| {
				InflationManagerSnapshot::take_snapshot_at(
					do_recalculation_at + (BLOCKS_PER_YEAR * i as u32),
				)
			})
			.collect();

		for i in 0..last_snapshot_year {
			// calculate expected inflation parameters manually
			let disinflation_rate = disinflation.saturating_pow(i.try_into().unwrap());
			let inflation_rate = inflation * disinflation_rate;
			expected_yearly_inflation_parameters
				.push(InflationParametersT { inflation_rate, disinflation_rate });
		}

		// verify snapshot inflation parameters
		for i in 0..last_snapshot_year {
			assert_eq!(
				yearly_snapshots[i].inflation_parameters,
				expected_yearly_inflation_parameters[i]
			);
		}
	})
}

/// Represents inflation manager storage snapshot at current block
#[derive(PartialEq, Eq, Clone, RuntimeDebug)]
struct InflationManagerSnapshot {
	inflation_configuration: InflationConfigurationT,
	inflation_parameters: InflationParametersT,
	do_recalculation_at: BlockNumber,
	current_year: u128,
	block_rewards: Balance,
}

impl InflationManagerSnapshot {
	fn new() -> Self {
		Self {
			inflation_configuration: InflationManager::inflation_configuration(),
			inflation_parameters: InflationManager::inflation_parameters(),
			do_recalculation_at: InflationManager::do_recalculation_at().try_into().unwrap(),
			current_year: InflationManager::current_year(),
			block_rewards: InflationManager::block_rewards(),
		}
	}
	fn take_snapshot_at(block_number: BlockNumber) -> Self {
		InflationManager::on_finalize(block_number.into());
		Self::new()
	}
}
