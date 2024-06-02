#![cfg_attr(not(feature = "std"), no_std)]
pub use pallet::*;

// #[cfg(test)]
// mod mocks;
// #[cfg(test)]
// mod tests;

// mod benchmarking;

mod types;

// pub mod weights;

use frame_support::{dispatch::DispatchResult, ensure};

pub use types::*;
// pub use weights::WeightInfo;

use frame_support::sp_runtime::{
	offchain::{
		http,
		storage::StorageValueRef,
		storage_lock::{BlockAndTime, StorageLock},
		Duration,
	},
	traits::BlockNumberProvider,
	transaction_validity::{
		InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction,
	},
	RuntimeDebug,
};

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	use frame_support::{
		pallet_prelude::{Get, OptionQuery, ValueQuery, *},
		sp_runtime::{
			traits::{AtLeast32BitUnsigned, One},
			Saturating,
		},
	};
	use frame_system::{
		ensure_signed,
		pallet_prelude::{BlockNumberFor, OriginFor},
	};
	use scale_info::prelude::vec::Vec;
	use serde::Deserialize;

	#[pallet::config]
	pub trait Config: frame_system::Config + scale_info::TypeInfo {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type KnowledgeBlockId: Parameter + AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen;
		// + Deserialize
		// type WeightInfo: WeightInfo;
		#[pallet::constant]
		type MaxLength: Get<u32>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	// individual block of document
	#[pallet::storage]
	#[pallet::getter(fn knowledge)]
	pub type Knowledge<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::KnowledgeBlockId,
		Blake2_128Concat,
		T::AccountId,
		Content<T, T::MaxLength>,
		OptionQuery,
	>;

	// knowledge owner's account
	#[pallet::storage]
	#[pallet::getter(fn account)]
	pub type Account<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::KnowledgeBlockId,
		u8,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn nonce)]
	/// Nonce for id of the next created asset.
	pub(super) type Nonce<T: Config> = StorageValue<_, T::KnowledgeBlockId, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Created {
			creator: T::AccountId,
			knowledge_block_id: T::KnowledgeBlockId,
			// parent: T::KnowledgeBlockId,
			// children: BoundedVec<T::KnowledgeBlockId, T::MaxLength>,
			// previous: T::KnowledgeBlockId,
			title: BoundedVec<u8, T::MaxLength>,
			text: BoundedVec<u8, T::MaxLength>,
		},
		Modified {
			creator: T::AccountId,
			owner: T::AccountId,
			knowledge_block_id: T::KnowledgeBlockId,
			title: BoundedVec<u8, T::MaxLength>,
			text: BoundedVec<u8, T::MaxLength>,
		},
		Transferred {
			from: T::AccountId,
			to: T::AccountId,
			knowledge_block_id: T::KnowledgeBlockId,
		},
		Got {
			owner: T::AccountId,
			knowledge_block_id: T::KnowledgeBlockId,
			title: BoundedVec<u8, T::MaxLength>,
			text: BoundedVec<u8, T::MaxLength>,
		},
		Owners {
			requestor: T::AccountId,
			knowledge_block_id: T::KnowledgeBlockId,
			owners: Vec<T::AccountId>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		KnowledgeNotExist,
		NotOwned,
		NoPermission,
		NoContent,
		UnknownOffchainMux,

		// Error returned when making signed transactions in off-chain worker
		NoLocalAcctForSigning,
		OffchainSignedTxError,

		// Error returned when making unsigned transactions in off-chain worker
		OffchainUnsignedTxError,

		// Error returned when making unsigned transactions with signed payloads in off-chain
		// worker
		OffchainUnsignedTxSignedPayloadError,

		// Error returned when fetching github info
		HttpFetchingError,
		DeserializeToObjError,
		DeserializeToStrError,
	}

	#[pallet::call]
	// weight(<T as Config>::WeightInfo)
	impl<T: Config> Pallet<T> {
		// create knowledge blocks with title, text
		// add parent knowledge block id, children knowledge block id, and previous knowledge block
		// id
		#[pallet::call_index(1)]
		#[pallet::weight(0)]
		pub fn create(
			origin: OriginFor<T>,
			title: BoundedVec<u8, T::MaxLength>,
			text: BoundedVec<u8, T::MaxLength>,
		) -> DispatchResult {
			let from = ensure_signed(origin)?;

			let knowledge_block_id = Self::nonce();
			Nonce::<T>::set(knowledge_block_id.saturating_add(T::KnowledgeBlockId::one()));

			let content =
				Content::new(from.clone(), knowledge_block_id, title.clone(), text.clone());

			Account::<T>::insert(from.clone(), knowledge_block_id, 1);
			Knowledge::<T>::insert(knowledge_block_id, from.clone(), content);

			Self::deposit_event(Event::<T>::Created {
				creator: from.clone(),
				knowledge_block_id,
				title: title.clone(),
				text: text.clone(),
			});
			Ok(())
		}

		// every owner of the knowledge block can modify the knowledge block they own
		#[pallet::call_index(2)]
		#[pallet::weight(0)]
		pub fn modify_content(
			origin: OriginFor<T>,
			knowledge_block_id: T::KnowledgeBlockId,
			title: BoundedVec<u8, T::MaxLength>,
			text: BoundedVec<u8, T::MaxLength>,
		) -> DispatchResult {
			let from = ensure_signed(origin)?;

			ensure!(
				Knowledge::<T>::get(knowledge_block_id, from.clone()).is_some(),
				Error::<T>::NoContent
			);

			let mut content = Knowledge::<T>::get(knowledge_block_id, from.clone())
				.ok_or(Error::<T>::NoContent)
				.unwrap();

			Knowledge::<T>::mutate(knowledge_block_id, from.clone(), |k| {
				k.as_mut().unwrap().set_title(title.clone());
				k.as_mut().unwrap().set_text(text.clone());
			});

			let content = Knowledge::<T>::get(knowledge_block_id, from.clone())
				.ok_or(Error::<T>::NoContent)
				.unwrap();

			Self::deposit_event(Event::<T>::Modified {
				creator: content.owner(),
				owner: from.clone(),
				knowledge_block_id,
				title: content.title.clone(),
				text: content.text.clone(),
			});

			Ok(())
		}

		// get content using knowledge block id
		#[pallet::call_index(3)]
		#[pallet::weight(0)]
		pub fn get_content(
			origin: OriginFor<T>,
			knowledge_block_id: T::KnowledgeBlockId,
		) -> DispatchResult {
			let from = ensure_signed(origin)?;

			ensure!(
				Knowledge::<T>::get(knowledge_block_id, from.clone()).is_some(),
				Error::<T>::NoContent
			);

			let content = Knowledge::<T>::get(knowledge_block_id, from.clone())
				.ok_or(Error::<T>::NoContent)
				.unwrap();

			Self::deposit_event(Event::<T>::Got {
				owner: from.clone(),
				knowledge_block_id,
				title: content.title,
				text: content.text,
			});

			Ok(())
		}

		// based on knowledge block id, get all the knowledge block owners
		#[pallet::call_index(4)]
		#[pallet::weight(0)]
		pub fn get_knowledge_owners(
			origin: OriginFor<T>,
			knowledge_block_id: T::KnowledgeBlockId,
		) -> DispatchResult {
			let from = ensure_signed(origin)?;

			ensure!(
				Knowledge::<T>::get(knowledge_block_id, from.clone()).is_some(),
				Error::<T>::NoContent
			);

			let owners = Knowledge::<T>::iter_prefix(&knowledge_block_id).map(|k| k.0).collect();

			Self::deposit_event(Event::<T>::Owners {
				requestor: from.clone(),
				knowledge_block_id,
				owners,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	// transfer the knowledge block to new account
	fn inner_transfer(
		knowledge_block_id: T::KnowledgeBlockId,
		from: T::AccountId,
		to: T::AccountId,
	) {
		let content = Knowledge::<T>::get(knowledge_block_id, from.clone())
			.ok_or(Error::<T>::NoContent)
			.unwrap();
		Knowledge::<T>::insert(knowledge_block_id, to.clone(), content);
		Account::<T>::insert(to.clone(), knowledge_block_id, 1);
	}
}

impl<T: Config> Sellable<T::AccountId, T::KnowledgeBlockId> for Pallet<T> {
	fn origin_owner(
		knowledge_block_id: T::KnowledgeBlockId,
		account_id: T::AccountId,
	) -> T::AccountId {
		let content = Knowledge::<T>::get(knowledge_block_id, account_id)
			.ok_or(Error::<T>::NoContent)
			.unwrap();
		content.owner()
	}

	fn transfer(knowledge_block_id: T::KnowledgeBlockId, from: T::AccountId, to: T::AccountId) {
		Self::inner_transfer(knowledge_block_id, from.clone(), to.clone());
	}
}
