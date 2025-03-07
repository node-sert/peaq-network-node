// KILT Blockchain – https://botlabs.org
// Copyright (C) 2019-2022 BOTLabs GmbH

// The KILT Blockchain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The KILT Blockchain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// If you feel like getting in touch with us, you can do so at info@botlabs.org

//! Unit testing

use std::{convert::TryInto, iter};

use frame_support::{
	assert_noop, assert_ok, storage::bounded_btree_map::BoundedBTreeMap,
	traits::EstimateNextSessionRotation, BoundedVec,
};
use frame_system::RawOrigin;
use pallet_balances::{BalanceLock, Error as BalancesError, Reasons};
use pallet_session::{SessionManager, ShouldEndSession};
use sp_runtime::{traits::Zero, Perbill, Permill, Perquintill, SaturatedConversion};

use crate::{
	mock::{
		almost_equal, events, last_event, roll_to, AccountId, Balance, Balances, BlockNumber,
		ExtBuilder, RuntimeEvent as MetaEvent, RuntimeOrigin, Session, StakePallet, System, Test,
		BLOCKS_PER_ROUND, BLOCK_REWARD_IN_GENESIS_SESSION, BLOCK_REWARD_IN_NORMAL_SESSION,
		DECIMALS,
	},
	set::OrderedSet,
	types::{
		BalanceOf, Candidate, CandidateStatus, DelegationCounter, Delegator, Reward, RoundInfo,
		Stake, StakeOf, TotalStake,
	},
	CandidatePool, Config, Error, Event, STAKING_ID,
};

#[test]
fn should_select_collators_genesis_session() {
	ExtBuilder::default()
		.with_balances(vec![
			(1, 20),
			(2, 20),
			(3, 20),
			(4, 20),
			(5, 20),
			(6, 20),
			(7, 20),
			(8, 20),
			(9, 20),
			(10, 20),
			(11, 20),
		])
		.with_collators(vec![(1, 20), (2, 20)])
		.build()
		.execute_with(|| {
			assert_eq!(
				StakePallet::new_session(0)
					.expect("first session must return new collators")
					.len(),
				2
			);
			assert_eq!(
				StakePallet::new_session(1)
					.expect("second session must return new collators")
					.len(),
				2
			);
		});
}

#[test]
fn genesis() {
	ExtBuilder::default()
		.with_balances(vec![
			(1, 1000),
			(2, 300),
			(3, 100),
			(4, 100),
			(5, 100),
			(6, 100),
			(7, 100),
			(8, 9),
			(9, 4),
		])
		.with_collators(vec![(1, 500), (2, 200)])
		.with_delegators(vec![(3, 1, 100), (4, 1, 100), (5, 2, 100), (6, 2, 100)])
		.build()
		.execute_with(|| {
			assert!(System::events().is_empty());

			// Collators
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 700, delegators: 400 }
			);
			assert_eq!(
				vec![
					StakeOf::<Test> { owner: 1, amount: 700 },
					StakeOf::<Test> { owner: 2, amount: 400 }
				]
				.try_into(),
				Ok(StakePallet::top_candidates().into_bounded_vec())
			);
			assert_eq!(CandidatePool::<Test>::count(), 2);

			// 1
			assert_eq!(Balances::usable_balance(1), 500);
			assert_eq!(Balances::free_balance(1), 1000);
			assert!(StakePallet::is_active_candidate(&1).is_some());
			assert_eq!(
				StakePallet::candidate_pool(1),
				Some(Candidate::<AccountId, Balance, <Test as Config>::MaxDelegatorsPerCollator> {
					id: 1,
					stake: 500,
					delegators: OrderedSet::from_sorted_set(
						vec![
							StakeOf::<Test> { owner: 3, amount: 100 },
							StakeOf::<Test> { owner: 4, amount: 100 }
						]
						.try_into()
						.unwrap()
					),
					total: 700,
					status: CandidateStatus::Active,
					commission: Default::default(),
				})
			);
			// 2
			assert_eq!(Balances::usable_balance(2), 100);
			assert_eq!(Balances::free_balance(2), 300);
			assert!(StakePallet::is_active_candidate(&2).is_some());
			assert_eq!(
				StakePallet::candidate_pool(2),
				Some(Candidate::<AccountId, Balance, <Test as Config>::MaxDelegatorsPerCollator> {
					id: 2,
					stake: 200,
					delegators: OrderedSet::from_sorted_set(
						vec![
							StakeOf::<Test> { owner: 5, amount: 100 },
							StakeOf::<Test> { owner: 6, amount: 100 }
						]
						.try_into()
						.unwrap()
					),
					total: 400,
					status: CandidateStatus::Active,
					commission: Default::default(),
				})
			);
			// Delegators
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 700, delegators: 400 }
			);
			for x in 3..7 {
				assert!(StakePallet::is_delegator(&x));
				assert_eq!(Balances::usable_balance(x), 0);
				assert_eq!(Balances::free_balance(x), 100);
			}
			// Uninvolved
			for x in 7..10 {
				assert!(!StakePallet::is_delegator(&x));
			}
			assert_eq!(Balances::free_balance(7), 100);
			assert_eq!(Balances::usable_balance(7), 100);
			assert_eq!(Balances::free_balance(8), 9);
			assert_eq!(Balances::usable_balance(8), 9);
			assert_eq!(Balances::free_balance(9), 4);
			assert_eq!(Balances::usable_balance(9), 4);

			// Safety first checks
			assert_eq!(
				StakePallet::max_selected_candidates(),
				<Test as Config>::MinCollators::get()
			);
			assert_eq!(
				StakePallet::round(),
				RoundInfo::new(0u32, 0u32.into(), <Test as Config>::DefaultBlocksPerRound::get())
			);
		});
	ExtBuilder::default()
		.with_balances(vec![
			(1, 100),
			(2, 100),
			(3, 100),
			(4, 100),
			(5, 100),
			(6, 100),
			(7, 100),
			(8, 100),
			(9, 100),
			(10, 100),
		])
		.with_collators(vec![(1, 20), (2, 20), (3, 20), (4, 20), (5, 10)])
		.with_delegators(vec![(6, 1, 10), (7, 1, 10), (8, 2, 10), (9, 2, 10), (10, 1, 10)])
		.build()
		.execute_with(|| {
			assert!(System::events().is_empty());
			assert_eq!(CandidatePool::<Test>::count(), 5);

			// Collators
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 40, delegators: 50 }
			);
			assert_eq!(
				Ok(StakePallet::top_candidates().into_bounded_vec()),
				vec![
					StakeOf::<Test> { owner: 1, amount: 50 },
					StakeOf::<Test> { owner: 2, amount: 40 },
					StakeOf::<Test> { owner: 3, amount: 20 },
					StakeOf::<Test> { owner: 4, amount: 20 },
					StakeOf::<Test> { owner: 5, amount: 10 }
				]
				.try_into()
			);
			for x in 1..5 {
				assert!(StakePallet::is_active_candidate(&x).is_some());
				assert_eq!(Balances::free_balance(x), 100);
				assert_eq!(Balances::usable_balance(x), 80);
			}
			assert!(StakePallet::is_active_candidate(&5).is_some());
			assert_eq!(Balances::free_balance(5), 100);
			assert_eq!(Balances::usable_balance(5), 90);
			// Delegators
			for x in 6..11 {
				assert!(StakePallet::is_delegator(&x));
				assert_eq!(Balances::free_balance(x), 100);
				assert_eq!(Balances::usable_balance(x), 90);
			}

			// Safety first checks
			assert_eq!(
				StakePallet::max_selected_candidates(),
				<Test as Config>::MinCollators::get()
			);
			assert_eq!(
				StakePallet::round(),
				RoundInfo::new(0, 0, <Test as Config>::DefaultBlocksPerRound::get())
			);
		});
}

#[test]
fn join_collator_candidates() {
	ExtBuilder::default()
		.with_balances(vec![
			(1, 1000),
			(2, 300),
			(3, 100),
			(4, 100),
			(5, 100),
			(6, 100),
			(7, 100),
			(8, 9),
			(9, 4),
			(10, 161_000_000 * DECIMALS),
		])
		.with_collators(vec![(1, 500), (2, 200)])
		.with_delegators(vec![(3, 1, 100), (4, 1, 100), (5, 2, 100), (6, 2, 100)])
		.build()
		.execute_with(|| {
			assert_eq!(CandidatePool::<Test>::count(), 2);
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 700, delegators: 400 }
			);
			assert_noop!(
				StakePallet::join_candidates(RuntimeOrigin::signed(1), 11u128,),
				Error::<Test>::CandidateExists
			);
			assert_noop!(
				StakePallet::join_delegators(RuntimeOrigin::signed(1), 1, 11u128,),
				Error::<Test>::CandidateExists
			);
			assert_noop!(
				StakePallet::join_candidates(RuntimeOrigin::signed(3), 11u128,),
				Error::<Test>::DelegatorExists
			);
			assert_noop!(
				StakePallet::join_candidates(RuntimeOrigin::signed(7), 9u128,),
				Error::<Test>::ValStakeBelowMin
			);
			assert_noop!(
				StakePallet::join_candidates(RuntimeOrigin::signed(8), 10u128,),
				BalancesError::<Test>::InsufficientBalance
			);

			assert_eq!(CandidatePool::<Test>::count(), 2);
			assert!(System::events().is_empty());

			assert_ok!(StakePallet::set_max_selected_candidates(RuntimeOrigin::root(), 5));
			assert_ok!(StakePallet::join_candidates(RuntimeOrigin::signed(7), 10u128,));
			assert_eq!(
				last_event(),
				MetaEvent::StakePallet(Event::JoinedCollatorCandidates(7, 10u128))
			);

			// MaxCollatorCandidateStake
			assert_noop!(
				StakePallet::join_candidates(RuntimeOrigin::signed(10), 161_000_000 * DECIMALS),
				Error::<Test>::ValStakeAboveMax
			);
			assert_ok!(StakePallet::join_candidates(
				RuntimeOrigin::signed(10),
				StakePallet::max_candidate_stake()
			));
			assert_eq!(CandidatePool::<Test>::count(), 4);

			assert_eq!(
				last_event(),
				MetaEvent::StakePallet(Event::JoinedCollatorCandidates(
					10,
					StakePallet::max_candidate_stake(),
				))
			);
		});
}

#[test]
fn collator_exit_executes_after_delay() {
	ExtBuilder::default()
		.with_balances(vec![
			(1, 1000),
			(2, 300),
			(3, 110),
			(4, 100),
			(5, 100),
			(6, 100),
			(7, 100),
			(8, 9),
			(9, 4),
		])
		.with_collators(vec![(1, 500), (2, 200), (7, 100)])
		.with_delegators(vec![(3, 1, 100), (4, 1, 100), (5, 2, 100), (6, 2, 100)])
		.build()
		.execute_with(|| {
			assert_eq!(CandidatePool::<Test>::count(), 3);
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 700, delegators: 400 }
			);
			assert_ok!(StakePallet::set_max_selected_candidates(RuntimeOrigin::root(), 5));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 800, delegators: 400 }
			);
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2, 7]);
			roll_to(4, vec![]);
			assert_noop!(
				StakePallet::init_leave_candidates(RuntimeOrigin::signed(3)),
				Error::<Test>::CandidateNotFound
			);

			roll_to(11, vec![]);
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(2)));
			// Still three, candidate didn't leave yet
			assert_eq!(CandidatePool::<Test>::count(), 3);
			assert_noop!(
				StakePallet::delegate_another_candidate(RuntimeOrigin::signed(3), 2, 10),
				Error::<Test>::CannotDelegateIfLeaving
			);
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 7]);
			assert_eq!(last_event(), MetaEvent::StakePallet(Event::CollatorScheduledExit(2, 2, 4)));
			let info = StakePallet::candidate_pool(2).unwrap();
			assert_eq!(info.status, CandidateStatus::Leaving(4));

			roll_to(21, vec![]);
			assert_ok!(StakePallet::execute_leave_candidates(RuntimeOrigin::signed(2), 2));
			assert_eq!(CandidatePool::<Test>::count(), 2);

			// we must exclude leaving collators from rewards while
			// holding them retroactively accountable for previous faults
			// (within the last T::StakeDuration blocks)
			roll_to(25, vec![]);
			let expected = vec![
				Event::MaxSelectedCandidatesSet(2, 5),
				Event::NewRound(5, 1),
				Event::NewRound(10, 2),
				Event::LeftTopCandidates(2),
				Event::CollatorScheduledExit(2, 2, 4),
				Event::NewRound(15, 3),
				Event::NewRound(20, 4),
				Event::CandidateLeft(2, 400),
				Event::NewRound(25, 5),
			];
			assert_eq!(events(), expected);
		});
}

#[test]
fn collator_selection_chooses_top_candidates() {
	ExtBuilder::default()
		.with_balances(vec![
			(1, 1000),
			(2, 1000),
			(3, 1000),
			(4, 1000),
			(5, 1000),
			(6, 1000),
			(7, 33),
			(8, 33),
			(9, 33),
		])
		.with_collators(vec![(1, 100), (2, 90), (3, 80), (4, 70), (5, 60), (6, 50)])
		.build()
		.execute_with(|| {
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2]);
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 190, delegators: 0 }
			);
			assert_ok!(StakePallet::set_max_selected_candidates(RuntimeOrigin::root(), 5));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 400, delegators: 0 }
			);
			roll_to(8, vec![]);
			// should choose top MaxSelectedCandidates (5), in order
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2, 3, 4, 5]);
			let expected = vec![Event::MaxSelectedCandidatesSet(2, 5), Event::NewRound(5, 1)];
			assert_eq!(events(), expected);
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(6)));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2, 3, 4, 5],);
			assert_eq!(last_event(), MetaEvent::StakePallet(Event::CollatorScheduledExit(1, 6, 3)));

			roll_to(15, vec![]);
			assert_ok!(StakePallet::execute_leave_candidates(RuntimeOrigin::signed(6), 6));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2, 3, 4, 5]);

			roll_to(21, vec![]);
			assert_ok!(StakePallet::join_candidates(RuntimeOrigin::signed(6), 69u128));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2, 3, 4, 6]);
			assert_eq!(
				last_event(),
				MetaEvent::StakePallet(Event::JoinedCollatorCandidates(6, 69u128))
			);

			roll_to(27, vec![]);
			// should choose top MaxSelectedCandidates (5), in order
			let expected = vec![
				Event::MaxSelectedCandidatesSet(2, 5),
				Event::NewRound(5, 1),
				Event::LeftTopCandidates(6),
				Event::CollatorScheduledExit(1, 6, 3),
				// TotalCollatorStake is updated once candidate 6 left in
				// `execute_delayed_collator_exits`
				Event::NewRound(10, 2),
				Event::NewRound(15, 3),
				Event::CandidateLeft(6, 50),
				Event::NewRound(20, 4),
				// 5 had staked 60 which was exceeded by 69 of 6
				Event::EnteredTopCandidates(6),
				Event::JoinedCollatorCandidates(6, 69),
				Event::NewRound(25, 5),
			];
			assert_eq!(events(), expected);
		});
}

#[test]
fn exit_queue_with_events() {
	ExtBuilder::default()
		.with_balances(vec![
			(1, 1000),
			(2, 1000),
			(3, 1000),
			(4, 1000),
			(5, 1000),
			(6, 1000),
			(7, 33),
			(8, 33),
			(9, 33),
		])
		.with_collators(vec![(1, 100), (2, 90), (3, 80), (4, 70), (5, 60), (6, 50)])
		.build()
		.execute_with(|| {
			assert_eq!(CandidatePool::<Test>::count(), 6);
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2]);
			assert_ok!(StakePallet::set_max_selected_candidates(RuntimeOrigin::root(), 5));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2, 3, 4, 5]);

			roll_to(8, vec![]);
			// should choose top MaxSelectedCandidates (5), in order
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2, 3, 4, 5]);
			let mut expected = vec![Event::MaxSelectedCandidatesSet(2, 5), Event::NewRound(5, 1)];
			assert_eq!(events(), expected);
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(6)));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2, 3, 4, 5]);
			assert_eq!(last_event(), MetaEvent::StakePallet(Event::CollatorScheduledExit(1, 6, 3)));

			roll_to(11, vec![]);
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(5)));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2, 3, 4]);
			assert_eq!(last_event(), MetaEvent::StakePallet(Event::CollatorScheduledExit(2, 5, 4)));

			assert_eq!(CandidatePool::<Test>::count(), 6, "No collators have left yet.");
			roll_to(16, vec![]);
			assert_ok!(StakePallet::execute_leave_candidates(RuntimeOrigin::signed(6), 6));
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(4)));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2, 3]);
			assert_eq!(last_event(), MetaEvent::StakePallet(Event::CollatorScheduledExit(3, 4, 5)));
			assert_noop!(
				StakePallet::init_leave_candidates(RuntimeOrigin::signed(4)),
				Error::<Test>::AlreadyLeaving
			);

			assert_eq!(CandidatePool::<Test>::count(), 5, "Collator #5 left.");
			roll_to(20, vec![]);
			assert_ok!(StakePallet::execute_leave_candidates(RuntimeOrigin::signed(5), 5));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2, 3]);
			assert_eq!(CandidatePool::<Test>::count(), 4, "Two out of six collators left.");

			roll_to(26, vec![]);
			assert_ok!(StakePallet::execute_leave_candidates(RuntimeOrigin::signed(4), 4));
			assert_eq!(CandidatePool::<Test>::count(), 3, "Three out of six collators left.");

			roll_to(30, vec![]);
			let mut new_events = vec![
				Event::LeftTopCandidates(6),
				Event::CollatorScheduledExit(1, 6, 3),
				Event::NewRound(10, 2),
				Event::LeftTopCandidates(5),
				Event::CollatorScheduledExit(2, 5, 4),
				Event::NewRound(15, 3),
				Event::CandidateLeft(6, 50),
				Event::LeftTopCandidates(4),
				Event::CollatorScheduledExit(3, 4, 5),
				Event::NewRound(20, 4),
				Event::CandidateLeft(5, 60),
				Event::NewRound(25, 5),
				Event::CandidateLeft(4, 70),
				Event::NewRound(30, 6),
			];
			expected.append(&mut new_events);
			assert_eq!(events(), expected);
		});
}

#[test]
fn execute_leave_candidates_with_delay() {
	ExtBuilder::default()
		.with_balances(vec![
			(1, 1000),
			(2, 1000),
			(3, 1000),
			(4, 1000),
			(5, 1000),
			(6, 1000),
			(7, 1000),
			(8, 1000),
			(9, 1000),
			(10, 1000),
			(11, 1000),
			(12, 1000),
			(13, 1000),
			(14, 1000),
		])
		.with_collators(vec![
			(1, 10),
			(2, 20),
			(3, 30),
			(4, 40),
			(5, 50),
			(6, 60),
			(7, 70),
			(8, 80),
			(9, 90),
			(10, 100),
		])
		.with_delegators(vec![(11, 1, 110), (12, 1, 120), (13, 2, 130), (14, 2, 140)])
		.build()
		.execute_with(|| {
			assert_eq!(CandidatePool::<Test>::count(), 10);
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 30, delegators: 500 }
			);
			assert_ok!(StakePallet::set_max_selected_candidates(RuntimeOrigin::root(), 5));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 300, delegators: 500 }
			);

			roll_to(5, vec![]);
			// should choose top MaxSelectedCandidates (5), in order
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![2, 1, 10, 9, 8]);
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(10)));
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(9)));
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(1)));
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(7)));
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(6)));
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(5)));
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(8)));
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(2)));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![4, 3]);
			for owner in [1, 2, 5, 6, 7, 8, 9, 10].iter() {
				assert!(StakePallet::candidate_pool(owner)
					.unwrap()
					.can_exit(1 + <Test as Config>::ExitQueueDelay::get()));
			}
			let total_stake = TotalStake { collators: 70, delegators: 0 };
			assert_eq!(StakePallet::total_collator_stake(), total_stake);
			assert_eq!(
				StakePallet::candidate_pool(1),
				Some(Candidate::<AccountId, Balance, <Test as Config>::MaxDelegatorsPerCollator> {
					id: 1,
					stake: 10,
					delegators: OrderedSet::from(
						vec![
							StakeOf::<Test> { owner: 11, amount: 110 },
							StakeOf::<Test> { owner: 12, amount: 120 }
						]
						.try_into()
						.unwrap()
					),
					total: 240,
					status: CandidateStatus::Leaving(3),
					commission: Default::default(),
				})
			);
			assert_eq!(
				StakePallet::candidate_pool(2),
				Some(Candidate::<AccountId, Balance, <Test as Config>::MaxDelegatorsPerCollator> {
					id: 2,
					stake: 20,
					delegators: OrderedSet::from(
						vec![
							StakeOf::<Test> { owner: 13, amount: 130 },
							StakeOf::<Test> { owner: 14, amount: 140 }
						]
						.try_into()
						.unwrap()
					),
					total: 290,
					status: CandidateStatus::Leaving(3),
					commission: Default::default(),
				})
			);
			for collator in 5u64..=10u64 {
				assert_eq!(
					StakePallet::candidate_pool(collator),
					Some(Candidate::<
						AccountId,
						Balance,
						<Test as Config>::MaxDelegatorsPerCollator,
					> {
						id: collator,
						stake: collator as u128 * 10u128,
						delegators: OrderedSet::from(BoundedVec::default()),
						total: collator as u128 * 10u128,
						status: CandidateStatus::Leaving(3),
						commission: Default::default(),
					})
				);
				assert!(StakePallet::is_active_candidate(&collator).is_some());
				assert!(StakePallet::unstaking(collator).is_empty());
			}
			assert_eq!(
				StakePallet::delegator_state(11),
				Some(Delegator::<AccountId, Balance, <Test as Config>::MaxCollatorsPerDelegator> {
					delegations: OrderedSet::from(
						vec![StakeOf::<Test> { owner: 1, amount: 110 }].try_into().unwrap()
					),
					total: 110
				})
			);
			assert_eq!(
				StakePallet::delegator_state(12),
				Some(Delegator::<AccountId, Balance, <Test as Config>::MaxCollatorsPerDelegator> {
					delegations: OrderedSet::from(
						vec![StakeOf::<Test> { owner: 1, amount: 120 }].try_into().unwrap()
					),
					total: 120
				})
			);
			assert_eq!(
				StakePallet::delegator_state(13),
				Some(Delegator::<AccountId, Balance, <Test as Config>::MaxCollatorsPerDelegator> {
					delegations: OrderedSet::from(
						vec![StakeOf::<Test> { owner: 2, amount: 130 }].try_into().unwrap()
					),
					total: 130
				})
			);
			assert_eq!(
				StakePallet::delegator_state(14),
				Some(Delegator::<AccountId, Balance, <Test as Config>::MaxCollatorsPerDelegator> {
					delegations: OrderedSet::from(
						vec![StakeOf::<Test> { owner: 2, amount: 140 }].try_into().unwrap()
					),
					total: 140
				})
			);
			for delegator in 11u64..=14u64 {
				assert!(StakePallet::is_delegator(&delegator));
				assert!(StakePallet::unstaking(delegator).is_empty());
			}

			// exits cannot be executed yet but in the next round
			roll_to(10, vec![]);
			assert_eq!(StakePallet::total_collator_stake(), total_stake);
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![4, 3]);
			for owner in [1, 2, 5, 6, 7, 8, 9, 10].iter() {
				assert!(StakePallet::candidate_pool(owner)
					.unwrap()
					.can_exit(1 + <Test as Config>::ExitQueueDelay::get()));
				assert_noop!(
					StakePallet::execute_leave_candidates(RuntimeOrigin::signed(*owner), *owner),
					Error::<Test>::CannotLeaveYet
				);
			}
			assert_eq!(StakePallet::total_collator_stake(), total_stake);
			assert_eq!(
				StakePallet::candidate_pool(1),
				Some(Candidate::<AccountId, Balance, <Test as Config>::MaxDelegatorsPerCollator> {
					id: 1,
					stake: 10,
					delegators: OrderedSet::from(
						vec![
							StakeOf::<Test> { owner: 11, amount: 110 },
							StakeOf::<Test> { owner: 12, amount: 120 }
						]
						.try_into()
						.unwrap()
					),
					total: 240,
					status: CandidateStatus::Leaving(3),
					commission: Default::default(),
				})
			);
			assert_eq!(
				StakePallet::candidate_pool(2),
				Some(Candidate::<AccountId, Balance, <Test as Config>::MaxDelegatorsPerCollator> {
					id: 2,
					stake: 20,
					delegators: OrderedSet::from(
						vec![
							StakeOf::<Test> { owner: 13, amount: 130 },
							StakeOf::<Test> { owner: 14, amount: 140 }
						]
						.try_into()
						.unwrap()
					),
					total: 290,
					status: CandidateStatus::Leaving(3),
					commission: Default::default(),
				})
			);
			for collator in 5u64..=10u64 {
				assert_eq!(
					StakePallet::candidate_pool(collator),
					Some(Candidate::<
						AccountId,
						Balance,
						<Test as Config>::MaxDelegatorsPerCollator,
					> {
						id: collator,
						stake: collator as u128 * 10u128,
						delegators: OrderedSet::from(BoundedVec::default()),
						total: collator as u128 * 10u128,
						status: CandidateStatus::Leaving(3),
						commission: Default::default(),
					})
				);
				assert!(StakePallet::is_active_candidate(&collator).is_some());
				assert!(StakePallet::unstaking(collator).is_empty());
			}
			assert_eq!(
				StakePallet::delegator_state(11),
				Some(Delegator::<AccountId, Balance, <Test as Config>::MaxCollatorsPerDelegator> {
					delegations: OrderedSet::from(
						vec![StakeOf::<Test> { owner: 1, amount: 110 }].try_into().unwrap()
					),
					total: 110
				})
			);
			assert_eq!(
				StakePallet::delegator_state(12),
				Some(Delegator::<AccountId, Balance, <Test as Config>::MaxCollatorsPerDelegator> {
					delegations: OrderedSet::from(
						vec![StakeOf::<Test> { owner: 1, amount: 120 }].try_into().unwrap()
					),
					total: 120
				})
			);
			assert_eq!(
				StakePallet::delegator_state(13),
				Some(Delegator::<AccountId, Balance, <Test as Config>::MaxCollatorsPerDelegator> {
					delegations: OrderedSet::from(
						vec![StakeOf::<Test> { owner: 2, amount: 130 }].try_into().unwrap()
					),
					total: 130
				})
			);
			assert_eq!(
				StakePallet::delegator_state(14),
				Some(Delegator::<AccountId, Balance, <Test as Config>::MaxCollatorsPerDelegator> {
					delegations: OrderedSet::from(
						vec![StakeOf::<Test> { owner: 2, amount: 140 }].try_into().unwrap()
					),
					total: 140
				})
			);
			for delegator in 11u64..=14u64 {
				assert!(StakePallet::is_delegator(&delegator));
				assert!(StakePallet::unstaking(delegator).is_empty());
			}

			// first five exits are executed
			roll_to(15, vec![]);
			assert_eq!(StakePallet::total_collator_stake(), total_stake);
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![4, 3]);
			for collator in [1u64, 2u64, 5u64, 6u64, 7u64].iter() {
				assert_ok!(StakePallet::execute_leave_candidates(
					RuntimeOrigin::signed(*collator),
					*collator
				));
				assert!(StakePallet::candidate_pool(&collator).is_none());
				assert!(StakePallet::is_active_candidate(collator).is_none());
				assert_eq!(StakePallet::unstaking(collator).len(), 1);
			}
			assert_eq!(CandidatePool::<Test>::count(), 5, "Five collators left.");

			assert_eq!(StakePallet::total_collator_stake(), total_stake);
			for delegator in 11u64..=14u64 {
				assert!(!StakePallet::is_delegator(&delegator));
				assert_eq!(StakePallet::unstaking(delegator).len(), 1);
			}

			// last 3 exits are executed
			roll_to(20, vec![]);
			for collator in 8u64..=10u64 {
				assert_ok!(StakePallet::execute_leave_candidates(
					RuntimeOrigin::signed(collator),
					collator
				));
				assert!(StakePallet::candidate_pool(collator).is_none());
				assert!(StakePallet::is_active_candidate(&collator).is_none());
				assert_eq!(StakePallet::unstaking(collator).len(), 1);
			}
			assert_eq!(CandidatePool::<Test>::count(), 2, "3 collators left.");
		});
}

#[test]
fn multiple_delegations() {
	ExtBuilder::default()
		.with_balances(vec![
			(1, 100),
			(2, 100),
			(3, 100),
			(4, 100),
			(5, 100),
			(6, 100),
			(7, 100),
			(8, 100),
			(9, 100),
			(10, 100),
			(11, 100),
			(12, 100),
		])
		.with_collators(vec![(1, 20), (2, 20), (3, 20), (4, 20), (5, 10)])
		.with_delegators(vec![(6, 1, 10), (7, 1, 10), (8, 2, 10), (9, 2, 10), (10, 1, 10)])
		.set_blocks_per_round(5)
		.build()
		.execute_with(|| {
			assert_ok!(StakePallet::set_max_selected_candidates(RuntimeOrigin::root(), 5));
			roll_to(8, vec![]);
			// chooses top MaxSelectedCandidates (5), in order
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2, 3, 4, 5]);
			let mut expected = vec![Event::MaxSelectedCandidatesSet(2, 5), Event::NewRound(5, 1)];
			assert_eq!(events(), expected);
			assert_noop!(
				StakePallet::delegate_another_candidate(RuntimeOrigin::signed(6), 1, 10),
				Error::<Test>::AlreadyDelegatedCollator,
			);
			assert_noop!(
				StakePallet::delegate_another_candidate(RuntimeOrigin::signed(6), 2, 2),
				Error::<Test>::DelegationBelowMin,
			);
			assert_ok!(StakePallet::delegate_another_candidate(RuntimeOrigin::signed(6), 2, 10));
			assert_ok!(StakePallet::delegate_another_candidate(RuntimeOrigin::signed(6), 4, 10));
			assert_ok!(StakePallet::delegate_another_candidate(RuntimeOrigin::signed(6), 3, 10));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2, 4, 3, 5]);
			assert_noop!(
				StakePallet::delegate_another_candidate(RuntimeOrigin::signed(6), 5, 10),
				Error::<Test>::MaxCollatorsPerDelegatorExceeded,
			);

			roll_to(16, vec![]);
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2, 4, 3, 5]);
			let mut new = vec![
				Event::Delegation(6, 10, 2, 50),
				Event::Delegation(6, 10, 4, 30),
				Event::Delegation(6, 10, 3, 30),
				Event::NewRound(10, 2),
				Event::NewRound(15, 3),
			];
			expected.append(&mut new);
			assert_eq!(events(), expected);

			roll_to(21, vec![]);
			assert_ok!(StakePallet::delegate_another_candidate(RuntimeOrigin::signed(7), 2, 80));
			assert_noop!(
				StakePallet::delegate_another_candidate(RuntimeOrigin::signed(7), 3, 11),
				BalancesError::<Test>::InsufficientBalance
			);
			assert_noop!(
				StakePallet::delegate_another_candidate(RuntimeOrigin::signed(10), 2, 10),
				Error::<Test>::TooManyDelegators
			);
			// kick 6
			assert!(StakePallet::unstaking(6).is_empty());
			assert_ok!(StakePallet::delegate_another_candidate(RuntimeOrigin::signed(10), 2, 11));
			assert!(StakePallet::delegator_state(6).is_some());
			assert_eq!(StakePallet::unstaking(6).get(&23), Some(&10u128));
			// kick 9
			assert!(StakePallet::unstaking(9).is_empty());
			assert_ok!(StakePallet::join_delegators(RuntimeOrigin::signed(11), 2, 11));
			assert!(StakePallet::delegator_state(9).is_none());
			assert_eq!(StakePallet::unstaking(9).get(&23), Some(&10u128));
			assert!(!StakePallet::candidate_pool(2)
				.unwrap()
				.delegators
				.contains(&StakeOf::<Test> { owner: 9, amount: 10 }));

			roll_to(26, vec![]);
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![2, 1, 4, 3, 5]);
			let mut new2 = vec![
				Event::NewRound(20, 4),
				Event::Delegation(7, 80, 2, 130),
				Event::DelegationReplaced(10, 11, 6, 10, 2, 131),
				Event::Delegation(10, 11, 2, 131),
				Event::DelegationReplaced(11, 11, 9, 10, 2, 132),
				Event::Delegation(11, 11, 2, 132),
				Event::NewRound(25, 5),
			];
			expected.append(&mut new2);
			assert_eq!(events(), expected);
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(2)));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 4, 3, 5]);
			assert_eq!(last_event(), MetaEvent::StakePallet(Event::CollatorScheduledExit(5, 2, 7)));

			roll_to(31, vec![]);
			let mut new3 = vec![
				Event::LeftTopCandidates(2),
				Event::CollatorScheduledExit(5, 2, 7),
				Event::NewRound(30, 6),
			];
			expected.append(&mut new3);
			assert_eq!(events(), expected);

			// test join_delegator errors
			assert_ok!(StakePallet::delegate_another_candidate(RuntimeOrigin::signed(8), 1, 10));
			assert_noop!(
				StakePallet::join_delegators(RuntimeOrigin::signed(12), 1, 10),
				Error::<Test>::TooManyDelegators
			);
			assert_noop!(
				StakePallet::delegate_another_candidate(RuntimeOrigin::signed(12), 1, 10),
				Error::<Test>::NotYetDelegating
			);
			assert_ok!(StakePallet::join_delegators(RuntimeOrigin::signed(12), 1, 11));

			// verify that delegations are removed after collator leaves, not before
			assert_eq!(StakePallet::delegator_state(7).unwrap().total, 90);
			assert_eq!(StakePallet::delegator_state(7).unwrap().delegations.len(), 2usize);
			assert_eq!(StakePallet::delegator_state(11).unwrap().total, 11);
			assert_eq!(StakePallet::delegator_state(11).unwrap().delegations.len(), 1usize);
			// 6 already has 10 in
			assert_eq!(Balances::usable_balance(7), 10);
			assert_eq!(Balances::usable_balance(11), 89);
			assert_eq!(Balances::free_balance(7), 100);
			assert_eq!(Balances::free_balance(11), 100);

			roll_to(35, vec![]);
			assert_ok!(StakePallet::execute_leave_candidates(RuntimeOrigin::signed(2), 2));
			let mut unbonding_7: BoundedBTreeMap<
				BlockNumber,
				BalanceOf<Test>,
				<Test as Config>::MaxUnstakeRequests,
			> = BoundedBTreeMap::new();
			assert_ok!(
				unbonding_7.try_insert(35u64 + <Test as Config>::StakeDuration::get() as u64, 80)
			);
			assert_eq!(StakePallet::unstaking(7), unbonding_7);
			let mut unbonding_11: BoundedBTreeMap<
				BlockNumber,
				BalanceOf<Test>,
				<Test as Config>::MaxUnstakeRequests,
			> = BoundedBTreeMap::new();
			assert_ok!(
				unbonding_11.try_insert(35u64 + <Test as Config>::StakeDuration::get() as u64, 11)
			);
			assert_eq!(StakePallet::unstaking(11), unbonding_11);

			roll_to(37, vec![]);
			assert_eq!(StakePallet::delegator_state(7).unwrap().total, 10);
			assert!(StakePallet::delegator_state(11).is_none());
			assert_eq!(StakePallet::delegator_state(7).unwrap().delegations.len(), 1usize);
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(7), 7));
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(11), 11));
			assert_noop!(
				StakePallet::unlock_unstaked(RuntimeOrigin::signed(12), 12),
				Error::<Test>::UnstakingIsEmpty
			);
			assert_eq!(Balances::usable_balance(11), 100);
			assert_eq!(Balances::usable_balance(7), 90);
			assert_eq!(Balances::free_balance(11), 100);
			assert_eq!(Balances::free_balance(7), 100);
		});
}

#[test]
fn should_update_total_stake() {
	ExtBuilder::default()
		.with_balances(vec![
			(1, 100),
			(2, 100),
			(3, 100),
			(4, 100),
			(5, 100),
			(6, 100),
			(7, 100),
			(8, 100),
			(9, 100),
			(10, 100),
			(11, 161_000_000 * DECIMALS),
		])
		.with_collators(vec![(1, 20), (2, 20), (3, 20), (4, 20), (5, 10)])
		.with_delegators(vec![(7, 1, 10), (8, 2, 10), (9, 2, 10)])
		.set_blocks_per_round(5)
		.build()
		.execute_with(|| {
			let mut old_stake = StakePallet::total_collator_stake();
			assert_eq!(old_stake, TotalStake { collators: 40, delegators: 30 });
			assert_ok!(StakePallet::candidate_stake_more(RuntimeOrigin::signed(1), 50));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: old_stake.collators + 50, ..old_stake }
			);

			old_stake = StakePallet::total_collator_stake();
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(1), 50));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: old_stake.collators - 50, ..old_stake }
			);

			old_stake = StakePallet::total_collator_stake();
			assert_ok!(StakePallet::delegator_stake_more(RuntimeOrigin::signed(7), 1, 50));
			assert_noop!(
				StakePallet::delegator_stake_more(RuntimeOrigin::signed(7), 1, 0),
				Error::<Test>::ValStakeZero
			);
			assert_noop!(
				StakePallet::delegator_stake_less(RuntimeOrigin::signed(7), 1, 0),
				Error::<Test>::ValStakeZero
			);
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { delegators: old_stake.delegators + 50, ..old_stake }
			);

			old_stake = StakePallet::total_collator_stake();
			assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(7), 1, 50));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { delegators: old_stake.delegators - 50, ..old_stake }
			);

			old_stake = StakePallet::total_collator_stake();
			assert_ok!(StakePallet::join_delegators(RuntimeOrigin::signed(11), 1, 200));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { delegators: old_stake.delegators + 200, ..old_stake }
			);

			old_stake = StakePallet::total_collator_stake();
			assert_ok!(StakePallet::delegate_another_candidate(RuntimeOrigin::signed(11), 2, 150));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { delegators: old_stake.delegators + 150, ..old_stake }
			);

			old_stake = StakePallet::total_collator_stake();
			assert_eq!(StakePallet::delegator_state(11).unwrap().total, 350);
			assert_ok!(StakePallet::leave_delegators(RuntimeOrigin::signed(11)));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { delegators: old_stake.delegators - 350, ..old_stake }
			);

			let old_stake = StakePallet::total_collator_stake();
			assert_eq!(StakePallet::delegator_state(8).unwrap().total, 10);
			assert_ok!(StakePallet::revoke_delegation(RuntimeOrigin::signed(8), 2));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { delegators: old_stake.delegators - 10, ..old_stake }
			);

			// should immediately affect total stake because collator can't be chosen in
			// active set from now on, thus delegated stake is reduced
			let old_stake = StakePallet::total_collator_stake();
			assert_eq!(StakePallet::candidate_pool(2).unwrap().total, 30);
			assert_eq!(StakePallet::candidate_pool(2).unwrap().stake, 20);
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![2, 1]);
			assert_eq!(
				StakePallet::candidate_pool(2).unwrap().stake,
				StakePallet::candidate_pool(3).unwrap().stake
			);
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(2)));
			let old_stake = TotalStake {
				delegators: old_stake.delegators - 10,
				// total active collator stake is unchanged because number of selected candidates is
				// 2 and 2's replacement has the same self stake as 2
				collators: old_stake.collators,
			};
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 3]);
			assert_eq!(StakePallet::total_collator_stake(), old_stake);

			// shouldn't change total stake when 2 leaves
			roll_to(10, vec![]);
			assert_eq!(StakePallet::total_collator_stake(), old_stake);
			assert_ok!(StakePallet::execute_leave_candidates(RuntimeOrigin::signed(2), 2));
			assert_eq!(StakePallet::total_collator_stake(), old_stake);
		})
}

#[test]
fn collators_bond() {
	ExtBuilder::default()
		.with_balances(vec![
			(1, 100),
			(2, 100),
			(3, 100),
			(4, 100),
			(5, 100),
			(6, 100),
			(7, 100),
			(8, 100),
			(9, 100),
			(10, 100),
			(11, 161_000_000 * DECIMALS),
		])
		.with_collators(vec![(1, 20), (2, 20), (3, 20), (4, 20), (5, 10)])
		.with_delegators(vec![(6, 1, 10), (7, 1, 10), (8, 2, 10), (9, 2, 10), (10, 1, 10)])
		.set_blocks_per_round(5)
		.build()
		.execute_with(|| {
			roll_to(4, vec![]);
			assert_noop!(
				StakePallet::candidate_stake_more(RuntimeOrigin::signed(6), 50),
				Error::<Test>::CandidateNotFound
			);
			assert_noop!(
				StakePallet::candidate_stake_less(RuntimeOrigin::signed(6), 50),
				Error::<Test>::CandidateNotFound
			);
			assert_noop!(
				StakePallet::delegate_another_candidate(RuntimeOrigin::signed(6), 50, 10),
				Error::<Test>::CandidateNotFound
			);
			assert_ok!(StakePallet::candidate_stake_more(RuntimeOrigin::signed(1), 50));
			assert_noop!(
				StakePallet::candidate_stake_more(RuntimeOrigin::signed(1), 40),
				BalancesError::<Test>::InsufficientBalance
			);
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(1)));
			assert!(StakePallet::candidate_pool(1)
				.unwrap()
				.can_exit(<Test as Config>::ExitQueueDelay::get()));

			assert_noop!(
				StakePallet::candidate_stake_more(RuntimeOrigin::signed(1), 30),
				Error::<Test>::CannotStakeIfLeaving
			);
			assert_noop!(
				StakePallet::candidate_stake_less(RuntimeOrigin::signed(1), 10),
				Error::<Test>::CannotStakeIfLeaving
			);

			roll_to(30, vec![]);
			assert_ok!(StakePallet::execute_leave_candidates(RuntimeOrigin::signed(1), 1));
			assert_noop!(
				StakePallet::candidate_stake_more(RuntimeOrigin::signed(1), 40),
				Error::<Test>::CandidateNotFound
			);
			assert_ok!(StakePallet::candidate_stake_more(RuntimeOrigin::signed(2), 80));
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(2), 90));
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(3), 10));
			assert_noop!(
				StakePallet::candidate_stake_less(RuntimeOrigin::signed(2), 11),
				Error::<Test>::Underflow
			);
			assert_noop!(
				StakePallet::candidate_stake_less(RuntimeOrigin::signed(2), 1),
				Error::<Test>::ValStakeBelowMin
			);
			assert_noop!(
				StakePallet::candidate_stake_less(RuntimeOrigin::signed(3), 1),
				Error::<Test>::ValStakeBelowMin
			);
			assert_noop!(
				StakePallet::candidate_stake_less(RuntimeOrigin::signed(4), 11),
				Error::<Test>::ValStakeBelowMin
			);
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(4), 10));

			// MaxCollatorCandidateStake
			assert_ok!(StakePallet::join_candidates(
				RuntimeOrigin::signed(11),
				StakePallet::max_candidate_stake()
			));
			assert_noop!(
				StakePallet::candidate_stake_more(RuntimeOrigin::signed(11), 1u128),
				Error::<Test>::ValStakeAboveMax,
			);
		});
}

#[test]
fn delegators_bond() {
	ExtBuilder::default()
		.with_balances(vec![
			(1, 100),
			(2, 100),
			(3, 100),
			(4, 100),
			(5, 100),
			(6, 100),
			(7, 100),
			(8, 100),
			(9, 100),
			(10, 100),
		])
		.with_collators(vec![(1, 20), (2, 20), (3, 20), (4, 20), (5, 10)])
		.with_delegators(vec![(6, 1, 10), (7, 1, 10), (8, 2, 10), (9, 2, 10)])
		.set_blocks_per_round(5)
		.build()
		.execute_with(|| {
			roll_to(4, vec![]);
			assert_noop!(
				StakePallet::join_delegators(RuntimeOrigin::signed(6), 2, 50),
				Error::<Test>::AlreadyDelegating
			);
			assert_noop!(
				StakePallet::delegator_stake_more(RuntimeOrigin::signed(1), 2, 50),
				Error::<Test>::DelegatorNotFound
			);
			assert_noop!(
				StakePallet::delegator_stake_less(RuntimeOrigin::signed(1), 2, 50),
				Error::<Test>::DelegatorNotFound
			);
			assert_noop!(
				StakePallet::delegator_stake_more(RuntimeOrigin::signed(6), 2, 50),
				Error::<Test>::DelegationNotFound
			);
			assert_noop!(
				StakePallet::delegator_stake_more(RuntimeOrigin::signed(7), 6, 50),
				Error::<Test>::CandidateNotFound
			);
			assert_noop!(
				StakePallet::delegator_stake_less(RuntimeOrigin::signed(7), 6, 50),
				Error::<Test>::CandidateNotFound
			);
			assert_noop!(
				StakePallet::delegator_stake_less(RuntimeOrigin::signed(6), 1, 11),
				Error::<Test>::Underflow
			);
			assert_noop!(
				StakePallet::delegator_stake_less(RuntimeOrigin::signed(6), 1, 8),
				Error::<Test>::DelegationBelowMin
			);
			assert_noop!(
				StakePallet::delegator_stake_less(RuntimeOrigin::signed(6), 1, 6),
				Error::<Test>::NomStakeBelowMin
			);
			assert_ok!(StakePallet::delegator_stake_more(RuntimeOrigin::signed(6), 1, 10));
			assert_noop!(
				StakePallet::delegator_stake_less(RuntimeOrigin::signed(6), 2, 5),
				Error::<Test>::DelegationNotFound
			);
			assert_noop!(
				StakePallet::delegator_stake_more(RuntimeOrigin::signed(6), 1, 81),
				BalancesError::<Test>::InsufficientBalance
			);
			assert_noop!(
				StakePallet::join_delegators(RuntimeOrigin::signed(10), 1, 4),
				Error::<Test>::NomStakeBelowMin
			);

			roll_to(9, vec![]);
			assert_eq!(Balances::usable_balance(6), 80);
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(1)));
			assert!(StakePallet::candidate_pool(1)
				.unwrap()
				.can_exit(1 + <Test as Config>::ExitQueueDelay::get()));

			roll_to(31, vec![]);
			assert!(StakePallet::is_delegator(&6));
			assert_ok!(StakePallet::execute_leave_candidates(RuntimeOrigin::signed(1), 1));
			assert!(!StakePallet::is_delegator(&6));
			assert_eq!(Balances::usable_balance(6), 80);
			assert_eq!(Balances::free_balance(6), 100);
		});
}

#[test]
fn revoke_delegation_or_leave_delegators() {
	ExtBuilder::default()
		.with_balances(vec![
			(1, 100),
			(2, 100),
			(3, 100),
			(4, 100),
			(5, 100),
			(6, 100),
			(7, 100),
			(8, 100),
			(9, 100),
			(10, 100),
		])
		.with_collators(vec![(1, 20), (2, 20), (3, 20), (4, 20), (5, 10)])
		.with_delegators(vec![(6, 1, 10), (7, 1, 10), (8, 2, 10), (9, 2, 10), (10, 1, 10)])
		.set_blocks_per_round(5)
		.build()
		.execute_with(|| {
			roll_to(4, vec![]);
			assert_noop!(
				StakePallet::revoke_delegation(RuntimeOrigin::signed(1), 2),
				Error::<Test>::DelegatorNotFound
			);
			assert_noop!(
				StakePallet::revoke_delegation(RuntimeOrigin::signed(6), 2),
				Error::<Test>::DelegationNotFound
			);
			assert_noop!(
				StakePallet::leave_delegators(RuntimeOrigin::signed(1)),
				Error::<Test>::DelegatorNotFound
			);
			assert_ok!(StakePallet::delegate_another_candidate(RuntimeOrigin::signed(6), 2, 3));
			assert_ok!(StakePallet::delegate_another_candidate(RuntimeOrigin::signed(6), 3, 3));
			assert_ok!(StakePallet::revoke_delegation(RuntimeOrigin::signed(6), 1));
			// cannot revoke delegation because would leave remaining total below
			// MinDelegatorStake
			assert_noop!(
				StakePallet::revoke_delegation(RuntimeOrigin::signed(6), 2),
				Error::<Test>::NomStakeBelowMin
			);
			assert_noop!(
				StakePallet::revoke_delegation(RuntimeOrigin::signed(6), 3),
				Error::<Test>::NomStakeBelowMin
			);
			// can revoke both remaining by calling leave delegators
			assert_ok!(StakePallet::leave_delegators(RuntimeOrigin::signed(6)));
			// this leads to 8 leaving set of delegators
			assert_ok!(StakePallet::revoke_delegation(RuntimeOrigin::signed(8), 2));
		});
}

#[test]
fn round_transitions() {
	// round_immediately_jumps_if_current_duration_exceeds_new_blocks_per_round
	// change from 5 bpr to 3 in block 5 -> 8 should be new round
	ExtBuilder::default()
		.with_balances(vec![(1, 100), (2, 100), (3, 100), (4, 100), (5, 100), (6, 100)])
		.with_collators(vec![(1, 20)])
		.with_delegators(vec![(2, 1, 10), (3, 1, 10)])
		.build()
		.execute_with(|| {
			roll_to(5, vec![]);
			let init = vec![Event::NewRound(5, 1)];
			assert_eq!(events(), init);
			assert_ok!(StakePallet::set_blocks_per_round(RuntimeOrigin::root(), 3));
			assert_noop!(
				StakePallet::set_blocks_per_round(RuntimeOrigin::root(), 1),
				Error::<Test>::CannotSetBelowMin
			);
			assert_eq!(last_event(), MetaEvent::StakePallet(Event::BlocksPerRoundSet(1, 5, 5, 3)));

			// last round startet at 5 but we are already at 9, so we expect 9 to be the new
			// round
			roll_to(8, vec![]);
			assert_eq!(last_event(), MetaEvent::StakePallet(Event::NewRound(8, 2)));
		});

	// if duration of current round is less than new bpr, round waits until new bpr
	// passes
	// change from 5 bpr to 3 in block 6 -> 8 should be new round
	ExtBuilder::default()
		.with_balances(vec![(1, 100), (2, 100), (3, 100), (4, 100), (5, 100), (6, 100)])
		.with_collators(vec![(1, 20)])
		.with_delegators(vec![(2, 1, 10), (3, 1, 10)])
		.build()
		.execute_with(|| {
			// Default round every 5 blocks, but MinBlocksPerRound is 3 and we set it to min
			// 3 blocks
			roll_to(6, vec![]);
			// chooses top MaxSelectedCandidates (5), in order
			let init = vec![Event::NewRound(5, 1)];
			assert_eq!(events(), init);
			assert_ok!(StakePallet::set_blocks_per_round(RuntimeOrigin::root(), 3));
			assert_eq!(last_event(), MetaEvent::StakePallet(Event::BlocksPerRoundSet(1, 5, 5, 3)));

			// there should not be a new event
			roll_to(7, vec![]);
			assert_eq!(last_event(), MetaEvent::StakePallet(Event::BlocksPerRoundSet(1, 5, 5, 3)));

			roll_to(8, vec![]);
			assert_eq!(last_event(), MetaEvent::StakePallet(Event::NewRound(8, 2)));
		});

	// round_immediately_jumps_if_current_duration_exceeds_new_blocks_per_round
	// change from 5 bpr (blocks_per_round) to 3 in block 7 -> 8 should be new round
	ExtBuilder::default()
		.with_balances(vec![(1, 100), (2, 100), (3, 100), (4, 100), (5, 100), (6, 100)])
		.with_collators(vec![(1, 20)])
		.with_delegators(vec![(2, 1, 10), (3, 1, 10)])
		.build()
		.execute_with(|| {
			// Default round every 5 blocks, but MinBlocksPerRound is 3 and we set it to min
			// 3 blocks
			roll_to(7, vec![]);
			// chooses top MaxSelectedCandidates (5), in order
			let init = vec![Event::NewRound(5, 1)];
			assert_eq!(events(), init);
			assert_ok!(StakePallet::set_blocks_per_round(RuntimeOrigin::root(), 3));

			assert_eq!(last_event(), MetaEvent::StakePallet(Event::BlocksPerRoundSet(1, 5, 5, 3)));
			roll_to(8, vec![]);

			// last round startet at 5, so we expect 8 to be the new round
			assert_eq!(last_event(), MetaEvent::StakePallet(Event::NewRound(8, 2)));
		});
}

#[test]
fn delegator_should_not_receive_rewards_after_revoking() {
	// test edge case of 1 delegator
	ExtBuilder::default()
		.with_balances(vec![(1, 10_000_000 * DECIMALS), (2, 10_000_000 * DECIMALS)])
		.with_collators(vec![(1, 10_000_000 * DECIMALS)])
		.with_delegators(vec![(2, 1, 10_000_000 * DECIMALS)])
		.build()
		.execute_with(|| {
			assert_ok!(StakePallet::revoke_delegation(RuntimeOrigin::signed(2), 1));
			let authors: Vec<Option<AccountId>> = (1u64..100u64).map(|_| Some(1u64)).collect();
			assert_eq!(Balances::usable_balance(1), Balance::zero());
			assert_eq!(Balances::usable_balance(2), Balance::zero());
			roll_to(100, authors);
			assert!(Balances::usable_balance(1) > Balance::zero());
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(2), 2));
			assert_eq!(Balances::usable_balance(2), 10_000_000 * DECIMALS);
		});

	ExtBuilder::default()
		.with_balances(vec![
			(1, 10_000_000 * DECIMALS),
			(2, 10_000_000 * DECIMALS),
			(3, 10_000_000 * DECIMALS),
		])
		.with_collators(vec![(1, 10_000_000 * DECIMALS)])
		.with_delegators(vec![(2, 1, 10_000_000 * DECIMALS), (3, 1, 10_000_000 * DECIMALS)])
		.build()
		.execute_with(|| {
			assert_ok!(StakePallet::revoke_delegation(RuntimeOrigin::signed(3), 1));
			let authors: Vec<Option<AccountId>> = (1u64..100u64).map(|_| Some(1u64)).collect();
			assert_eq!(Balances::usable_balance(1), Balance::zero());
			assert_eq!(Balances::usable_balance(2), Balance::zero());
			assert_eq!(Balances::usable_balance(3), Balance::zero());
			roll_to(100, authors);
			assert!(Balances::usable_balance(1) > Balance::zero());
			assert!(Balances::usable_balance(2) > Balance::zero());
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(3), 3));
			assert_eq!(Balances::usable_balance(3), 10_000_000 * DECIMALS);
		});
}

#[test]
fn coinbase_rewards_many_blocks_simple_check() {
	ExtBuilder::default()
		.with_balances(vec![
			(1, 40_000_000 * DECIMALS),
			(2, 40_000_000 * DECIMALS),
			(3, 40_000_000 * DECIMALS),
			(4, 20_000_000 * DECIMALS),
			(5, 20_000_000 * DECIMALS),
		])
		.with_collators(vec![(1, 32_000_000 * DECIMALS), (2, 8_000_000 * DECIMALS)])
		.with_delegators(vec![
			(3, 1, 8_000_000 * DECIMALS),
			(4, 1, 16_000_000 * DECIMALS),
			(5, 2, 16_000_000 * DECIMALS),
		])
		.build()
		.execute_with(|| {
			let total_issuance = <Test as Config>::Currency::total_issuance();
			assert_eq!(total_issuance, 160_000_000 * DECIMALS);

			let end_block: BlockNumber = 26295;
			// set round robin authoring
			let authors: Vec<Option<AccountId>> =
				(0u64..=end_block).map(|i| Some(i % 2 + 1)).collect();
			// adding one is to force the session go next
			roll_to(5, authors.clone());

			let genesis_reward_1 = Perbill::from_float(32. / 80.) * BLOCK_REWARD_IN_GENESIS_SESSION;
			let genesis_reward_3 = Perbill::from_float(8. / 80.) * BLOCK_REWARD_IN_GENESIS_SESSION;
			let genesis_reward_4 = Perbill::from_float(16. / 80.) * BLOCK_REWARD_IN_GENESIS_SESSION;

			let genesis_reward_2 = Perbill::from_float(8. / 80.) * BLOCK_REWARD_IN_GENESIS_SESSION;
			let genesis_reward_5 = Perbill::from_float(16. / 80.) * BLOCK_REWARD_IN_GENESIS_SESSION;

			assert_eq!(Balances::free_balance(1), genesis_reward_1 + 40_000_000 * DECIMALS);
			assert_eq!(Balances::free_balance(2), genesis_reward_2 + 40_000_000 * DECIMALS);
			assert_eq!(Balances::free_balance(3), genesis_reward_3 + 40_000_000 * DECIMALS);
			assert_eq!(Balances::free_balance(4), genesis_reward_4 + 20_000_000 * DECIMALS);
			assert_eq!(Balances::free_balance(5), genesis_reward_5 + 20_000_000 * DECIMALS);

			// 2 is block author for 3 blocks, 1 is block author for 2 block
			roll_to(10, authors.clone());
			let normal_odd_total_stake: u64 = 2 * (32 + 8 + 16) + 3 * (8 + 16);

			let normal_odd_reward_1 = Perbill::from_rational(2 * 32, normal_odd_total_stake) *
				BLOCK_REWARD_IN_NORMAL_SESSION;
			let normal_odd_reward_3 = Perbill::from_rational(2 * 8, normal_odd_total_stake) *
				BLOCK_REWARD_IN_NORMAL_SESSION;
			let normal_odd_reward_4 = Perbill::from_rational(2 * 16, normal_odd_total_stake) *
				BLOCK_REWARD_IN_NORMAL_SESSION;
			let normal_odd_reward_2 = Perbill::from_rational(3 * 8, normal_odd_total_stake) *
				BLOCK_REWARD_IN_NORMAL_SESSION;
			let normal_odd_reward_5 = Perbill::from_rational(3 * 16, normal_odd_total_stake) *
				BLOCK_REWARD_IN_NORMAL_SESSION;

			assert_eq!(
				Balances::free_balance(1),
				genesis_reward_1 + normal_odd_reward_1 + 40_000_000 * DECIMALS
			);
			assert_eq!(
				Balances::free_balance(2),
				genesis_reward_2 + normal_odd_reward_2 + 40_000_000 * DECIMALS
			);
			assert_eq!(
				Balances::free_balance(3),
				genesis_reward_3 + normal_odd_reward_3 + 40_000_000 * DECIMALS
			);
			assert_eq!(
				Balances::free_balance(4),
				genesis_reward_4 + normal_odd_reward_4 + 20_000_000 * DECIMALS
			);
			assert_eq!(
				Balances::free_balance(5),
				genesis_reward_5 + normal_odd_reward_5 + 20_000_000 * DECIMALS
			);

			// 2 is block author for 3 blocks, 1 is block author for 2 block
			roll_to(15, authors.clone());
			let normal_even_total_stake: u64 = 3 * (32 + 8 + 16) + 2 * (8 + 16);

			let normal_even_reward_1 = Perbill::from_rational(3 * 32, normal_even_total_stake) *
				BLOCK_REWARD_IN_NORMAL_SESSION;
			let normal_even_reward_3 = Perbill::from_rational(3 * 8, normal_even_total_stake) *
				BLOCK_REWARD_IN_NORMAL_SESSION;
			let normal_even_reward_4 = Perbill::from_rational(3 * 16, normal_even_total_stake) *
				BLOCK_REWARD_IN_NORMAL_SESSION;
			let normal_even_reward_2 = Perbill::from_rational(2 * 8, normal_even_total_stake) *
				BLOCK_REWARD_IN_NORMAL_SESSION;
			let normal_even_reward_5 = Perbill::from_rational(2 * 16, normal_even_total_stake) *
				BLOCK_REWARD_IN_NORMAL_SESSION;

			assert_eq!(
				Balances::free_balance(1),
				genesis_reward_1 +
					normal_odd_reward_1 + normal_even_reward_1 +
					40_000_000 * DECIMALS
			);
			assert_eq!(
				Balances::free_balance(2),
				genesis_reward_2 +
					normal_odd_reward_2 + normal_even_reward_2 +
					40_000_000 * DECIMALS
			);
			assert_eq!(
				Balances::free_balance(3),
				genesis_reward_3 +
					normal_odd_reward_3 + normal_even_reward_3 +
					40_000_000 * DECIMALS
			);
			assert_eq!(
				Balances::free_balance(4),
				genesis_reward_4 +
					normal_odd_reward_4 + normal_even_reward_4 +
					20_000_000 * DECIMALS
			);
			assert_eq!(
				Balances::free_balance(5),
				genesis_reward_5 +
					normal_odd_reward_5 + normal_even_reward_5 +
					20_000_000 * DECIMALS
			);

			roll_to(end_block, authors.clone());
			let multiply_factor = (end_block as u128 - 5) / 10;
			assert_eq!(
				Balances::free_balance(1),
				genesis_reward_1 +
					(normal_odd_reward_1 + normal_even_reward_1) * multiply_factor +
					40_000_000 * DECIMALS
			);
			assert_eq!(
				Balances::free_balance(2),
				genesis_reward_2 +
					(normal_odd_reward_2 + normal_even_reward_2) * multiply_factor +
					40_000_000 * DECIMALS
			);
			assert_eq!(
				Balances::free_balance(3),
				genesis_reward_3 +
					(normal_odd_reward_3 + normal_even_reward_3) * multiply_factor +
					40_000_000 * DECIMALS
			);
			assert_eq!(
				Balances::free_balance(4),
				genesis_reward_4 +
					(normal_odd_reward_4 + normal_even_reward_4) * multiply_factor +
					20_000_000 * DECIMALS
			);
			assert_eq!(
				Balances::free_balance(5),
				genesis_reward_5 +
					(normal_odd_reward_5 + normal_even_reward_5) * multiply_factor +
					20_000_000 * DECIMALS
			);

			// Check total issue number
			assert!(almost_equal(
				total_issuance +
					(normal_odd_reward_1 + normal_even_reward_1) * multiply_factor +
					(normal_odd_reward_2 + normal_even_reward_2) * multiply_factor +
					(normal_odd_reward_3 + normal_even_reward_3) * multiply_factor +
					(normal_odd_reward_4 + normal_even_reward_4) * multiply_factor +
					(normal_odd_reward_5 + normal_even_reward_5) * multiply_factor,
				<Test as Config>::Currency::total_issuance(),
				Perbill::from_perthousand(1)
			));
		});
}

// Could only occur if we increase MinDelegatorStakeOf::<Test> via runtime
// upgrade and don't migrate delegators which fall below minimum
#[test]
fn should_reward_delegators_below_min_stake() {
	let stake_num = 10 * DECIMALS;
	ExtBuilder::default()
		.with_balances(vec![(1, stake_num), (2, stake_num), (3, stake_num), (4, stake_num)])
		.with_collators(vec![(1, stake_num), (2, stake_num)])
		.with_delegators(vec![(3, 2, stake_num)])
		.build()
		.execute_with(|| {
			// impossible but lets assume it happened
			let mut state =
				StakePallet::candidate_pool(1).expect("CollatorState cannot be missing");
			let delegator_stake_below_min = <Test as Config>::MinDelegatorStake::get() - 1;
			state.stake += delegator_stake_below_min;
			state.total += delegator_stake_below_min;
			let impossible_bond =
				StakeOf::<Test> { owner: 4u64, amount: delegator_stake_below_min };
			assert_eq!(state.delegators.try_insert(impossible_bond), Ok(true));
			<crate::CandidatePool<Test>>::insert(1u64, state);

			let authors: Vec<Option<AccountId>> =
				vec![None, Some(1u64), Some(1u64), Some(1u64), Some(1u64)];
			assert_eq!(Balances::usable_balance(1), Balance::zero());
			assert_eq!(Balances::usable_balance(2), Balance::zero());
			assert_eq!(Balances::usable_balance(3), Balance::zero());
			assert_eq!(Balances::usable_balance(4), stake_num);

			// should only reward 1
			let total_stake_num = stake_num + delegator_stake_below_min;
			roll_to(5, authors);
			assert_eq!(
				Balances::usable_balance(1),
				Perquintill::from_rational(stake_num, total_stake_num) *
					BLOCK_REWARD_IN_GENESIS_SESSION
			);
			assert_eq!(
				Balances::usable_balance(4) - stake_num,
				Perquintill::from_rational(delegator_stake_below_min, total_stake_num) *
					BLOCK_REWARD_IN_GENESIS_SESSION
			);

			assert_eq!(Balances::usable_balance(2), Balance::zero());
			assert_eq!(Balances::usable_balance(3), Balance::zero());
		});
}

#[test]
#[should_panic]
fn should_deny_low_delegator_stake() {
	ExtBuilder::default()
		.with_balances(vec![(1, 10 * DECIMALS), (2, 10 * DECIMALS), (3, 10 * DECIMALS), (4, 1)])
		.with_collators(vec![(1, 10 * DECIMALS), (2, 10 * DECIMALS)])
		.with_delegators(vec![(4, 2, 1)])
		.build()
		.execute_with(|| {});
}

#[test]
#[should_panic]
fn should_deny_low_collator_stake() {
	ExtBuilder::default()
		.with_balances(vec![(1, 10 * DECIMALS), (2, 5)])
		.with_collators(vec![(1, 10 * DECIMALS), (2, 5)])
		.build()
		.execute_with(|| {});
}

#[test]
#[should_panic]
fn should_deny_duplicate_collators() {
	ExtBuilder::default()
		.with_balances(vec![(1, 10 * DECIMALS)])
		.with_collators(vec![(1, 10 * DECIMALS), (1, 10 * DECIMALS)])
		.build()
		.execute_with(|| {});
}

#[test]
fn reach_max_top_candidates() {
	ExtBuilder::default()
		.with_balances(vec![
			(1, 11),
			(2, 20),
			(3, 11),
			(4, 11),
			(5, 11),
			(6, 11),
			(7, 11),
			(8, 11),
			(9, 11),
			(10, 11),
			(11, 11),
			(12, 12),
			(13, 13),
		])
		.with_collators(vec![
			(1, 10),
			(2, 20),
			(3, 10),
			(4, 10),
			(5, 10),
			(6, 10),
			(7, 10),
			(8, 10),
			(9, 10),
			(10, 10),
		])
		.build()
		.execute_with(|| {
			assert_eq!(
				StakePallet::top_candidates().len().saturated_into::<u32>(),
				<Test as Config>::MaxTopCandidates::get()
			);
			// should not be possible to join candidate pool, even with more stake
			assert_ok!(StakePallet::join_candidates(RuntimeOrigin::signed(11), 11));
			assert_eq!(
				StakePallet::top_candidates().into_iter().map(|s| s.owner).collect::<Vec<u64>>(),
				vec![2, 11, 1, 3, 4, 5, 6, 7, 8, 9]
			);
			// last come, last one in the list
			assert_ok!(StakePallet::join_candidates(RuntimeOrigin::signed(12), 11));
			assert_eq!(
				StakePallet::top_candidates().into_iter().map(|s| s.owner).collect::<Vec<u64>>(),
				vec![2, 11, 12, 1, 3, 4, 5, 6, 7, 8]
			);
			assert_ok!(StakePallet::candidate_stake_more(RuntimeOrigin::signed(1), 1));
			assert_ok!(StakePallet::candidate_stake_more(RuntimeOrigin::signed(3), 1));
			assert_ok!(StakePallet::candidate_stake_more(RuntimeOrigin::signed(4), 1));
			assert_ok!(StakePallet::candidate_stake_more(RuntimeOrigin::signed(5), 1));
			assert_ok!(StakePallet::candidate_stake_more(RuntimeOrigin::signed(6), 1));
			assert_ok!(StakePallet::candidate_stake_more(RuntimeOrigin::signed(7), 1));
			assert_ok!(StakePallet::candidate_stake_more(RuntimeOrigin::signed(8), 1));
			assert_eq!(
				StakePallet::top_candidates().into_iter().map(|s| s.owner).collect::<Vec<u64>>(),
				vec![2, 11, 12, 1, 3, 4, 5, 6, 7, 8]
			);
		});
}

#[test]
fn should_estimate_current_session_progress() {
	ExtBuilder::default()
		.set_blocks_per_round(100)
		.with_balances(vec![
			(1, 10),
			(2, 20),
			(3, 10),
			(4, 10),
			(5, 10),
			(6, 10),
			(7, 10),
			(8, 10),
			(9, 10),
			(10, 10),
			(11, 10),
		])
		.with_collators(vec![
			(1, 10),
			(2, 20),
			(3, 10),
			(4, 10),
			(5, 10),
			(6, 10),
			(7, 10),
			(8, 10),
			(9, 10),
			(10, 10),
		])
		.build()
		.execute_with(|| {
			assert_eq!(
				StakePallet::estimate_current_session_progress(10).0.unwrap(),
				Permill::from_percent(10)
			);
			assert_eq!(
				StakePallet::estimate_current_session_progress(20).0.unwrap(),
				Permill::from_percent(20)
			);
			assert_eq!(
				StakePallet::estimate_current_session_progress(30).0.unwrap(),
				Permill::from_percent(30)
			);
			assert_eq!(
				StakePallet::estimate_current_session_progress(60).0.unwrap(),
				Permill::from_percent(60)
			);
			assert_eq!(
				StakePallet::estimate_current_session_progress(100).0.unwrap(),
				Permill::from_percent(100)
			);
		});
}

#[test]
fn should_estimate_next_session_rotation() {
	ExtBuilder::default()
		.set_blocks_per_round(100)
		.with_balances(vec![
			(1, 10),
			(2, 20),
			(3, 10),
			(4, 10),
			(5, 10),
			(6, 10),
			(7, 10),
			(8, 10),
			(9, 10),
			(10, 10),
			(11, 10),
		])
		.with_collators(vec![
			(1, 10),
			(2, 20),
			(3, 10),
			(4, 10),
			(5, 10),
			(6, 10),
			(7, 10),
			(8, 10),
			(9, 10),
			(10, 10),
		])
		.build()
		.execute_with(|| {
			assert_eq!(StakePallet::estimate_next_session_rotation(10).0.unwrap(), 100);
			assert_eq!(StakePallet::estimate_next_session_rotation(20).0.unwrap(), 100);
			assert_eq!(StakePallet::estimate_next_session_rotation(30).0.unwrap(), 100);
			assert_eq!(StakePallet::estimate_next_session_rotation(60).0.unwrap(), 100);
			assert_eq!(StakePallet::estimate_next_session_rotation(100).0.unwrap(), 100);
		});
}

#[test]
fn should_end_session_when_appropriate() {
	ExtBuilder::default()
		.set_blocks_per_round(100)
		.with_balances(vec![
			(1, 10),
			(2, 20),
			(3, 10),
			(4, 10),
			(5, 10),
			(6, 10),
			(7, 10),
			(8, 10),
			(9, 10),
			(10, 10),
			(11, 10),
		])
		.with_collators(vec![
			(1, 10),
			(2, 20),
			(3, 10),
			(4, 10),
			(5, 10),
			(6, 10),
			(7, 10),
			(8, 10),
			(9, 10),
			(10, 10),
		])
		.build()
		.execute_with(|| {
			assert!(!StakePallet::should_end_session(10));
			assert!(!StakePallet::should_end_session(20));
			assert!(!StakePallet::should_end_session(30));
			assert!(!StakePallet::should_end_session(60));
			assert!(StakePallet::should_end_session(100));
		});
}

#[test]
fn set_max_selected_candidates_safe_guards() {
	ExtBuilder::default()
		.with_balances(vec![(1, 10)])
		.with_collators(vec![(1, 10)])
		.build()
		.execute_with(|| {
			assert_noop!(
				StakePallet::set_max_selected_candidates(
					RuntimeOrigin::root(),
					<Test as Config>::MinCollators::get() - 1
				),
				Error::<Test>::CannotSetBelowMin
			);
			assert_noop!(
				StakePallet::set_max_selected_candidates(
					RuntimeOrigin::root(),
					<Test as Config>::MaxTopCandidates::get() + 1
				),
				Error::<Test>::CannotSetAboveMax
			);
			assert_ok!(StakePallet::set_max_selected_candidates(
				RuntimeOrigin::root(),
				<Test as Config>::MinCollators::get() + 1
			));
		});
}

#[test]
fn set_max_selected_candidates_total_stake() {
	let balances: Vec<(AccountId, Balance)> = (1..19).map(|x| (x, 100)).collect();
	ExtBuilder::default()
		.with_balances(balances)
		.with_collators(vec![
			(1, 11),
			(2, 12),
			(3, 13),
			(4, 14),
			(5, 15),
			(6, 16),
			(7, 17),
			(8, 18),
		])
		.with_delegators(vec![
			(11, 1, 21),
			(12, 2, 22),
			(13, 3, 23),
			(14, 4, 24),
			(15, 5, 25),
			(16, 6, 26),
			(17, 7, 27),
			(18, 8, 28),
		])
		.build()
		.execute_with(|| {
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 35, delegators: 55 }
			);

			assert_ok!(StakePallet::set_max_selected_candidates(RuntimeOrigin::root(), 3));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 51, delegators: 81 }
			);

			assert_ok!(StakePallet::set_max_selected_candidates(RuntimeOrigin::root(), 5));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 80, delegators: 130 }
			);

			assert_ok!(StakePallet::set_max_selected_candidates(RuntimeOrigin::root(), 10));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 116, delegators: 196 }
			);

			assert_ok!(StakePallet::set_max_selected_candidates(RuntimeOrigin::root(), 2));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 35, delegators: 55 }
			);
		});
}

#[test]
fn unlock_unstaked() {
	// same_unstaked_as_restaked
	// block 1: stake & unstake for 100
	// block 2: stake & unstake for 100
	// should remove first entry in unstaking BoundedBTreeMap when staking in block
	// 2 should still have 100 locked until unlocking
	ExtBuilder::default()
		.with_balances(vec![(1, 10), (2, 100)])
		.with_collators(vec![(1, 10)])
		.with_delegators(vec![(2, 1, 100)])
		.build()
		.execute_with(|| {
			assert_ok!(StakePallet::revoke_delegation(RuntimeOrigin::signed(2), 1));
			let mut unstaking: BoundedBTreeMap<
				BlockNumber,
				BalanceOf<Test>,
				<Test as Config>::MaxUnstakeRequests,
			> = BoundedBTreeMap::new();
			assert_ok!(unstaking.try_insert(3, 100));
			let lock = BalanceLock { id: STAKING_ID, amount: 100, reasons: Reasons::All };
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);
			// shouldn't be able to unlock anything
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(2), 2));
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);

			// join delegators and revoke again --> consume unstaking at block 3
			roll_to(2, vec![]);
			assert_ok!(StakePallet::join_delegators(RuntimeOrigin::signed(2), 1, 100));
			assert_ok!(StakePallet::revoke_delegation(RuntimeOrigin::signed(2), 1));
			unstaking.remove(&3);
			assert_ok!(unstaking.try_insert(4, 100));
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);
			// shouldn't be able to unlock anything
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(2), 2));
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);

			// should reduce unlocking but not unlock anything
			roll_to(3, vec![]);
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);
			// shouldn't be able to unlock anything
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(2), 2));
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);

			roll_to(4, vec![]);
			unstaking.remove(&4);
			assert_eq!(Balances::locks(2), vec![lock]);
			// shouldn't be able to unlock anything
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(2), 2));
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![]);
		});

	// less_unstaked_than_restaked
	// block 1: stake & unstake for 10
	// block 2: stake & unstake for 100
	// should remove first entry in unstaking BoundedBTreeMap when staking in block
	// 2 should still have 90 locked until unlocking in block 4
	ExtBuilder::default()
		.with_balances(vec![(1, 10), (2, 100)])
		.with_collators(vec![(1, 10)])
		.with_delegators(vec![(2, 1, 10)])
		.build()
		.execute_with(|| {
			assert_ok!(StakePallet::revoke_delegation(RuntimeOrigin::signed(2), 1));
			let mut unstaking: BoundedBTreeMap<
				BlockNumber,
				BalanceOf<Test>,
				<Test as Config>::MaxUnstakeRequests,
			> = BoundedBTreeMap::new();
			assert_ok!(unstaking.try_insert(3, 10));
			let mut lock = BalanceLock { id: STAKING_ID, amount: 10, reasons: Reasons::All };
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);
			// shouldn't be able to unlock anything
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(2), 2));
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);

			// join delegators and revoke again
			roll_to(2, vec![]);
			assert_ok!(StakePallet::join_delegators(RuntimeOrigin::signed(2), 1, 100));
			assert_ok!(StakePallet::revoke_delegation(RuntimeOrigin::signed(2), 1));
			unstaking.remove(&3);
			assert_ok!(unstaking.try_insert(4, 100));
			lock.amount = 100;
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);
			// shouldn't be able to unlock anything
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(2), 2));
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);

			roll_to(3, vec![]);
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);
			// shouldn't be able to unlock anything
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(2), 2));
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);

			// unlock unstaked, remove lock, empty unlocking
			roll_to(4, vec![]);
			unstaking.remove(&4);
			assert_eq!(Balances::locks(2), vec![lock]);
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(2), 2));
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![]);
		});

	// more_unstaked_than_restaked
	// block 1: stake & unstake for 100
	// block 2: stake & unstake for 10
	// should reduce first entry from amount 100 to 90 in unstaking BoundedBTreeMap
	// when staking in block 2
	// should have 100 locked until unlocking in block 3, then 10
	// should have 10 locked until further unlocking in block 4
	ExtBuilder::default()
		.with_balances(vec![(1, 10), (2, 100)])
		.with_collators(vec![(1, 10)])
		.with_delegators(vec![(2, 1, 100)])
		.build()
		.execute_with(|| {
			assert_ok!(StakePallet::revoke_delegation(RuntimeOrigin::signed(2), 1));
			let mut unstaking: BoundedBTreeMap<
				BlockNumber,
				BalanceOf<Test>,
				<Test as Config>::MaxUnstakeRequests,
			> = BoundedBTreeMap::new();
			assert_ok!(unstaking.try_insert(3, 100));
			let mut lock = BalanceLock { id: STAKING_ID, amount: 100, reasons: Reasons::All };
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);
			// shouldn't be able to unlock anything
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(2), 2));
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);

			// join delegators and revoke again
			roll_to(2, vec![]);
			assert_ok!(StakePallet::join_delegators(RuntimeOrigin::signed(2), 1, 10));
			assert_ok!(StakePallet::revoke_delegation(RuntimeOrigin::signed(2), 1));
			assert_ok!(unstaking.try_insert(3, 90));
			assert_ok!(unstaking.try_insert(4, 10));
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);
			// shouldn't be able to unlock anything
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(2), 2));
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);

			// should reduce unlocking but not unlock anything
			roll_to(3, vec![]);
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);
			// should be able to unlock 90 of 100 from unstaking
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(2), 2));
			unstaking.remove(&3);
			lock.amount = 10;
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);

			roll_to(4, vec![]);
			assert_eq!(Balances::locks(2), vec![lock]);
			// should be able to unlock 10 of remaining 10
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(2), 2));
			unstaking.remove(&4);
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(2), vec![]);
		});

	// test_stake_less
	// block 1: stake & unstake for 100
	// block 2: stake & unstake for 10
	// should reduce first entry from amount 100 to 90 in unstaking BoundedBTreeMap
	// when staking in block 2
	// should have 100 locked until unlocking in block 3, then 10
	// should have 10 locked until further unlocking in block 4
	ExtBuilder::default()
		.with_balances(vec![(1, 200), (2, 200)])
		.with_collators(vec![(1, 200)])
		.with_delegators(vec![(2, 1, 200)])
		.build()
		.execute_with(|| {
			// should be able to decrease more often than MaxUnstakeRequests because it's
			// the same block and thus unstaking is increased at block 3 instead of having
			// multiple entries for the same block
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(1), 10));
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(1), 10));
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(1), 10));
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(1), 10));
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(1), 10));
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(1), 10),);
			assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(2), 1, 10));
			assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(2), 1, 10));
			assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(2), 1, 10));
			assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(2), 1, 10));
			assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(2), 1, 10));
			assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(2), 1, 10),);
			let mut unstaking: BoundedBTreeMap<
				BlockNumber,
				BalanceOf<Test>,
				<Test as Config>::MaxUnstakeRequests,
			> = BoundedBTreeMap::new();
			assert_ok!(unstaking.try_insert(3, 60));
			let mut lock = BalanceLock { id: STAKING_ID, amount: 200, reasons: Reasons::All };
			assert_eq!(Balances::locks(1), vec![lock.clone()]);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);
			assert_eq!(StakePallet::unstaking(1), unstaking);
			assert_eq!(StakePallet::unstaking(2), unstaking);
			// shouldn't be able to unlock anything
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(1), 1));
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(2), 2));
			assert_eq!(StakePallet::unstaking(1), unstaking);
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(1), vec![lock.clone()]);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);

			roll_to(2, vec![]);
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(1), 10),);
			assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(2), 1, 10),);
			assert_ok!(unstaking.try_insert(4, 10));
			assert_eq!(Balances::locks(1), vec![lock.clone()]);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);
			assert_eq!(StakePallet::unstaking(1), unstaking);
			assert_eq!(StakePallet::unstaking(2), unstaking);
			// shouldn't be able to unlock anything
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(1), 1));
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(2), 2));
			assert_eq!(StakePallet::unstaking(1), unstaking);
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(1), vec![lock.clone()]);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);

			roll_to(3, vec![]);
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(1), 10),);
			assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(2), 1, 10),);
			assert_ok!(unstaking.try_insert(5, 10));
			assert_ok!(unstaking.try_insert(5, 10));
			assert_eq!(Balances::locks(1), vec![lock.clone()]);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);
			assert_eq!(StakePallet::unstaking(1), unstaking);
			assert_eq!(StakePallet::unstaking(2), unstaking);
			// should unlock 60
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(1), 1));
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(2), 2));
			lock.amount = 140;
			unstaking.remove(&3);
			assert_eq!(StakePallet::unstaking(1), unstaking);
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(1), vec![lock.clone()]);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);

			// reach MaxUnstakeRequests
			roll_to(4, vec![]);
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(1), 10));
			assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(2), 1, 10));
			roll_to(5, vec![]);
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(1), 10));
			assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(2), 1, 10));
			roll_to(6, vec![]);
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(1), 10));
			assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(2), 1, 10));
			assert_ok!(unstaking.try_insert(6, 10));
			assert_ok!(unstaking.try_insert(7, 10));
			assert_ok!(unstaking.try_insert(8, 10));
			assert_eq!(StakePallet::unstaking(1), unstaking);
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(1), vec![lock.clone()]);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);

			roll_to(7, vec![]);
			assert_noop!(
				StakePallet::candidate_stake_less(RuntimeOrigin::signed(1), 10),
				Error::<Test>::NoMoreUnstaking
			);
			assert_noop!(
				StakePallet::delegator_stake_less(RuntimeOrigin::signed(2), 1, 10),
				Error::<Test>::NoMoreUnstaking
			);
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(1), 1));
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(2), 2));
			unstaking.remove(&4);
			unstaking.remove(&5);
			unstaking.remove(&6);
			unstaking.remove(&7);
			lock.amount = 100;
			assert_eq!(StakePallet::unstaking(1), unstaking);
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(1), vec![lock.clone()]);
			assert_eq!(Balances::locks(2), vec![lock.clone()]);
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(1), 40));
			assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(2), 1, 40));
			assert_ok!(unstaking.try_insert(9, 40));
			assert_ok!(StakePallet::candidate_stake_more(RuntimeOrigin::signed(1), 30));
			assert_ok!(StakePallet::delegator_stake_more(RuntimeOrigin::signed(2), 1, 30));
			unstaking.remove(&8);
			assert_ok!(unstaking.try_insert(9, 20));
			assert_eq!(StakePallet::unstaking(1), unstaking);
			assert_eq!(StakePallet::unstaking(2), unstaking);
			assert_eq!(Balances::locks(1), vec![lock.clone()]);
			assert_eq!(Balances::locks(2), vec![lock]);
		});
}

#[test]
fn kick_candidate_with_full_unstaking() {
	ExtBuilder::default()
		.with_balances(vec![(1, 200), (2, 200), (3, 300)])
		.with_collators(vec![(1, 200), (2, 200), (3, 200)])
		.build()
		.execute_with(|| {
			let max_unstake_reqs: usize =
				<Test as Config>::MaxUnstakeRequests::get().saturating_sub(1).saturated_into();
			// Fill unstake requests
			for block in 1u64..1u64.saturating_add(max_unstake_reqs as u64) {
				System::set_block_number(block);
				assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(3), 1));
			}
			assert_eq!(StakePallet::unstaking(3).into_inner().len(), max_unstake_reqs);

			// Additional unstake should fail
			System::set_block_number(100);
			assert_noop!(
				StakePallet::candidate_stake_less(RuntimeOrigin::signed(3), 1),
				Error::<Test>::NoMoreUnstaking
			);

			// Fill last unstake request by removing candidate and unstaking all stake
			assert_ok!(StakePallet::force_remove_candidate(RuntimeOrigin::root(), 3));

			// Cannot join with full unstaking
			assert_eq!(StakePallet::unstaking(3).into_inner().len(), max_unstake_reqs + 1);
			assert_noop!(
				StakePallet::join_candidates(RuntimeOrigin::signed(3), 100),
				Error::<Test>::CannotJoinBeforeUnlocking
			);
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(3), 3));
			assert_ok!(StakePallet::join_candidates(RuntimeOrigin::signed(3), 100));
		});
}
#[test]
fn kick_delegator_with_full_unstaking() {
	ExtBuilder::default()
		.with_balances(vec![(1, 200), (2, 200), (3, 200), (4, 200), (5, 420), (6, 200)])
		.with_collators(vec![(1, 200)])
		.with_delegators(vec![(2, 1, 200), (3, 1, 200), (4, 1, 200), (5, 1, 200)])
		.build()
		.execute_with(|| {
			let max_unstake_reqs: usize =
				<Test as Config>::MaxUnstakeRequests::get().saturating_sub(1).saturated_into();
			// Fill unstake requests
			for block in 1u64..1u64.saturating_add(max_unstake_reqs as u64) {
				System::set_block_number(block);
				assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(5), 1, 1));
			}
			assert_eq!(StakePallet::unstaking(5).into_inner().len(), max_unstake_reqs);

			// Additional unstake should fail
			System::set_block_number(100);
			assert_noop!(
				StakePallet::delegator_stake_less(RuntimeOrigin::signed(5), 1, 1),
				Error::<Test>::NoMoreUnstaking
			);

			// Fill last unstake request by replacing delegator
			assert_ok!(StakePallet::join_delegators(RuntimeOrigin::signed(6), 1, 200));
			assert_eq!(StakePallet::unstaking(5).into_inner().len(), max_unstake_reqs + 1);
			assert!(!StakePallet::is_delegator(&5));

			// Cannot join with full unstaking
			assert_noop!(
				StakePallet::join_delegators(RuntimeOrigin::signed(5), 1, 100),
				Error::<Test>::CannotJoinBeforeUnlocking
			);
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(5), 5));
			assert_ok!(StakePallet::join_delegators(RuntimeOrigin::signed(5), 1, 220));
		});
}

#[test]
fn candidate_leaves() {
	let balances: Vec<(AccountId, Balance)> = (1u64..=15u64).map(|id| (id, 100)).collect();
	ExtBuilder::default()
		.with_balances(balances)
		.with_collators(vec![(1, 100), (2, 100)])
		.with_delegators(vec![(12, 1, 100), (13, 1, 10)])
		.build()
		.execute_with(|| {
			assert_eq!(
				StakePallet::top_candidates().into_iter().map(|s| s.owner).collect::<Vec<u64>>(),
				vec![1, 2]
			);
			assert_noop!(
				StakePallet::init_leave_candidates(RuntimeOrigin::signed(11)),
				Error::<Test>::CandidateNotFound
			);
			assert_noop!(
				StakePallet::init_leave_candidates(RuntimeOrigin::signed(1)),
				Error::<Test>::TooFewCollatorCandidates
			);
			// add five more collator to max fill TopCandidates
			for candidate in 3u64..11u64 {
				assert_ok!(StakePallet::join_candidates(RuntimeOrigin::signed(candidate), 100));
			}
			assert_eq!(
				StakePallet::top_candidates().into_iter().map(|s| s.owner).collect::<Vec<u64>>(),
				(1u64..11u64).collect::<Vec<u64>>()
			);
			assert_eq!(CandidatePool::<Test>::count(), 10);
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(1)));
			assert_eq!(
				StakePallet::top_candidates().into_iter().map(|s| s.owner).collect::<Vec<u64>>(),
				(2u64..11u64).collect::<Vec<u64>>()
			);
			assert_noop!(
				StakePallet::join_delegators(RuntimeOrigin::signed(15), 1, 10),
				Error::<Test>::CannotDelegateIfLeaving
			);
			assert_noop!(
				StakePallet::delegator_stake_more(RuntimeOrigin::signed(12), 1, 1),
				Error::<Test>::CannotDelegateIfLeaving
			);
			assert_noop!(
				StakePallet::delegator_stake_less(RuntimeOrigin::signed(12), 1, 1),
				Error::<Test>::CannotDelegateIfLeaving
			);
			assert_noop!(
				StakePallet::candidate_stake_less(RuntimeOrigin::signed(1), 1),
				Error::<Test>::CannotStakeIfLeaving
			);
			assert_noop!(
				StakePallet::candidate_stake_more(RuntimeOrigin::signed(1), 1),
				Error::<Test>::CannotStakeIfLeaving
			);
			assert_noop!(
				StakePallet::init_leave_candidates(RuntimeOrigin::signed(1)),
				Error::<Test>::AlreadyLeaving
			);
			assert_eq!(StakePallet::candidate_pool(1).unwrap().status, CandidateStatus::Leaving(2));
			assert!(StakePallet::candidate_pool(1).unwrap().can_exit(2));
			assert!(!StakePallet::candidate_pool(1).unwrap().can_exit(1));
			assert!(StakePallet::candidate_pool(1).unwrap().can_exit(3));

			// next rounds starts, cannot leave yet
			roll_to(5, vec![]);
			assert_noop!(
				StakePallet::execute_leave_candidates(RuntimeOrigin::signed(2), 2),
				Error::<Test>::NotLeaving
			);
			assert_noop!(
				StakePallet::execute_leave_candidates(RuntimeOrigin::signed(2), 1),
				Error::<Test>::CannotLeaveYet
			);
			// add 11 as candidate to reach max size for TopCandidates and then try leave
			// again as 1 which should not be possible
			assert_ok!(StakePallet::join_candidates(RuntimeOrigin::signed(11), 100));
			assert_eq!(
				StakePallet::top_candidates().into_iter().map(|s| s.owner).collect::<Vec<u64>>(),
				(2u64..12u64).collect::<Vec<u64>>()
			);
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(11)));
			// join back
			assert_ok!(StakePallet::cancel_leave_candidates(RuntimeOrigin::signed(1)));
			assert_eq!(
				StakePallet::top_candidates().into_iter().map(|s| s.owner).collect::<Vec<u64>>(),
				(1u64..11u64).collect::<Vec<u64>>()
			);

			let stake: Vec<Stake<AccountId, Balance>> = (1u64..11u64)
				.zip(iter::once(210).chain(iter::repeat(100)))
				.map(|(id, amount)| StakeOf::<Test> { owner: id, amount })
				.collect();
			assert_eq!(StakePallet::top_candidates(), OrderedSet::from(stake.try_into().unwrap()));
			let state = StakePallet::candidate_pool(1).unwrap();
			assert_eq!(state.status, CandidateStatus::Active);
			assert_eq!(state.delegators.len(), 2);
			assert_eq!(state.total, 210);
			assert_eq!(
				state.total,
				StakePallet::top_candidates()
					.into_bounded_vec()
					.iter()
					.find(|other| other.owner == 1)
					.unwrap()
					.amount
			);
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2]);

			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(1)));

			roll_to(15, vec![]);
			assert_ok!(StakePallet::execute_leave_candidates(RuntimeOrigin::signed(13), 1));
			let mut unstaking: BoundedBTreeMap<
				BlockNumber,
				BalanceOf<Test>,
				<Test as Config>::MaxUnstakeRequests,
			> = BoundedBTreeMap::new();
			assert_ok!(unstaking.try_insert(17, 100));
			assert_eq!(StakePallet::unstaking(1), unstaking);
			assert_eq!(StakePallet::unstaking(12), unstaking);

			// cannot unlock yet
			roll_to(16, vec![]);
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(4), 1));
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(4), 12));
			assert_eq!(StakePallet::unstaking(1), unstaking);
			assert_eq!(StakePallet::unstaking(12), unstaking);

			// can unlock now
			roll_to(17, vec![]);
			unstaking.remove(&17);
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(4), 1));
			assert_ok!(StakePallet::unlock_unstaked(RuntimeOrigin::signed(4), 12));
			assert_eq!(StakePallet::unstaking(1), unstaking);
			assert_eq!(StakePallet::unstaking(12), unstaking);
		});
}

#[test]
fn increase_max_candidate_stake() {
	let max_stake = 160_000_000 * DECIMALS;
	ExtBuilder::default()
		.with_balances(vec![(1, 200_000_000 * DECIMALS)])
		.with_collators(vec![(1, max_stake)])
		.build()
		.execute_with(|| {
			assert_eq!(StakePallet::max_candidate_stake(), max_stake);
			assert_noop!(
				StakePallet::candidate_stake_more(RuntimeOrigin::signed(1), 1),
				Error::<Test>::ValStakeAboveMax
			);

			assert_ok!(StakePallet::set_max_candidate_stake(RuntimeOrigin::root(), max_stake + 1));
			assert_eq!(
				last_event(),
				MetaEvent::StakePallet(Event::MaxCandidateStakeChanged(max_stake + 1))
			);
			assert_eq!(StakePallet::max_candidate_stake(), max_stake + 1);
			assert_ok!(StakePallet::candidate_stake_more(RuntimeOrigin::signed(1), 1));
			assert_noop!(
				StakePallet::candidate_stake_more(RuntimeOrigin::signed(1), 1),
				Error::<Test>::ValStakeAboveMax
			);
		});
}

#[test]
fn decrease_max_candidate_stake() {
	ExtBuilder::default()
		.with_balances(vec![(1, 100), (2, 100), (3, 100), (4, 100), (5, 100)])
		.with_collators(vec![(1, 100), (2, 90), (3, 40)])
		.with_delegators(vec![(4, 2, 10), (5, 3, 20)])
		.build()
		.execute_with(|| {
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2]);
			assert_eq!(
				StakePallet::top_candidates(),
				OrderedSet::from_sorted_set(
					vec![
						StakeOf::<Test> { owner: 1, amount: 100 },
						StakeOf::<Test> { owner: 2, amount: 100 },
						StakeOf::<Test> { owner: 3, amount: 60 }
					]
					.try_into()
					.unwrap()
				)
			);

			assert_ok!(StakePallet::set_max_candidate_stake(RuntimeOrigin::root(), 100));
			assert_eq!(StakePallet::max_candidate_stake(), 100);
			assert_eq!(last_event(), MetaEvent::StakePallet(Event::MaxCandidateStakeChanged(100)));

			// check collator states, nothing changed
			assert_eq!(
				StakePallet::top_candidates(),
				OrderedSet::from_sorted_set(
					vec![
						StakeOf::<Test> { owner: 1, amount: 100 },
						StakeOf::<Test> { owner: 2, amount: 100 },
						StakeOf::<Test> { owner: 3, amount: 60 }
					]
					.try_into()
					.unwrap()
				)
			);

			assert_noop!(
				StakePallet::candidate_stake_more(RuntimeOrigin::signed(1), 0),
				Error::<Test>::ValStakeZero
			);
			assert_noop!(
				StakePallet::candidate_stake_less(RuntimeOrigin::signed(1), 0),
				Error::<Test>::ValStakeZero
			);
			assert_noop!(
				StakePallet::candidate_stake_more(RuntimeOrigin::signed(1), 1),
				Error::<Test>::ValStakeAboveMax
			);
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(1), 50));
			assert_noop!(
				StakePallet::set_max_candidate_stake(RuntimeOrigin::root(), 9),
				Error::<Test>::CannotSetBelowMin
			);
		});
}

#[test]
fn exceed_delegations_per_round() {
	ExtBuilder::default()
		.with_balances(vec![(1, 100), (2, 100), (3, 100), (4, 100), (5, 100), (6, 100)])
		.with_collators(vec![(1, 100), (2, 100), (3, 100), (4, 100), (5, 100)])
		.with_delegators(vec![(6, 1, 10)])
		.build()
		.execute_with(|| {
			assert_ok!(StakePallet::delegate_another_candidate(RuntimeOrigin::signed(6), 2, 10));
			assert_ok!(StakePallet::delegate_another_candidate(RuntimeOrigin::signed(6), 3, 10));
			assert_ok!(StakePallet::delegate_another_candidate(RuntimeOrigin::signed(6), 4, 10));
			assert_noop!(
				StakePallet::delegate_another_candidate(RuntimeOrigin::signed(6), 5, 10),
				Error::<Test>::MaxCollatorsPerDelegatorExceeded
			);

			// revoke delegation to allow one more collator for this delegator
			assert_ok!(StakePallet::revoke_delegation(RuntimeOrigin::signed(6), 4));
			// reached max delegations in this round
			assert_noop!(
				StakePallet::delegate_another_candidate(RuntimeOrigin::signed(6), 5, 10),
				Error::<Test>::DelegationsPerRoundExceeded
			);

			// revoke all delegations in the same round
			assert_ok!(StakePallet::leave_delegators(RuntimeOrigin::signed(6)));
			assert_noop!(
				StakePallet::join_delegators(RuntimeOrigin::signed(6), 1, 10),
				Error::<Test>::DelegationsPerRoundExceeded
			);

			// roll to next round to clear DelegationCounter
			roll_to(5, vec![]);
			assert_eq!(StakePallet::last_delegation(6), DelegationCounter { round: 0, counter: 4 });
			assert_ok!(StakePallet::join_delegators(RuntimeOrigin::signed(6), 1, 10),);
			assert_eq!(StakePallet::last_delegation(6), DelegationCounter { round: 1, counter: 1 });
			assert_ok!(StakePallet::leave_delegators(RuntimeOrigin::signed(6)));
			assert_ok!(StakePallet::join_delegators(RuntimeOrigin::signed(6), 1, 10),);
			assert_ok!(StakePallet::leave_delegators(RuntimeOrigin::signed(6)));
			assert_ok!(StakePallet::join_delegators(RuntimeOrigin::signed(6), 1, 10),);
			assert_ok!(StakePallet::leave_delegators(RuntimeOrigin::signed(6)));
			assert_ok!(StakePallet::join_delegators(RuntimeOrigin::signed(6), 1, 10),);
			assert_ok!(StakePallet::leave_delegators(RuntimeOrigin::signed(6)));
			assert_eq!(StakePallet::last_delegation(6), DelegationCounter { round: 1, counter: 4 });
			assert_noop!(
				StakePallet::join_delegators(RuntimeOrigin::signed(6), 1, 10),
				Error::<Test>::DelegationsPerRoundExceeded
			);
		});
}

#[test]
fn force_remove_candidate() {
	ExtBuilder::default()
		.with_balances(vec![(1, 100), (2, 100), (3, 100), (4, 100), (5, 100), (6, 100)])
		.with_collators(vec![(1, 100), (2, 100), (3, 100)])
		.with_delegators(vec![(4, 1, 50), (5, 1, 50)])
		.build()
		.execute_with(|| {
			assert_eq!(CandidatePool::<Test>::count(), 3);
			assert_ok!(StakePallet::delegate_another_candidate(RuntimeOrigin::signed(4), 2, 50));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2]);
			assert!(StakePallet::unstaking(1).get(&3).is_none());
			assert!(StakePallet::unstaking(2).get(&3).is_none());
			assert!(StakePallet::unstaking(3).get(&3).is_none());

			// force remove 1
			assert!(Session::disabled_validators().is_empty());
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 200, delegators: 150 }
			);
			assert_ok!(StakePallet::force_remove_candidate(RuntimeOrigin::root(), 1));
			// collator stake does not change since 3, who took 1's place, has staked the
			// same amount
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 200, delegators: 50 }
			);
			assert_eq!(Session::disabled_validators(), vec![0]);
			assert_eq!(last_event(), MetaEvent::StakePallet(Event::CollatorRemoved(1, 200)));
			assert!(
				!StakePallet::top_candidates().contains(&StakeOf::<Test> { owner: 1, amount: 100 })
			);
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![2, 3]);
			assert_eq!(CandidatePool::<Test>::count(), 2);
			assert!(StakePallet::candidate_pool(1).is_none());
			assert!(StakePallet::delegator_state(5).is_none());
			assert_eq!(
				StakePallet::delegator_state(4),
				Some(Delegator {
					delegations: OrderedSet::from(
						vec![StakeOf::<Test> { owner: 2, amount: 50 }].try_into().unwrap()
					),
					total: 50
				})
			);
			assert_eq!(StakePallet::unstaking(1).get(&3), Some(&100));
			assert_eq!(StakePallet::unstaking(4).get(&3), Some(&50));
			assert_eq!(StakePallet::unstaking(5).get(&3), Some(&50));

			assert_noop!(
				StakePallet::force_remove_candidate(RuntimeOrigin::root(), 2),
				Error::<Test>::TooFewCollatorCandidates
			);
			assert_noop!(
				StakePallet::force_remove_candidate(RuntimeOrigin::root(), 4),
				Error::<Test>::CandidateNotFound
			);

			// session 1: expect 1 to still be in validator set but as disabled
			roll_to(5, vec![]);
			assert_eq!(Session::current_index(), 1);
			assert_eq!(Session::validators(), vec![1, 2]);
			assert_eq!(Session::disabled_validators(), vec![0]);

			// session 2: expect validator set to have changed
			roll_to(10, vec![]);
			assert_eq!(Session::validators(), vec![2, 3]);
			assert!(Session::disabled_validators().is_empty());
		});
}

#[test]
fn prioritize_collators() {
	ExtBuilder::default()
		.with_balances(vec![(1, 200), (2, 200), (3, 200), (4, 200), (5, 200), (6, 200), (7, 200)])
		.with_collators(vec![(2, 100), (3, 100)])
		.build()
		.execute_with(|| {
			assert_eq!(
				StakePallet::top_candidates(),
				OrderedSet::from_sorted_set(
					vec![2, 3]
						.into_iter()
						.map(|id| StakeOf::<Test> { owner: id, amount: 100 })
						.collect::<Vec<StakeOf<Test>>>()
						.try_into()
						.unwrap()
				)
			);
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![2, 3]);
			assert_ok!(StakePallet::join_candidates(RuntimeOrigin::signed(1), 100));
			assert_eq!(
				StakePallet::top_candidates(),
				OrderedSet::from_sorted_set(
					vec![2, 3, 1]
						.into_iter()
						.map(|id| StakeOf::<Test> { owner: id, amount: 100 })
						.collect::<Vec<StakeOf<Test>>>()
						.try_into()
						.unwrap()
				)
			);
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![2, 3]);
			assert_ok!(StakePallet::init_leave_candidates(RuntimeOrigin::signed(2)));
			assert_eq!(StakePallet::top_candidates().len(), 2);
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![3, 1]);
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(3), 10));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 3]);

			// add 6
			assert_ok!(StakePallet::join_candidates(RuntimeOrigin::signed(6), 100));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 6]);
			assert_eq!(
				StakePallet::top_candidates(),
				OrderedSet::from_sorted_set(
					vec![1, 6]
						.into_iter()
						.map(|id| StakeOf::<Test> { owner: id, amount: 100 })
						.chain(vec![StakeOf::<Test> { owner: 3, amount: 90 }])
						.collect::<Vec<StakeOf<Test>>>()
						.try_into()
						.unwrap()
				)
			);

			// add 4
			assert_ok!(StakePallet::join_candidates(RuntimeOrigin::signed(4), 100));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 6]);
			assert_eq!(
				StakePallet::top_candidates(),
				OrderedSet::from_sorted_set(
					vec![1, 6, 4]
						.into_iter()
						.map(|id| StakeOf::<Test> { owner: id, amount: 100 })
						.chain(vec![StakeOf::<Test> { owner: 3, amount: 90 }])
						.collect::<Vec<StakeOf<Test>>>()
						.try_into()
						.unwrap()
				)
			);

			// add 5
			assert_ok!(StakePallet::join_candidates(RuntimeOrigin::signed(5), 100));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 6]);
			assert_eq!(
				StakePallet::top_candidates(),
				OrderedSet::from_sorted_set(
					vec![1, 6, 4, 5]
						.into_iter()
						.map(|id| StakeOf::<Test> { owner: id, amount: 100 })
						.chain(vec![StakeOf::<Test> { owner: 3, amount: 90 }])
						.collect::<Vec<StakeOf<Test>>>()
						.try_into()
						.unwrap()
				)
			);

			// 3 stake_more
			assert_ok!(StakePallet::candidate_stake_more(RuntimeOrigin::signed(3), 20));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![3, 1]);
			assert_eq!(
				StakePallet::top_candidates(),
				OrderedSet::from_sorted_set(
					vec![
						StakeOf::<Test> { owner: 3, amount: 110 },
						StakeOf::<Test> { owner: 1, amount: 100 },
						StakeOf::<Test> { owner: 6, amount: 100 },
						StakeOf::<Test> { owner: 4, amount: 100 },
						StakeOf::<Test> { owner: 5, amount: 100 },
					]
					.try_into()
					.unwrap()
				)
			);

			// 1 stake_less
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(1), 1));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![3, 6]);
			assert_eq!(
				StakePallet::top_candidates(),
				OrderedSet::from_sorted_set(
					vec![
						StakeOf::<Test> { owner: 3, amount: 110 },
						StakeOf::<Test> { owner: 6, amount: 100 },
						StakeOf::<Test> { owner: 4, amount: 100 },
						StakeOf::<Test> { owner: 5, amount: 100 },
						StakeOf::<Test> { owner: 1, amount: 99 },
					]
					.try_into()
					.unwrap()
				)
			);

			// 7 delegates to 4
			assert_ok!(StakePallet::join_delegators(RuntimeOrigin::signed(7), 5, 20));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![5, 3]);
			assert_eq!(
				StakePallet::top_candidates(),
				OrderedSet::from_sorted_set(
					vec![
						StakeOf::<Test> { owner: 5, amount: 120 },
						StakeOf::<Test> { owner: 3, amount: 110 },
						StakeOf::<Test> { owner: 6, amount: 100 },
						StakeOf::<Test> { owner: 4, amount: 100 },
						StakeOf::<Test> { owner: 1, amount: 99 },
					]
					.try_into()
					.unwrap()
				)
			);

			// 7 decreases delegation
			assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(7), 5, 10));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![5, 3]);
			assert_eq!(
				StakePallet::top_candidates(),
				OrderedSet::from_sorted_set(
					vec![
						StakeOf::<Test> { owner: 5, amount: 110 },
						StakeOf::<Test> { owner: 3, amount: 110 },
						StakeOf::<Test> { owner: 6, amount: 100 },
						StakeOf::<Test> { owner: 4, amount: 100 },
						StakeOf::<Test> { owner: 1, amount: 99 },
					]
					.try_into()
					.unwrap()
				)
			);
			assert_ok!(StakePallet::revoke_delegation(RuntimeOrigin::signed(7), 5));
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![3, 5]);
			assert_eq!(
				StakePallet::top_candidates(),
				OrderedSet::from_sorted_set(
					vec![
						StakeOf::<Test> { owner: 3, amount: 110 },
						StakeOf::<Test> { owner: 5, amount: 100 },
						StakeOf::<Test> { owner: 6, amount: 100 },
						StakeOf::<Test> { owner: 4, amount: 100 },
						StakeOf::<Test> { owner: 1, amount: 99 },
					]
					.try_into()
					.unwrap()
				)
			);
		});
}

#[test]
fn prioritize_delegators() {
	ExtBuilder::default()
		.with_balances(vec![
			(1, 1000),
			(2, 1000),
			(3, 1000),
			(4, 1000),
			(5, 1000),
			(6, 1000),
			(7, 1000),
		])
		.with_collators(vec![(1, 100), (2, 100), (3, 100)])
		.with_delegators(vec![(5, 1, 100), (4, 2, 100), (7, 2, 100), (6, 2, 100)])
		.build()
		.execute_with(|| {
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![2, 1]);
			assert_eq!(
				StakePallet::candidate_pool(2).unwrap().delegators,
				OrderedSet::from_sorted_set(
					vec![
						StakeOf::<Test> { owner: 4, amount: 100 },
						StakeOf::<Test> { owner: 7, amount: 100 },
						StakeOf::<Test> { owner: 6, amount: 100 },
					]
					.try_into()
					.unwrap()
				)
			);
			assert_ok!(StakePallet::delegate_another_candidate(RuntimeOrigin::signed(5), 2, 110));
			assert_eq!(
				StakePallet::candidate_pool(2).unwrap().delegators,
				OrderedSet::from_sorted_set(
					vec![
						StakeOf::<Test> { owner: 5, amount: 110 },
						StakeOf::<Test> { owner: 4, amount: 100 },
						StakeOf::<Test> { owner: 7, amount: 100 },
						StakeOf::<Test> { owner: 6, amount: 100 },
					]
					.try_into()
					.unwrap()
				)
			);
			assert_eq!(
				StakePallet::delegator_state(5).unwrap().delegations,
				OrderedSet::from_sorted_set(
					vec![
						StakeOf::<Test> { owner: 2, amount: 110 },
						StakeOf::<Test> { owner: 1, amount: 100 }
					]
					.try_into()
					.unwrap()
				)
			);

			// delegate_less
			assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(5), 2, 10));
			assert_eq!(
				StakePallet::candidate_pool(2).unwrap().delegators,
				OrderedSet::from_sorted_set(
					vec![
						StakeOf::<Test> { owner: 5, amount: 100 },
						StakeOf::<Test> { owner: 4, amount: 100 },
						StakeOf::<Test> { owner: 7, amount: 100 },
						StakeOf::<Test> { owner: 6, amount: 100 },
					]
					.try_into()
					.unwrap()
				)
			);
			assert_eq!(
				StakePallet::delegator_state(5).unwrap().delegations,
				OrderedSet::from_sorted_set(
					vec![
						StakeOf::<Test> { owner: 2, amount: 100 },
						StakeOf::<Test> { owner: 1, amount: 100 }
					]
					.try_into()
					.unwrap()
				)
			);

			// delegate_more
			assert_ok!(StakePallet::delegator_stake_more(RuntimeOrigin::signed(6), 2, 10));
			assert_eq!(
				StakePallet::candidate_pool(2).unwrap().delegators,
				OrderedSet::from_sorted_set(
					vec![
						StakeOf::<Test> { owner: 6, amount: 110 },
						StakeOf::<Test> { owner: 5, amount: 100 },
						StakeOf::<Test> { owner: 4, amount: 100 },
						StakeOf::<Test> { owner: 7, amount: 100 },
					]
					.try_into()
					.unwrap()
				)
			);
			assert_ok!(StakePallet::delegator_stake_more(RuntimeOrigin::signed(7), 2, 10));
			assert_eq!(
				StakePallet::candidate_pool(2).unwrap().delegators,
				OrderedSet::from_sorted_set(
					vec![
						StakeOf::<Test> { owner: 6, amount: 110 },
						StakeOf::<Test> { owner: 7, amount: 110 },
						StakeOf::<Test> { owner: 5, amount: 100 },
						StakeOf::<Test> { owner: 4, amount: 100 },
					]
					.try_into()
					.unwrap()
				)
			);
		});
}

#[test]
fn authorities_per_round() {
	let stake = 100 * DECIMALS;
	ExtBuilder::default()
		.with_balances(vec![
			(1, stake),
			(2, stake),
			(3, stake),
			(4, stake),
			(5, stake),
			(6, stake),
			(7, stake),
			(8, stake),
			(9, stake),
			(10, stake),
			(11, 100 * stake),
		])
		.with_collators(vec![(1, stake), (2, stake), (3, stake), (4, stake)])
		.build()
		.execute_with(|| {
			assert_eq!(StakePallet::selected_candidates().into_inner(), vec![1, 2]);
			// reward 1 once per round
			let authors: Vec<Option<AccountId>> =
				(0u64..=100).map(|i| if i % 5 == 2 { Some(1u64) } else { None }).collect();

			// roll to new round 1
			let reward_0 = 1000;
			roll_to(BLOCKS_PER_ROUND, authors.clone());
			assert_eq!(Balances::free_balance(1), stake + reward_0);
			// increase max selected candidates which will become effective in round 2
			assert_ok!(StakePallet::set_max_selected_candidates(RuntimeOrigin::root(), 10));

			// roll to new round 2
			roll_to(BLOCKS_PER_ROUND * 2, authors.clone());
			assert_eq!(Balances::free_balance(1), stake + reward_0 * 2);

			// roll to new round 3
			roll_to(BLOCKS_PER_ROUND * 3, authors.clone());
			assert_eq!(Balances::free_balance(1), stake + reward_0 * 3);

			// roll to new round 4
			roll_to(BLOCKS_PER_ROUND * 4, authors);
			assert_eq!(Balances::free_balance(1), stake + reward_0 * 4);
		});
}

#[test]
fn force_new_round() {
	ExtBuilder::default()
		.with_balances(vec![(1, 100), (2, 100), (3, 100), (4, 100), (5, 100), (6, 100)])
		.with_collators(vec![(1, 100), (2, 100), (3, 100), (4, 100)])
		.build()
		.execute_with(|| {
			let mut round = RoundInfo { current: 0, first: 0, length: 5 };
			assert_eq!(StakePallet::round(), round);
			assert_eq!(Session::validators(), vec![1, 2]);
			assert_eq!(Session::current_index(), 0);
			// 3 should be validator in round 2
			assert_ok!(StakePallet::join_delegators(RuntimeOrigin::signed(5), 3, 100));

			// init force new round from 0 to 1, updating the authorities
			assert_ok!(StakePallet::force_new_round(RuntimeOrigin::root()));
			assert_eq!(StakePallet::round(), round);
			assert_eq!(Session::current_index(), 0);
			assert!(StakePallet::new_round_forced());

			// force new round should become active by starting next block
			roll_to(2, vec![]);
			round = RoundInfo { current: 1, first: 2, length: 5 };
			assert_eq!(Session::current_index(), 1);
			assert_eq!(Session::validators(), vec![1, 2]);
			assert!(!StakePallet::new_round_forced());

			// roll to next block in same round 1
			roll_to(3, vec![]);
			assert_eq!(Session::current_index(), 1);
			assert_eq!(StakePallet::round(), round);
			// assert_eq!(Session::validators(), vec![3, 1]);
			assert!(!StakePallet::new_round_forced());
			// 4 should become validator in session 3 if we do not force a new round
			assert_ok!(StakePallet::join_delegators(RuntimeOrigin::signed(6), 4, 100));

			// end session 2 naturally
			roll_to(7, vec![]);
			round = RoundInfo { current: 2, first: 7, length: 5 };
			assert_eq!(StakePallet::round(), round);
			assert_eq!(Session::current_index(), 2);
			assert!(!StakePallet::new_round_forced());
			assert_eq!(Session::validators(), vec![3, 1]);

			// force new round 3
			assert_ok!(StakePallet::force_new_round(RuntimeOrigin::root()));
			assert_eq!(StakePallet::round(), round);
			assert_eq!(Session::current_index(), 2);
			// validator set should not change until next round
			assert_eq!(Session::validators(), vec![3, 1]);
			assert!(StakePallet::new_round_forced());

			// force new round should become active by starting next block
			roll_to(8, vec![]);
			round = RoundInfo { current: 3, first: 8, length: 5 };
			assert_eq!(Session::current_index(), 3);
			assert_eq!(StakePallet::round(), round);
			assert_eq!(Session::validators(), vec![3, 4]);
			assert!(!StakePallet::new_round_forced());
		});
}

#[test]
fn replace_lowest_delegator() {
	ExtBuilder::default()
		.with_balances(vec![(1, 100), (2, 100), (3, 100), (4, 100), (5, 100), (6, 100)])
		.with_collators(vec![(1, 100)])
		.with_delegators(vec![(2, 1, 51), (3, 1, 51), (4, 1, 51), (5, 1, 50)])
		.build()
		.execute_with(|| {
			assert_eq!(
				StakePallet::candidate_pool(1).unwrap().delegators.len() as u32,
				<Test as Config>::MaxDelegatorsPerCollator::get()
			);

			// 6 replaces 5
			assert_ok!(StakePallet::join_delegators(RuntimeOrigin::signed(6), 1, 51));
			assert!(StakePallet::delegator_state(5).is_none());
			assert_eq!(
				StakePallet::candidate_pool(1)
					.unwrap()
					.delegators
					.into_bounded_vec()
					.into_inner(),
				vec![
					Stake { owner: 2, amount: 51 },
					Stake { owner: 3, amount: 51 },
					Stake { owner: 4, amount: 51 },
					Stake { owner: 6, amount: 51 }
				]
			);

			// 5 attempts to replace 6 with more balance than available
			frame_support::assert_noop!(
				StakePallet::join_delegators(RuntimeOrigin::signed(5), 1, 101),
				BalancesError::<Test>::InsufficientBalance
			);
			assert!(StakePallet::delegator_state(6).is_some());
		})
}

#[test]
fn update_total_stake_collators_stay() {
	ExtBuilder::default()
		.with_balances(vec![(1, 200), (2, 200), (3, 200), (4, 200)])
		.with_collators(vec![(1, 100), (2, 50)])
		.with_delegators(vec![(3, 1, 100), (4, 2, 50)])
		.build()
		.execute_with(|| {
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 150, delegators: 150 }
			);
			assert_ok!(StakePallet::candidate_stake_more(RuntimeOrigin::signed(1), 10));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 160, delegators: 150 }
			);
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(2), 5));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 155, delegators: 150 }
			);
			assert_ok!(StakePallet::delegator_stake_more(RuntimeOrigin::signed(3), 1, 10));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 155, delegators: 160 }
			);
			assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(4), 2, 5));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 155, delegators: 155 }
			);
		});
}

#[test]
fn update_total_stake_displace_collators() {
	ExtBuilder::default()
		.with_balances(vec![
			(1, 200),
			(2, 200),
			(3, 200),
			(4, 200),
			(5, 200),
			(6, 200),
			(7, 200),
			(8, 200),
			(1337, 200),
		])
		.with_collators(vec![(1, 10), (2, 20), (3, 30), (4, 40)])
		.with_delegators(vec![(5, 1, 50), (6, 2, 50), (7, 3, 55), (8, 4, 55)])
		.build()
		.execute_with(|| {
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 70, delegators: 110 }
			);

			// 4 is pushed out by staking less
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(4), 30));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 50, delegators: 105 }
			);
			assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(8), 4, 45));

			// 3 is pushed out by delegator staking less
			assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(7), 3, 45));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 30, delegators: 100 }
			);

			// 1 is pushed out by new candidate
			assert_ok!(StakePallet::join_candidates(RuntimeOrigin::signed(1337), 100));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 120, delegators: 50 }
			);
		});
}

#[test]
fn update_total_stake_new_collators() {
	ExtBuilder::default()
		.with_balances(vec![(1, 100), (2, 100), (3, 100), (4, 100)])
		.with_collators(vec![(1, 100)])
		.with_delegators(vec![(4, 1, 100)])
		.build()
		.execute_with(|| {
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 100, delegators: 100 }
			);
			assert_ok!(StakePallet::join_candidates(RuntimeOrigin::signed(2), 100));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 200, delegators: 100 }
			);
			assert_ok!(StakePallet::join_delegators(RuntimeOrigin::signed(3), 2, 50));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 200, delegators: 150 }
			);
			assert_ok!(StakePallet::leave_delegators(RuntimeOrigin::signed(4)));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 200, delegators: 50 }
			);
		});
}

#[test]
fn update_total_stake_no_collator_changes() {
	ExtBuilder::default()
		.with_balances(vec![
			(1, 200),
			(2, 200),
			(3, 200),
			(4, 200),
			(5, 200),
			(6, 200),
			(7, 200),
			(8, 200),
			(1337, 200),
		])
		.with_collators(vec![(1, 10), (2, 20), (3, 30), (4, 40)])
		.with_delegators(vec![(5, 1, 50), (6, 2, 50), (7, 3, 55), (8, 4, 55)])
		.build()
		.execute_with(|| {
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 70, delegators: 110 }
			);
			assert_ok!(StakePallet::candidate_stake_more(RuntimeOrigin::signed(1), 10));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 70, delegators: 110 }
			);
			assert_ok!(StakePallet::delegator_stake_more(RuntimeOrigin::signed(5), 1, 10));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 70, delegators: 110 }
			);
			assert_ok!(StakePallet::candidate_stake_less(RuntimeOrigin::signed(2), 10));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 70, delegators: 110 }
			);
			assert_ok!(StakePallet::delegator_stake_less(RuntimeOrigin::signed(6), 2, 10));
			assert_eq!(
				StakePallet::total_collator_stake(),
				TotalStake { collators: 70, delegators: 110 }
			);
		});
}

#[test]
fn check_collator_block() {
	let stake = 100 * DECIMALS;
	ExtBuilder::default()
		.with_balances(vec![(1, stake), (2, stake), (3, stake), (4, stake)])
		.with_collators(vec![(1, stake), (2, stake), (3, stake), (4, stake)])
		.build()
		.execute_with(|| {
			let authors: Vec<Option<AccountId>> =
				vec![None, Some(1u64), Some(1u64), Some(3u64), Some(4u64), Some(1u64)];

			roll_to(2, authors.clone());
			assert_eq!(StakePallet::collator_blocks(1), 1);
			assert_eq!(StakePallet::collator_blocks(2), 0);
			assert_eq!(StakePallet::collator_blocks(3), 0);
			assert_eq!(StakePallet::collator_blocks(4), 0);

			roll_to(3, authors.clone());
			assert_eq!(StakePallet::collator_blocks(1), 2);
			assert_eq!(StakePallet::collator_blocks(2), 0);
			assert_eq!(StakePallet::collator_blocks(3), 0);
			assert_eq!(StakePallet::collator_blocks(4), 0);

			roll_to(4, authors.clone());
			assert_eq!(StakePallet::collator_blocks(1), 2);
			assert_eq!(StakePallet::collator_blocks(2), 0);
			assert_eq!(StakePallet::collator_blocks(3), 1);
			assert_eq!(StakePallet::collator_blocks(4), 0);

			// Because the new session start, we'll add the counter and clean the all collator
			// blocks immediately the session number is BLOCKS_PER_ROUND (5)
			roll_to(5, authors.clone());
			assert_eq!(StakePallet::collator_blocks(1), 0);
			assert_eq!(StakePallet::collator_blocks(2), 0);
			assert_eq!(StakePallet::collator_blocks(3), 0);
			assert_eq!(StakePallet::collator_blocks(4), 0);
		});
}

#[test]
fn check_claim_block_normal_wo_delegator() {
	let stake = 100 * DECIMALS;
	let origin_balance = 100 * stake;
	ExtBuilder::default()
		.with_balances(vec![
			(1, origin_balance),
			(2, origin_balance),
			(3, origin_balance),
			(4, origin_balance),
		])
		.with_collators(vec![(1, 1 * stake), (2, 2 * stake), (3, 3 * stake), (4, 4 * stake)])
		.build()
		.execute_with(|| {
			let authors: Vec<Option<AccountId>> = vec![
				None,
				Some(1u64),
				Some(2u64),
				Some(3u64),
				Some(4u64),
				Some(1u64),
				Some(1u64),
				Some(1u64),
				Some(1u64),
				Some(1u64),
			];

			roll_to(5, authors.clone());

			assert_eq!(
				Balances::free_balance(1),
				Perquintill::from_float(1. / 10.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);
			assert_eq!(
				Balances::free_balance(2),
				Perquintill::from_float(2. / 10.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);
			assert_eq!(
				Balances::free_balance(3),
				Perquintill::from_float(3. / 10.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);
			assert_eq!(
				Balances::free_balance(4),
				Perquintill::from_float(4. / 10.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);

			// Cross session but only 1 is selected
			roll_to(10, authors.clone());
			assert_eq!(
				Balances::free_balance(1),
				Perquintill::from_float(1. / 10.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					BLOCK_REWARD_IN_NORMAL_SESSION +
					origin_balance
			);
			assert_eq!(
				Balances::free_balance(2),
				Perquintill::from_float(2. / 10.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);
			assert_eq!(
				Balances::free_balance(3),
				Perquintill::from_float(3. / 10.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);
			assert_eq!(
				Balances::free_balance(4),
				Perquintill::from_float(4. / 10.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);
		});
}

#[test]
fn check_claim_block_normal_wi_delegator() {
	let stake = 100 * DECIMALS;
	let origin_balance = 100 * stake;
	ExtBuilder::default()
		.with_balances(vec![
			(1, origin_balance),
			(2, origin_balance),
			(3, origin_balance),
			(4, origin_balance),
			(5, origin_balance),
			(6, origin_balance),
			(7, origin_balance),
			(8, origin_balance),
			(9, origin_balance),
			(10, origin_balance),
		])
		.with_collators(vec![(1, 1 * stake), (2, 2 * stake), (3, 3 * stake), (4, 4 * stake)])
		.with_delegators(vec![
			(5, 1, 5 * stake),
			(6, 1, 6 * stake),
			(7, 2, 7 * stake),
			(8, 3, 8 * stake),
			(9, 4, 9 * stake),
			(10, 4, 10 * stake),
		])
		.build()
		.execute_with(|| {
			let authors: Vec<Option<AccountId>> = vec![
				None,
				Some(1u64),
				Some(2u64),
				Some(3u64),
				Some(4u64),
				Some(1u64),
				Some(1u64),
				Some(1u64),
				Some(1u64),
				Some(1u64),
			];

			roll_to(5, authors.clone());

			assert_eq!(
				Balances::free_balance(1),
				Perquintill::from_float(1. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);
			assert_eq!(
				Balances::free_balance(5),
				Perquintill::from_float(5. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);
			assert_eq!(
				Balances::free_balance(6),
				Perquintill::from_float(6. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);

			assert_eq!(
				Balances::free_balance(2),
				Perquintill::from_float(2. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);
			assert_eq!(
				Balances::free_balance(7),
				Perquintill::from_float(7. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);

			assert_eq!(
				Balances::free_balance(3),
				Perquintill::from_float(3. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);
			assert_eq!(
				Balances::free_balance(8),
				Perquintill::from_float(8. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);

			assert_eq!(
				Balances::free_balance(4),
				Perquintill::from_float(4. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);
			assert_eq!(
				Balances::free_balance(9),
				Perquintill::from_float(9. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);
			assert_eq!(
				Balances::free_balance(10),
				Perquintill::from_float(10. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);

			// Cross session but only 1 is selected
			roll_to(10, authors.clone());

			assert_eq!(
				Balances::free_balance(1),
				Perquintill::from_float(1. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					Perquintill::from_float(1. / 12.) * BLOCK_REWARD_IN_NORMAL_SESSION +
					origin_balance
			);
			assert_eq!(
				Balances::free_balance(5),
				Perquintill::from_float(5. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					Perquintill::from_float(5. / 12.) * BLOCK_REWARD_IN_NORMAL_SESSION +
					origin_balance
			);
			assert_eq!(
				Balances::free_balance(6),
				Perquintill::from_float(6. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					Perquintill::from_float(6. / 12.) * BLOCK_REWARD_IN_NORMAL_SESSION +
					origin_balance
			);

			// Nothing change
			assert_eq!(
				Balances::free_balance(2),
				Perquintill::from_float(2. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);
			assert_eq!(
				Balances::free_balance(7),
				Perquintill::from_float(7. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);

			assert_eq!(
				Balances::free_balance(3),
				Perquintill::from_float(3. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);
			assert_eq!(
				Balances::free_balance(8),
				Perquintill::from_float(8. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);

			assert_eq!(
				Balances::free_balance(4),
				Perquintill::from_float(4. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);
			assert_eq!(
				Balances::free_balance(9),
				Perquintill::from_float(9. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);
			assert_eq!(
				Balances::free_balance(10),
				Perquintill::from_float(10. / 55.) * BLOCK_REWARD_IN_GENESIS_SESSION +
					origin_balance
			);
		});
}

#[test]
fn collator_reward_per_session_only_collator() {
	ExtBuilder::default()
		.with_balances(vec![(1, 1000)])
		.with_collators(vec![(1, 500)])
		.build()
		.execute_with(|| {
			assert!(System::events().is_empty());

			let state = CandidatePool::<Test>::get(1).unwrap();
			// Avoid keep live error
			assert_ok!(Balances::force_set_balance(
				RawOrigin::Root.into(),
				StakePallet::account_id(),
				1000,
			));

			let reward = StakePallet::get_collator_reward_per_session(&state, 10, 50000, 1000);
			assert_eq!(reward, Reward { owner: 1, amount: 100 });
		});
}

#[test]
fn collator_reward_per_session_with_delegator() {
	ExtBuilder::default()
		.with_balances(vec![(1, 1000), (2, 1000), (3, 1000)])
		.with_collators(vec![(1, 500)])
		.with_delegators(vec![(2, 1, 600), (3, 1, 400)])
		.build()
		.execute_with(|| {
			assert!(System::events().is_empty());

			let state = CandidatePool::<Test>::get(1).unwrap();
			// Avoid keep live error
			assert_ok!(Balances::force_set_balance(
				RawOrigin::Root.into(),
				StakePallet::account_id(),
				1000,
			));

			let rewards = StakePallet::get_delgators_reward_per_session(&state, 10, 50000, 1000);
			assert_eq!(
				rewards[0],
				Reward {
					owner: 2,
					amount: Perquintill::from_rational(10 as u64 * 600, 50000) * 1000
				}
			);
			assert_eq!(
				rewards[1],
				Reward {
					owner: 3,
					amount: Perquintill::from_rational(10 as u64 * 400, 50000) * 1000
				}
			);
		});
}

#[test]
fn check_total_collator_staking_num() {
	ExtBuilder::default()
		.with_balances(vec![(1, 1000), (2, 1000), (3, 1000), (4, 1000), (5, 1000)])
		.with_collators(vec![(1, 500), (4, 100)])
		.with_delegators(vec![(2, 1, 600), (3, 1, 400), (5, 4, 200)])
		.build()
		.execute_with(|| {
			assert!(System::events().is_empty());

			let authors: Vec<Option<AccountId>> =
				vec![None, Some(1u64), Some(1u64), Some(4u64), Some(4u64), Some(1u64)];

			roll_to(4, authors.clone());

			let (_weight, balance) = StakePallet::get_total_collator_staking_num();
			assert_eq!(balance, 2 * (500 + 600 + 400) + 1 * (100 + 200));
		});
}

#[test]
fn delegated_funds_less_than_min_delegator_stake() {
	ExtBuilder::default()
		.with_balances(vec![
			(1, 100),
			(2, 100),
			(3, 100),
			(4, 100),
			(5, 100),
			(6, 100),
			(7, 100),
			(8, 100),
			(9, 100),
			(10, 100),
		])
		.with_collators(vec![(1, 20), (2, 20), (3, 20), (4, 20), (5, 10)])
		.with_delegators(vec![(6, 1, 10), (7, 1, 10), (8, 2, 10), (9, 2, 10)])
		.set_blocks_per_round(5)
		.build()
		.execute_with(|| {
			// delegate funds to multiple candidates.
			roll_to(4, vec![]);
			assert_ok!(StakePallet::delegate_another_candidate(RuntimeOrigin::signed(6), 2, 9));
			assert_ok!(StakePallet::delegate_another_candidate(RuntimeOrigin::signed(6), 3, 8));
			assert_ok!(StakePallet::delegate_another_candidate(RuntimeOrigin::signed(6), 4, 7));
			// reduce the 6 delegated funds of collator owned by `1`.
			assert_noop!(
				StakePallet::delegator_stake_less(RuntimeOrigin::signed(6), 1, 8),
				Error::<Test>::DelegationBelowMin
			);
		});
}

#[test]
fn max_candidate_stake_over_max_staking() {
	let max_stake = 160_000_000 * DECIMALS;
	ExtBuilder::default()
		.with_balances(vec![(1, 200_000_000 * DECIMALS)])
		.with_collators(vec![(1, max_stake)])
		.build()
		.execute_with(|| {
			assert_noop!(
				StakePallet::set_max_candidate_stake(RuntimeOrigin::root(), max_stake - 1),
				Error::<Test>::ValStakeAboveMax
			);
		});
}

#[test]
fn collator_reward_per_session_with_delegator_and_commission() {
	ExtBuilder::default()
		.with_balances(vec![(1, 1000), (2, 1000), (3, 1000)])
		.with_collators(vec![(1, 500)])
		.with_delegators(vec![(2, 1, 600), (3, 1, 400)])
		.build()
		.execute_with(|| {
			assert!(System::events().is_empty());

			let mut state = CandidatePool::<Test>::get(1).unwrap();
			state.set_commission(Permill::from_percent(10)); // 10% commission rate
			assert_ok!(Balances::force_set_balance(
				RawOrigin::Root.into(),
				StakePallet::account_id(),
				1000,
			));

			let reward = StakePallet::get_collator_reward_per_session(&state, 10, 50000, 1000);
			assert_eq!(reward, Reward { owner: 1, amount: 120 });
			let rewards = StakePallet::get_delgators_reward_per_session(&state, 10, 50000, 1000);
			assert_eq!(
				rewards[0],
				Reward {
					owner: 2,
					amount: (Perquintill::from_rational(10u64 * 600, 50000) * 1000) * 90 / 100
				}
			);
			assert_eq!(
				rewards[1],
				Reward {
					owner: 3,
					amount: (Perquintill::from_rational(10u64 * 400, 50000) * 1000) * 90 / 100
				}
			);
			state.set_commission(Permill::from_percent(100)); // 10% commission rate
			let reward = StakePallet::get_collator_reward_per_session(&state, 10, 50000, 1000);
			assert_eq!(reward, Reward { owner: 1, amount: 300 });
			let rewards = StakePallet::get_delgators_reward_per_session(&state, 10, 50000, 1000);
			assert_eq!(rewards[0], Reward { owner: 2, amount: 0 });
			assert_eq!(rewards[1], Reward { owner: 3, amount: 0 });
		});
}
