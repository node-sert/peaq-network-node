use crate::{self as inflation_manager, types, weights, Perbill};
use frame_support::PalletId;

use frame_support::{
	construct_runtime, parameter_types, sp_io::TestExternalities, traits::GenesisBuild,
	weights::Weight,
};

use sp_core::{ConstU32, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

pub(crate) type AccountId = u64;
pub(crate) type BlockNumber = u64;
pub(crate) use peaq_primitives_xcm::Balance;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;

/// Value shouldn't be less than 2 for testing purposes, otherwise we cannot test certain corner
/// cases.
pub(crate) const EXISTENTIAL_DEPOSIT: Balance = 2;

construct_runtime!(
	pub enum TestRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		InflationManager: inflation_manager::{Pallet, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub BlockWeights: frame_system::limits::BlockWeights =
		frame_system::limits::BlockWeights::simple_max(Weight::from_parts(1024, 0));
}

impl frame_system::Config for TestRuntime {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type Index = u64;
	type RuntimeCall = RuntimeCall;
	type BlockNumber = BlockNumber;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub const MaxLocks: u32 = 4;
	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
}

impl pallet_balances::Config for TestRuntime {
	type MaxLocks = MaxLocks;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type Balance = Balance;
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type FreezeIdentifier = ();
	type MaxHolds = ();
	type HoldIdentifier = ();
	type MaxFreezes = ();
}

parameter_types! {
	pub const MinimumPeriod: u64 = 3;
}

impl pallet_timestamp::Config for TestRuntime {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

parameter_types! {
	pub const InfaltionPot: PalletId = PalletId(*b"inflapot");
	pub const DefaultTotalIssuanceNum: Balance = 10_000_000_000_000_000_000_000_000;
	pub const DefaultInflationConfiguration: types::InflationConfiguration = types::InflationConfiguration {
		inflation_parameters: types::InflationParameters {
			inflation_rate: Perbill::from_perthousand(35u32),
			disinflation_rate: Perbill::from_percent(10),
		},
		inflation_stagnation_rate: Perbill::from_percent(1),
		inflation_stagnation_year: 13,
	};
	pub const InitializeInflationAt: BlockNumber = 10;
	pub const BlockRewardBeforeInitialize: Balance = 1000;
}

impl inflation_manager::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type PotId = InfaltionPot;
	type DefaultTotalIssuanceNum = DefaultTotalIssuanceNum;
	type DefaultInflationConfiguration = DefaultInflationConfiguration;
	type BoundedDataLen = ConstU32<1024>;
	type WeightInfo = weights::WeightInfo<TestRuntime>;
	type DoInitializeAt = InitializeInflationAt;
	type BlockRewardBeforeInitialize = BlockRewardBeforeInitialize;
}
pub struct ExternalityBuilder {
	// endowed accounts with balances
	balances: Vec<(AccountId, Balance)>,
}

impl Default for ExternalityBuilder {
	fn default() -> ExternalityBuilder {
		ExternalityBuilder {
			balances: vec![
				(1, 1_400_000_000_000_000_000_000_000_000),
				(2, 1_400_000_000_000_000_000_000_000_000),
				(3, 1_400_000_000_000_000_000_000_000_000),
			],
		}
	}
}

impl ExternalityBuilder {
	pub(crate) fn with_balances(mut self, balances: Vec<(AccountId, Balance)>) -> Self {
		self.balances = balances;
		self
	}

	pub fn build(self) -> TestExternalities {
		let mut storage =
			frame_system::GenesisConfig::default().build_storage::<TestRuntime>().unwrap();

		// This will cause some initial issuance
		pallet_balances::GenesisConfig::<TestRuntime> { balances: self.balances }
			.assimilate_storage(&mut storage)
			.ok();
		inflation_manager::GenesisConfig::<TestRuntime> { _phantom: Default::default() }
			.assimilate_storage(&mut storage)
			.ok();

		let mut ext = TestExternalities::from(storage);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
