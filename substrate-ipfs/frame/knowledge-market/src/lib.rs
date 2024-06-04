#![cfg_attr(not(feature = "std"), no_std)]
pub use pallet::*;
// #[cfg(test)]
// mod tests;

// #[cfg(test)]
// mod mocks;

pub mod types;

pub use types::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		dispatch::DispatchResult,
		pallet_prelude::{ValueQuery, *},
		sp_runtime::traits::{
			AtLeast32BitUnsigned, CheckedAdd, CheckedConversion, CheckedDiv, CheckedMul,
		},
		traits::{Currency, ExistenceRequirement::KeepAlive},
	};
	use frame_system::{ensure_signed, pallet_prelude::OriginFor};
	use pallet_knowledge::Sellable;

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	#[pallet::config]
	pub trait Config: frame_system::Config + scale_info::TypeInfo {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type ResourceId: Parameter + AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen;
		type Resource: Sellable<Self::AccountId, Self::ResourceId>;
		type Currency: Currency<Self::AccountId>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		ListForSale(T::ResourceId, T::AccountId, BalanceOf<T>),
		Sold(T::ResourceId, T::AccountId, T::AccountId),
		PermissionSet { owner: T::AccountId, knowledge_block_id: T::ResourceId, permission: Permit },
	}

	#[pallet::error]
	pub enum Error<T> {
		PermissionNotSet,
		PermissionOwnerSetNotSale,
		NotOriginOwner,
		Overflow,
	}

	#[pallet::storage]
	pub type KnowledgeBlockForSale<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::ResourceId,
		Blake2_128Concat,
		T::AccountId,
		SaleData<T>,
		ValueQuery,
	>;

	// permission for the knowledge block
	// you can share it
	// you can edit it
	// you can sell it
	#[pallet::storage]
	#[pallet::getter(fn permission)]
	pub type Permission<T: Config> =
		StorageMap<_, Blake2_128Concat, T::ResourceId, Permit, OptionQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		// sale the knowledge block
		#[pallet::call_index(0)]
		#[pallet::weight(0)]
		pub fn set_sale(
			origin: OriginFor<T>,
			knowledge_block_id: T::ResourceId,
			price: BalanceOf<T>,
		) -> DispatchResult {
			let from = ensure_signed(origin)?;

			// ensure the block has permissions when it's on sale
			ensure!(
				Permission::<T>::get(knowledge_block_id).is_some(),
				Error::<T>::PermissionNotSet
			);

			let result = Permission::<T>::get(knowledge_block_id);

			let permission = result.unwrap();

			ensure!(permission.sellable == true, Error::<T>::PermissionOwnerSetNotSale);

			KnowledgeBlockForSale::<T>::insert(
				knowledge_block_id,
				from.clone(),
				SaleData { price },
			);

			Self::deposit_event(Event::<T>::ListForSale(knowledge_block_id, from.clone(), price));

			Ok(())
		}

		// set permission
		// #[pallet::call_index(1)]
		// #[pallet::weight(0)]
		// pub fn set_buyer_permission(
		// 	origin: OriginFor<T>,
		// 	knowledge_block_id: T::ResourceId,
		// 	viewable: bool,
		// 	editable: bool,
		// 	sellable: bool,
		// ) -> DispatchResult {
		// 	let from = ensure_signed(origin)?;

		// 	let origin_owner = T::Resource::origin_owner(knowledge_block_id, from.clone());

		// 	ensure!(from == origin_owner, Error::<T>::NotOriginOwner);

		// 	let permit = Permit::new(viewable, editable, sellable);
		// 	Permission::<T>::insert(knowledge_block_id, permit.clone());
		// 	Self::deposit_event(Event::<T>::PermissionSet {
		// 		owner: from.clone(),
		// 		knowledge_block_id,
		// 		permission: permit.clone(),
		// 	});
		// 	Ok(())
		// }

		// buy the knowledge block
		// #[pallet::call_index(2)]
		// #[pallet::weight(0)]
		// pub fn buy(
		// 	origin: OriginFor<T>,
		// 	seller: T::AccountId,
		// 	knowledge_block_id: T::ResourceId,
		// ) -> DispatchResult {
		// 	let buyer = ensure_signed(origin)?;

		// 	let result =
		// 		Permission::<T>::get(knowledge_block_id).ok_or(Error::<T>::PermissionNotSet);

		// 	let permission = result.unwrap();

		// 	let sale_data = KnowledgeBlockForSale::<T>::get(knowledge_block_id, seller.clone());
		// 	let pay = sale_data.price;

		// 	// for different level of permissions, the price can also be set according
		// 	if permission.viewable {
		// 		pay.checked_add(&100_u32.checked_into().ok_or(Error::<T>::Overflow)?);
		// 	}

		// 	if permission.editable {
		// 		pay.checked_add(&100_u32.checked_into().ok_or(Error::<T>::Overflow)?);
		// 	}

		// 	if permission.sellable {
		// 		pay.checked_add(&100_u32.checked_into().ok_or(Error::<T>::Overflow)?);
		// 	}

		// 	// make royalties to original owner of the knowledge, pay 90% to the person you are
		// 	// buying from, pay 10% to original owner
		// 	let pay90 = pay
		// 		.checked_mul(&9_u32.checked_into().ok_or(Error::<T>::Overflow)?)
		// 		.ok_or(Error::<T>::Overflow)?;
		// 	let pay90 = pay90
		// 		.checked_div(&10_u32.checked_into().ok_or(Error::<T>::Overflow)?)
		// 		.ok_or(Error::<T>::Overflow)?;
		// 	let pay10 = pay
		// 		.checked_mul(&1_u32.checked_into().ok_or(Error::<T>::Overflow)?)
		// 		.ok_or(Error::<T>::Overflow)?;
		// 	let pay10 = pay10
		// 		.checked_div(&10_u32.checked_into().ok_or(Error::<T>::Overflow)?)
		// 		.ok_or(Error::<T>::Overflow)?;

		// 	T::Currency::transfer(&buyer, &seller, pay90, KeepAlive)?;

		// 	let origin_owner = T::Resource::origin_owner(knowledge_block_id, seller.clone());

		// 	T::Currency::transfer(&buyer, &origin_owner, pay10, KeepAlive)?;
		// 	// transfer the knowledge to buyers

		// 	let _ = T::Resource::transfer(knowledge_block_id, seller.clone(), buyer.clone());

		// 	Self::deposit_event(Event::<T>::Sold(
		// 		knowledge_block_id,
		// 		buyer.clone(),
		// 		seller.clone(),
		// 	));

		// 	Ok(())
		// }
	}
}
