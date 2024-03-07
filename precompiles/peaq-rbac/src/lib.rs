// Copyright (C) 2020-2023 Peaq Foundation.

#![cfg_attr(not(feature = "std"), no_std)]

// primitives and utils imports
use fp_evm::PrecompileHandle;
use frame_support::{
	dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo},
	traits::ConstU32,
};
use peaq_pallet_rbac::{
	error::{RbacError, RbacErrorType},
	rbac::{Rbac, Role},
};
use precompile_utils::prelude::*;
use sp_core::{Decode, H256};
use sp_std::{marker::PhantomData, vec::Vec};

use pallet_evm::AddressMapping;
use peaq_pallet_rbac::rbac::{Group, Permission};

pub mod structs;
pub use structs::*;

pub mod selectors;
pub use selectors::*;

type AccountIdOf<Runtime> = <Runtime as frame_system::Config>::AccountId;
type EntityIdOf<Runtime> = <Runtime as peaq_pallet_rbac::Config>::EntityId;

type GetBytesLimit = ConstU32<{ 2u32.pow(16) }>;

pub fn err2str(error: &RbacError) -> &str {
	match error {
		RbacError { typ: RbacErrorType::EntityAlreadyExist, .. } => "RbacError.EntityAlreadyExists",
		RbacError { typ: RbacErrorType::EntityDoesNotExist, .. } => "RbacError.EntityDoesNotExist",
		RbacError { typ: RbacErrorType::EntityAuthorizationFailed, .. } =>
			"RbacError.EntityAuthorizationFailed",
		RbacError { typ: RbacErrorType::EntityDisabled, .. } => "RbacError.EntityDisabled",
		RbacError { typ: RbacErrorType::AssignmentAlreadyExist, .. } =>
			"RbacError.AssignmentAlreadyExist",
		RbacError { typ: RbacErrorType::AssignmentDoesNotExist, .. } =>
			"RbacError.AssignmentDoesNotExist",
		RbacError { typ: RbacErrorType::NameExceedMaxChar, .. } => "RbacError.NameExceedMaxChar",
		RbacError { typ: RbacErrorType::StorageExceedsMaxBounds, .. } =>
			"RbacError.StorageExceedsMaxBounds",
	}
}

// Precompule struct
// NOTE: Both AccoundId and EntityId are sized and aligned at 32 and 0x1, hence using H256 to
// represent both.
pub struct PeaqRbacPrecompile<Runtime>(PhantomData<Runtime>);

#[precompile_utils::precompile]
impl<Runtime> PeaqRbacPrecompile<Runtime>
where
	Runtime: pallet_evm::Config + peaq_pallet_rbac::Config + frame_system::pallet::Config,
	Runtime::RuntimeCall: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo + Decode,
	Runtime::RuntimeCall: From<peaq_pallet_rbac::Call<Runtime>>,
	<Runtime::RuntimeCall as Dispatchable>::RuntimeOrigin: From<Option<AccountIdOf<Runtime>>>,
	AccountIdOf<Runtime>: From<[u8; 32]>,
	EntityIdOf<Runtime>: From<[u8; 32]>,
	H256: From<<Runtime as peaq_pallet_rbac::Config>::EntityId>,
{
	#[precompile::public("fetch_role(bytes32,bytes32)")]
	#[precompile::view]
	fn fetch_role(
		handle: &mut impl PrecompileHandle,
		owner: H256,
		entity: H256,
	) -> EvmResult<Entity> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let owner_account = AccountIdOf::<Runtime>::from(owner.to_fixed_bytes());
		let entity_id = EntityIdOf::<Runtime>::from(entity.to_fixed_bytes());

		match peaq_pallet_rbac::Pallet::<Runtime>::get_role(&owner_account, entity_id) {
			Err(_e) => Err(Revert::new(RevertReason::custom(err2str(&_e))).into()),
			Ok(v) => Ok(Entity { id: v.id.into(), name: v.name.into(), enabled: v.enabled }),
		}
	}

	#[precompile::public("fetch_roles(bytes32)")]
	#[precompile::view]
	fn fetch_roles(handle: &mut impl PrecompileHandle, owner: H256) -> EvmResult<Vec<Entity>> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let owner_account = AccountIdOf::<Runtime>::from(owner.to_fixed_bytes());

		let result = match peaq_pallet_rbac::Pallet::<Runtime>::get_roles(&owner_account) {
			Err(_e) => Err(Revert::new(RevertReason::custom(err2str(&_e))).into()),
			Ok(v) => Ok(v
				.iter()
				.map(|entity| Entity {
					id: entity.id.into(),
					name: entity.name.clone().into(),
					enabled: entity.enabled,
				})
				.collect::<Vec<Entity>>()),
		};

		result
	}

	#[precompile::public("add_role(bytes32,bytes)")]
	fn add_role(
		handle: &mut impl PrecompileHandle,
		role_id: H256,
		name: BoundedBytes<GetBytesLimit>,
	) -> EvmResult<bool> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let caller_addr: AccountIdOf<Runtime> =
			Runtime::AddressMapping::into_account_id(handle.context().caller);
		let role_id_addr: EntityIdOf<Runtime> =
			EntityIdOf::<Runtime>::from(role_id.to_fixed_bytes());

		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(caller_addr).into(),
			peaq_pallet_rbac::Call::<Runtime>::add_role {
				role_id: role_id_addr,
				name: name.as_bytes().to_vec(),
			},
			0,
		)?;

		let event = log1(
			handle.context().address,
			SELECTOR_LOG_ADD_ROLE,
			solidity::encode_event_data((Address::from(handle.context().caller), role_id, name)),
		);
		event.record(handle)?;

		Ok(true)
	}

	#[precompile::public("update_role(bytes32,bytes)")]
	fn update_role(
		handle: &mut impl PrecompileHandle,
		role_id: H256,
		name: BoundedBytes<GetBytesLimit>,
	) -> EvmResult<bool> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let caller_addr: AccountIdOf<Runtime> =
			Runtime::AddressMapping::into_account_id(handle.context().caller);
		let role_id_addr: EntityIdOf<Runtime> =
			EntityIdOf::<Runtime>::from(role_id.to_fixed_bytes());

		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(caller_addr).into(),
			peaq_pallet_rbac::Call::<Runtime>::update_role {
				role_id: role_id_addr,
				name: name.as_bytes().to_vec(),
			},
			0,
		)?;

		let event = log1(
			handle.context().address,
			SELECTOR_LOG_UPDATE_ROLE,
			solidity::encode_event_data((Address::from(handle.context().caller), role_id, name)),
		);
		event.record(handle)?;

		Ok(true)
	}

	#[precompile::public("disable_role(bytes32)")]
	fn disable_role(handle: &mut impl PrecompileHandle, role_id: H256) -> EvmResult<bool> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let caller_addr: AccountIdOf<Runtime> =
			Runtime::AddressMapping::into_account_id(handle.context().caller);
		let role_id_addr: EntityIdOf<Runtime> =
			EntityIdOf::<Runtime>::from(role_id.to_fixed_bytes());

		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(caller_addr).into(),
			peaq_pallet_rbac::Call::<Runtime>::disable_role { role_id: role_id_addr },
			0,
		)?;

		let event = log1(
			handle.context().address,
			SELECTOR_LOG_DISABLE_ROLE,
			solidity::encode_event_data((Address::from(handle.context().caller), role_id)),
		);
		event.record(handle)?;

		Ok(true)
	}

	#[precompile::public("fetch_user_roles(bytes32,bytes32)")]
	#[precompile::view]
	fn fetch_user_roles(
		handle: &mut impl PrecompileHandle,
		owner: H256,
		user_id: H256,
	) -> EvmResult<Vec<Role2User>> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let user_id_addr: EntityIdOf<Runtime> =
			EntityIdOf::<Runtime>::from(user_id.to_fixed_bytes());
		let owner_addr = AccountIdOf::<Runtime>::from(owner.to_fixed_bytes());

		let result =
			match peaq_pallet_rbac::Pallet::<Runtime>::get_user_roles(&owner_addr, user_id_addr) {
				Err(_e) => Err(Revert::new(RevertReason::custom(err2str(&_e))).into()),
				Ok(v) => Ok(v
					.iter()
					.map(|val| Role2User { role: val.role.into(), user: val.user.into() })
					.collect::<Vec<Role2User>>()),
			};

		result
	}

	#[precompile::public("assign_role_to_user(bytes32,bytes32)")]
	fn assign_role_to_user(
		handle: &mut impl PrecompileHandle,
		role_id: H256,
		user_id: H256,
	) -> EvmResult<bool> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let caller_addr: AccountIdOf<Runtime> =
			Runtime::AddressMapping::into_account_id(handle.context().caller);

		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(caller_addr).into(),
			peaq_pallet_rbac::Call::<Runtime>::assign_role_to_user {
				role_id: EntityIdOf::<Runtime>::from(role_id.to_fixed_bytes()),
				user_id: EntityIdOf::<Runtime>::from(user_id.to_fixed_bytes()),
			},
			0,
		)?;

		let event = log1(
			handle.context().address,
			SELECTOR_LOG_ASSIGN_ROLE_TO_USER,
			solidity::encode_event_data((Address::from(handle.context().caller), role_id, user_id)),
		);
		event.record(handle)?;

		Ok(true)
	}

	#[precompile::public("unassign_role_to_user(bytes32,bytes32)")]
	fn unassign_role_to_user(
		handle: &mut impl PrecompileHandle,
		role_id: H256,
		user_id: H256,
	) -> EvmResult<bool> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let caller_addr: AccountIdOf<Runtime> =
			Runtime::AddressMapping::into_account_id(handle.context().caller);

		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(caller_addr).into(),
			peaq_pallet_rbac::Call::<Runtime>::unassign_role_to_user {
				role_id: EntityIdOf::<Runtime>::from(role_id.to_fixed_bytes()),
				user_id: EntityIdOf::<Runtime>::from(user_id.to_fixed_bytes()),
			},
			0,
		)?;

		let event = log1(
			handle.context().address,
			SELECTOR_LOG_UNASSIGNED_ROLE_TO_USER,
			solidity::encode_event_data((Address::from(handle.context().caller), role_id, user_id)),
		);
		event.record(handle)?;

		Ok(true)
	}

	#[precompile::public("fetch_permission(bytes32,bytes32)")]
	#[precompile::view]
	fn fetch_permission(
		handle: &mut impl PrecompileHandle,
		owner: H256,
		permission_id: H256,
	) -> EvmResult<Entity> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let owner: AccountIdOf<Runtime> = AccountIdOf::<Runtime>::from(owner.to_fixed_bytes());
		let permission_id: EntityIdOf<Runtime> =
			EntityIdOf::<Runtime>::from(permission_id.to_fixed_bytes());

		match peaq_pallet_rbac::Pallet::<Runtime>::get_permission(&owner, permission_id) {
			Err(_e) => Err(Revert::new(RevertReason::custom(err2str(&_e))).into()),
			Ok(v) => Ok(Entity { id: v.id.into(), name: v.name.into(), enabled: v.enabled }),
		}
	}

	#[precompile::public("fetch_permissions(bytes32)")]
	#[precompile::view]
	fn fetch_permissions(
		handle: &mut impl PrecompileHandle,
		owner: H256,
	) -> EvmResult<Vec<Entity>> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let owner: AccountIdOf<Runtime> = AccountIdOf::<Runtime>::from(owner.to_fixed_bytes());

		let result = match peaq_pallet_rbac::Pallet::<Runtime>::get_permissions(&owner) {
			Err(_e) => Err(Revert::new(RevertReason::custom(err2str(&_e))).into()),
			Ok(v) => Ok(v
				.iter()
				.map(|entity| Entity {
					id: entity.id.into(),
					name: entity.name.clone().into(),
					enabled: entity.enabled,
				})
				.collect::<Vec<Entity>>()),
		};

		result
	}

	#[precompile::public("add_permission(bytes32,bytes)")]
	fn add_permission(
		handle: &mut impl PrecompileHandle,
		permission_id: H256,
		name: BoundedBytes<GetBytesLimit>,
	) -> EvmResult<bool> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let caller_addr: AccountIdOf<Runtime> =
			Runtime::AddressMapping::into_account_id(handle.context().caller);

		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(caller_addr).into(),
			peaq_pallet_rbac::Call::<Runtime>::add_permission {
				permission_id: EntityIdOf::<Runtime>::from(permission_id.to_fixed_bytes()),
				name: name.clone().into(),
			},
			0,
		)?;

		let event = log1(
			handle.context().address,
			SELECTOR_LOG_ADD_PERMISSION,
			solidity::encode_event_data((
				Address::from(handle.context().caller),
				permission_id,
				name,
			)),
		);
		event.record(handle)?;

		Ok(true)
	}

	#[precompile::public("update_permission(bytes32,bytes)")]
	fn update_permission(
		handle: &mut impl PrecompileHandle,
		permission_id: H256,
		name: BoundedBytes<GetBytesLimit>,
	) -> EvmResult<bool> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let caller_addr: AccountIdOf<Runtime> =
			Runtime::AddressMapping::into_account_id(handle.context().caller);
		let permission_id: EntityIdOf<Runtime> =
			EntityIdOf::<Runtime>::from(permission_id.to_fixed_bytes());

		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(caller_addr).into(),
			peaq_pallet_rbac::Call::<Runtime>::update_permission {
				permission_id,
				name: name.clone().into(),
			},
			0,
		)?;

		let event = log1(
			handle.context().address,
			SELECTOR_LOG_UPDATE_PERMISSION,
			solidity::encode_event_data((
				Address::from(handle.context().caller),
				H256::from(permission_id),
				name,
			)),
		);
		event.record(handle)?;

		Ok(true)
	}

	#[precompile::public("disable_permission(bytes32)")]
	fn disable_permission(
		handle: &mut impl PrecompileHandle,
		permission_id: H256,
	) -> EvmResult<bool> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let caller_addr: AccountIdOf<Runtime> =
			Runtime::AddressMapping::into_account_id(handle.context().caller);

		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(caller_addr).into(),
			peaq_pallet_rbac::Call::<Runtime>::disable_permission {
				permission_id: EntityIdOf::<Runtime>::from(permission_id.to_fixed_bytes()),
			},
			0,
		)?;

		let event = log1(
			handle.context().address,
			SELECTOR_LOG_DISABLE_PERMISSION,
			solidity::encode_event_data((Address::from(handle.context().caller), permission_id)),
		);
		event.record(handle)?;

		Ok(true)
	}

	#[precompile::public("fetch_role_permissions(bytes32,bytes32)")]
	#[precompile::view]
	fn fetch_role_permissions(
		handle: &mut impl PrecompileHandle,
		owner: H256,
		role_id: H256,
	) -> EvmResult<Vec<Permission2Role>> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let owner: AccountIdOf<Runtime> = AccountIdOf::<Runtime>::from(owner.to_fixed_bytes());
		let role_id: EntityIdOf<Runtime> = EntityIdOf::<Runtime>::from(role_id.to_fixed_bytes());

		let result =
			match peaq_pallet_rbac::Pallet::<Runtime>::get_role_permissions(&owner, role_id) {
				Err(_e) => Err(Revert::new(RevertReason::custom(err2str(&_e))).into()),
				Ok(v) => Ok(v
					.iter()
					.map(|entity| Permission2Role {
						permission: entity.permission.into(),
						role: entity.role.into(),
					})
					.collect::<Vec<Permission2Role>>()),
			};

		result
	}

	#[precompile::public("assign_permission_to_role(bytes32,bytes32)")]
	fn assign_permission_to_role(
		handle: &mut impl PrecompileHandle,
		permission_id: H256,
		role_id: H256,
	) -> EvmResult<bool> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let caller_addr: AccountIdOf<Runtime> =
			Runtime::AddressMapping::into_account_id(handle.context().caller);

		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(caller_addr).into(),
			peaq_pallet_rbac::Call::<Runtime>::assign_permission_to_role {
				permission_id: EntityIdOf::<Runtime>::from(permission_id.to_fixed_bytes()),
				role_id: EntityIdOf::<Runtime>::from(role_id.to_fixed_bytes()),
			},
			0,
		)?;

		let event = log1(
			handle.context().address,
			SELECTOR_LOG_ASSIGN_PERMISSION_TO_ROLE,
			solidity::encode_event_data((
				Address::from(handle.context().caller),
				permission_id,
				role_id,
			)),
		);
		event.record(handle)?;

		Ok(true)
	}

	#[precompile::public("unassign_permission_to_role(bytes32,bytes32)")]
	fn unassign_permission_to_role(
		handle: &mut impl PrecompileHandle,
		permission_id: H256,
		role_id: H256,
	) -> EvmResult<bool> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let caller_addr: AccountIdOf<Runtime> =
			Runtime::AddressMapping::into_account_id(handle.context().caller);

		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(caller_addr).into(),
			peaq_pallet_rbac::Call::<Runtime>::unassign_permission_to_role {
				permission_id: EntityIdOf::<Runtime>::from(permission_id.to_fixed_bytes()),
				role_id: EntityIdOf::<Runtime>::from(role_id.to_fixed_bytes()),
			},
			0,
		)?;

		let event = log1(
			handle.context().address,
			SELECTOR_LOG_UNASSIGN_PERMISSION_TO_ROLE,
			solidity::encode_event_data((
				Address::from(handle.context().caller),
				permission_id,
				role_id,
			)),
		);
		event.record(handle)?;

		Ok(true)
	}

	#[precompile::public("fetch_group(bytes32,bytes32)")]
	#[precompile::view]
	fn fetch_group(
		handle: &mut impl PrecompileHandle,
		owner: H256,
		group_id: H256,
	) -> EvmResult<Entity> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let owner: AccountIdOf<Runtime> = AccountIdOf::<Runtime>::from(owner.to_fixed_bytes());
		let group_id: EntityIdOf<Runtime> = EntityIdOf::<Runtime>::from(group_id.to_fixed_bytes());

		match peaq_pallet_rbac::Pallet::<Runtime>::get_group(&owner, group_id) {
			Err(_e) => Err(Revert::new(RevertReason::custom(err2str(&_e))).into()),
			Ok(v) => Ok(Entity { id: v.id.into(), name: v.name.into(), enabled: v.enabled }),
		}
	}

	#[precompile::public("add_group(bytes32,bytes)")]
	fn add_group(
		handle: &mut impl PrecompileHandle,
		group_id: H256,
		name: BoundedBytes<GetBytesLimit>,
	) -> EvmResult<bool> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let caller_addr: AccountIdOf<Runtime> =
			Runtime::AddressMapping::into_account_id(handle.context().caller);

		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(caller_addr).into(),
			peaq_pallet_rbac::Call::<Runtime>::add_group {
				group_id: EntityIdOf::<Runtime>::from(group_id.to_fixed_bytes()),
				name: name.clone().into(),
			},
			0,
		)?;

		let event = log1(
			handle.context().address,
			SELECTOR_LOG_ADD_GROUP,
			solidity::encode_event_data((Address::from(handle.context().caller), group_id, name)),
		);
		event.record(handle)?;

		Ok(true)
	}

	#[precompile::public("update_group(bytes32,bytes)")]
	fn update_group(
		handle: &mut impl PrecompileHandle,
		group_id: H256,
		name: BoundedBytes<GetBytesLimit>,
	) -> EvmResult<bool> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let caller_addr: AccountIdOf<Runtime> =
			Runtime::AddressMapping::into_account_id(handle.context().caller);

		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(caller_addr).into(),
			peaq_pallet_rbac::Call::<Runtime>::update_group {
				group_id: EntityIdOf::<Runtime>::from(group_id.to_fixed_bytes()),
				name: name.clone().into(),
			},
			0,
		)?;

		let event = log1(
			handle.context().address,
			SELECTOR_LOG_UPDATE_GROUP,
			solidity::encode_event_data((Address::from(handle.context().caller), group_id, name)),
		);
		event.record(handle)?;

		Ok(true)
	}

	#[precompile::public("disable_group(bytes32)")]
	fn disable_group(handle: &mut impl PrecompileHandle, group_id: H256) -> EvmResult<bool> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let caller_addr: AccountIdOf<Runtime> =
			Runtime::AddressMapping::into_account_id(handle.context().caller);

		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(caller_addr).into(),
			peaq_pallet_rbac::Call::<Runtime>::disable_group {
				group_id: EntityIdOf::<Runtime>::from(group_id.to_fixed_bytes()),
			},
			0,
		)?;

		let event = log1(
			handle.context().address,
			SELECTOR_LOG_DISABLE_GROUP,
			solidity::encode_event_data((Address::from(handle.context().caller), group_id)),
		);
		event.record(handle)?;

		Ok(true)
	}

	#[precompile::public("assign_role_to_group(bytes32,bytes32)")]
	fn assign_role_to_group(
		handle: &mut impl PrecompileHandle,
		role_id: H256,
		group_id: H256,
	) -> EvmResult<bool> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let caller_addr: AccountIdOf<Runtime> =
			Runtime::AddressMapping::into_account_id(handle.context().caller);

		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(caller_addr).into(),
			peaq_pallet_rbac::Call::<Runtime>::assign_role_to_group {
				role_id: EntityIdOf::<Runtime>::from(role_id.to_fixed_bytes()),
				group_id: EntityIdOf::<Runtime>::from(group_id.to_fixed_bytes()),
			},
			0,
		)?;

		let event = log1(
			handle.context().address,
			SELECTOR_LOG_ASSIGN_ROLE_TO_GROUP,
			solidity::encode_event_data((
				Address::from(handle.context().caller),
				role_id,
				group_id,
			)),
		);
		event.record(handle)?;

		Ok(true)
	}

	#[precompile::public("unassign_role_to_group(bytes32,bytes32)")]
	fn unassign_role_to_group(
		handle: &mut impl PrecompileHandle,
		role_id: H256,
		group_id: H256,
	) -> EvmResult<bool> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let caller_addr: AccountIdOf<Runtime> =
			Runtime::AddressMapping::into_account_id(handle.context().caller);

		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(caller_addr).into(),
			peaq_pallet_rbac::Call::<Runtime>::unassign_role_to_group {
				role_id: EntityIdOf::<Runtime>::from(role_id.to_fixed_bytes()),
				group_id: EntityIdOf::<Runtime>::from(group_id.to_fixed_bytes()),
			},
			0,
		)?;

		let event = log1(
			handle.context().address,
			SELECTOR_LOG_UNASSIGN_ROLE_TO_GROUP,
			solidity::encode_event_data((
				Address::from(handle.context().caller),
				role_id,
				group_id,
			)),
		);
		event.record(handle)?;

		Ok(true)
	}

	#[precompile::public("fetch_group_roles(bytes32,bytes32)")]
	#[precompile::view]
	fn fetch_group_roles(
		handle: &mut impl PrecompileHandle,
		owner: H256,
		group_id: H256,
	) -> EvmResult<Vec<Role2Group>> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let owner: AccountIdOf<Runtime> = AccountIdOf::<Runtime>::from(owner.to_fixed_bytes());
		let group_id: EntityIdOf<Runtime> = EntityIdOf::<Runtime>::from(group_id.to_fixed_bytes());

		let result = match peaq_pallet_rbac::Pallet::<Runtime>::get_group_roles(&owner, group_id) {
			Err(_e) => Err(Revert::new(RevertReason::custom(err2str(&_e))).into()),
			Ok(v) => Ok(v
				.iter()
				.map(|val| Role2Group { role: val.role.into(), group: val.group.into() })
				.collect::<Vec<Role2Group>>()),
		};

		result
	}

	#[precompile::public("assign_user_to_group(bytes32,bytes32)")]
	fn assign_user_to_group(
		handle: &mut impl PrecompileHandle,
		user_id: H256,
		group_id: H256,
	) -> EvmResult<bool> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let caller_addr: AccountIdOf<Runtime> =
			Runtime::AddressMapping::into_account_id(handle.context().caller);

		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(caller_addr).into(),
			peaq_pallet_rbac::Call::<Runtime>::assign_user_to_group {
				user_id: EntityIdOf::<Runtime>::from(user_id.to_fixed_bytes()),
				group_id: EntityIdOf::<Runtime>::from(group_id.to_fixed_bytes()),
			},
			0,
		)?;

		let event = log1(
			handle.context().address,
			SELECTOR_LOG_ASSIGN_USER_TO_GROUP,
			solidity::encode_event_data((
				Address::from(handle.context().caller),
				user_id,
				group_id,
			)),
		);
		event.record(handle)?;

		Ok(true)
	}

	#[precompile::public("unassign_user_to_group(bytes32,bytes32)")]
	fn unassign_user_to_group(
		handle: &mut impl PrecompileHandle,
		user_id: H256,
		group_id: H256,
	) -> EvmResult<bool> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let caller_addr: AccountIdOf<Runtime> =
			Runtime::AddressMapping::into_account_id(handle.context().caller);

		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(caller_addr).into(),
			peaq_pallet_rbac::Call::<Runtime>::unassign_user_to_group {
				user_id: EntityIdOf::<Runtime>::from(user_id.to_fixed_bytes()),
				group_id: EntityIdOf::<Runtime>::from(group_id.to_fixed_bytes()),
			},
			0,
		)?;

		let event = log1(
			handle.context().address,
			SELECTOR_LOG_UNASSIGN_USER_TO_GROUP,
			solidity::encode_event_data((
				Address::from(handle.context().caller),
				user_id,
				group_id,
			)),
		);
		event.record(handle)?;

		Ok(true)
	}

	#[precompile::public("fetch_user_groups(bytes32,bytes32)")]
	#[precompile::view]
	fn fetch_user_groups(
		handle: &mut impl PrecompileHandle,
		owner: H256,
		user_id: H256,
	) -> EvmResult<Vec<User2Group>> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let owner: AccountIdOf<Runtime> = AccountIdOf::<Runtime>::from(owner.to_fixed_bytes());
		let user_id: EntityIdOf<Runtime> = EntityIdOf::<Runtime>::from(user_id.to_fixed_bytes());

		let result = match peaq_pallet_rbac::Pallet::<Runtime>::get_user_groups(&owner, user_id) {
			Err(_e) => Err(Revert::new(RevertReason::custom(err2str(&_e))).into()),
			Ok(v) => Ok(v
				.iter()
				.map(|val| User2Group { user: val.user.into(), group: val.group.into() })
				.collect::<Vec<User2Group>>()),
		};

		result
	}

	#[precompile::public("fetch_user_permissions(bytes32,bytes32)")]
	#[precompile::view]
	fn fetch_user_permissions(
		handle: &mut impl PrecompileHandle,
		owner: H256,
		user_id: H256,
	) -> EvmResult<Vec<Entity>> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let owner: AccountIdOf<Runtime> = AccountIdOf::<Runtime>::from(owner.to_fixed_bytes());
		let user_id: EntityIdOf<Runtime> = EntityIdOf::<Runtime>::from(user_id.to_fixed_bytes());

		let result =
			match peaq_pallet_rbac::Pallet::<Runtime>::get_user_permissions(&owner, user_id) {
				Err(_e) => Err(Revert::new(RevertReason::custom(err2str(&_e))).into()),
				Ok(v) => Ok(v
					.iter()
					.map(|val| Entity {
						id: val.id.into(),
						name: val.name.clone().into(),
						enabled: val.enabled,
					})
					.collect::<Vec<Entity>>()),
			};

		result
	}

	#[precompile::public("fetch_group_permissions(bytes32,bytes32)")]
	#[precompile::view]
	fn fetch_group_permissions(
		handle: &mut impl PrecompileHandle,
		owner: H256,
		group_id: H256,
	) -> EvmResult<Vec<Entity>> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let owner: AccountIdOf<Runtime> = AccountIdOf::<Runtime>::from(owner.to_fixed_bytes());
		let group_id: EntityIdOf<Runtime> = EntityIdOf::<Runtime>::from(group_id.to_fixed_bytes());

		let result =
			match peaq_pallet_rbac::Pallet::<Runtime>::get_group_permissions(&owner, group_id) {
				Err(_e) => Err(Revert::new(RevertReason::custom(err2str(&_e))).into()),
				Ok(v) => Ok(v
					.iter()
					.map(|val| Entity {
						id: val.id.into(),
						name: val.name.clone().into(),
						enabled: val.enabled,
					})
					.collect::<Vec<Entity>>()),
			};

		result
	}
}
