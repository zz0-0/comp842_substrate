#![cfg_attr(not(feature = "std"), no_std)]
pub use pallet::*;

// #[cfg(test)]
// mod mocks;
// #[cfg(test)]
// mod tests;

// mod benchmarking;

mod types;

// pub mod weights;

// use frame_support::{dispatch::DispatchResult, ensure};

pub use types::*;
// pub use weights::WeightInfo;

#[frame_support::pallet]

pub mod pallet {
	use super::*;

	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	// use frame_support::{
	// 	pallet_prelude::{Get, OptionQuery, ValueQuery, *},
	// 	sp_runtime::{
	// 		traits::{AtLeast32BitUnsigned, One},
	// 		Saturating,
	// 	},
	// };
	// use frame_system::{ensure_signed, pallet_prelude::OriginFor};
	use pallet_tds_ipfs::Commands;
	use pallet_tds_ipfs_core::{
		generate_id, storage::store_cid_data_for_values, CommandRequest, IpfsCommand,
	};
	// use scale_info::prelude::vec::Vec;
	use pallet_tds_ipfs_core::types::IpfsFile;
	use sp_std::{str, vec, vec::Vec};

	#[pallet::config]
	pub trait Config:
		frame_system::Config
		// + scale_info::TypeInfo
		+ pallet_tds_ipfs_core::Config
		+ pallet_tds_ipfs::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		// type KnowledgeBlockId: Parameter + AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen;
		// + Deserialize
		// type WeightInfo: WeightInfo;
		// #[pallet::constant]
		// type MaxLength: Get<u32>;
	}

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	// individual block of document
	// #[pallet::storage]
	// #[pallet::getter(fn knowledge)]
	// pub type Knowledge<T: Config> = StorageDoubleMap<
	// 	_,
	// 	Blake2_128Concat,
	// 	T::BlockNumber,
	// 	Blake2_128Concat,
	// 	T::AccountId,
	// 	Content<T, T::MaxLength>,
	// 	OptionQuery,
	// >;

	#[pallet::storage]
	#[pallet::getter(fn ipfs_files)]
	pub(super) type IpfsFileStorage<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		Vec<u8>, // key is cid
		IpfsFile,
		ValueQuery,
	>;

	// knowledge owner's account
	#[pallet::storage]
	#[pallet::getter(fn account)]
	pub type Account<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::BlockNumber,
		u8,
		OptionQuery,
	>;

	// #[pallet::storage]
	// #[pallet::getter(fn nonce)]
	// /// Nonce for id of the next created asset.
	// pub(super) type Nonce<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Created {
			creator: T::AccountId,
			block_number: T::BlockNumber,
			// parent: T::BlockNumber,
			// children: BoundedVec<T::BlockNumber, T::MaxLength>,
			// previous: T::BlockNumber,
			title: Vec<u8>,
			text: Vec<u8>,
		},
		Modified {
			creator: T::AccountId,
			owner: T::AccountId,
			block_number: T::BlockNumber,
			title: Vec<u8>,
			text: Vec<u8>,
		},
		Transferred {
			from: T::AccountId,
			to: T::AccountId,
			block_number: T::BlockNumber,
		},
		Got {
			owner: T::AccountId,
			block_number: T::BlockNumber,
			title: Vec<u8>,
			text: Vec<u8>,
		},
		Owners {
			requestor: T::AccountId,
			block_number: T::BlockNumber,
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
		pub fn create(origin: OriginFor<T>, title: Vec<u8>, text: Vec<u8>) -> DispatchResult {
			// let from = ensure_signed(origin)?;

			// let block_number = Self::nonce();
			// Nonce::<T>::set(block_number.saturating_add(T::BlockNumber::one()));

			// let content =
			// 	Content::new(from.clone(), block_number, title.clone(), text.clone());

			// Account::<T>::insert(from.clone(), block_number, 1);
			// Knowledge::<T>::insert(block_number, from.clone(), content);

			// Self::deposit_event(Event::<T>::Created {
			// 	creator: from.clone(),
			// 	block_number,
			// 	title: title.clone(),
			// 	text: text.clone(),
			// });
			// Ok(())

			let from = ensure_signed(origin)?;

			let block_number = frame_system::Pallet::<T>::block_number();

			Account::<T>::insert(from.clone(), block_number, 1);

			store_cid_data_for_values::<T>(
				block_number,
				text.clone().into(),
				title.clone().into(),
				false,
			);

			let mut commands = Vec::<IpfsCommand>::new();
			commands.push(IpfsCommand::AddBytes(1));

			let ipfs_command_request = CommandRequest::<T> {
				identifier: generate_id::<T>(),
				requester: from.clone(),
				ipfs_commands: commands,
			};

			Commands::<T>::mutate(|commands| {
				if let Some(ref mut commands) = commands {
					commands.push(ipfs_command_request);
				} else {
					*commands = Some(vec![ipfs_command_request]);
				}
			});

			// Emit the event
			Self::deposit_event(Event::<T>::Created {
				creator: from.clone(),
				block_number,
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
			block_number: T::BlockNumber,
			title: Vec<u8>,
			text: Vec<u8>,
		) -> DispatchResult {
			// let from = ensure_signed(origin)?;

			// ensure!(
			// 	Knowledge::<T>::get(block_number, from.clone()).is_some(),
			// 	Error::<T>::NoContent
			// );

			// let mut content = Knowledge::<T>::get(block_number, from.clone())
			// 	.ok_or(Error::<T>::NoContent)
			// 	.unwrap();

			// Knowledge::<T>::mutate(block_number, from.clone(), |k| {
			// 	k.as_mut().unwrap().set_title(title.clone());
			// 	k.as_mut().unwrap().set_text(text.clone());
			// });

			// let content = Knowledge::<T>::get(block_number, from.clone())
			// 	.ok_or(Error::<T>::NoContent)
			// 	.unwrap();

			// Self::deposit_event(Event::<T>::Modified {
			// 	creator: content.owner(),
			// 	owner: from.clone(),
			// 	block_number,
			// 	title: content.title.clone(),
			// 	text: content.text.clone(),
			// });

			// Ok(())

			let from = ensure_signed(origin)?;

			Ok(())
		}

		// get content using knowledge block id
		// #[pallet::call_index(3)]
		// #[pallet::weight(0)]
		// pub fn get_content(origin: OriginFor<T>, block_number: T::BlockNumber) -> DispatchResult
		// { 	let from = ensure_signed(origin)?;

		// 	ensure!(
		// 		Knowledge::<T>::get(block_number, from.clone()).is_some(),
		// 		Error::<T>::NoContent
		// 	);

		// 	let content = Knowledge::<T>::get(block_number, from.clone())
		// 		.ok_or(Error::<T>::NoContent)
		// 		.unwrap();

		// 	Self::deposit_event(Event::<T>::Got {
		// 		owner: from.clone(),
		// 		block_number,
		// 		title: content.title,
		// 		text: content.text,
		// 	});

		// 	Ok(())
		// }

		// based on knowledge block id, get all the knowledge block owners
		// #[pallet::call_index(4)]
		// #[pallet::weight(0)]
		// pub fn get_knowledge_owners(
		// 	origin: OriginFor<T>,
		// 	block_number: T::BlockNumber,
		// ) -> DispatchResult {
		// 	let from = ensure_signed(origin)?;

		// 	ensure!(
		// 		Knowledge::<T>::get(block_number, from.clone()).is_some(),
		// 		Error::<T>::NoContent
		// 	);

		// 	let owners = Knowledge::<T>::iter_prefix(&block_number).map(|k| k.0).collect();

		// 	Self::deposit_event(Event::<T>::Owners {
		// 		requestor: from.clone(),
		// 		block_number,
		// 		owners,
		// 	});

		// 	Ok(())
		// }
	}
}

use sp_std::{str, vec, vec::Vec};

impl<T: Config> Pallet<T> {
	// transfer the knowledge block to new account
	fn inner_transfer(block_number: T::BlockNumber, from: T::AccountId, to: T::AccountId) {
		// let content = Knowledge::<T>::get(block_number, from.clone())
		// 	.ok_or(Error::<T>::NoContent)
		// 	.unwrap();
		// Knowledge::<T>::insert(block_number, to.clone(), content);
		Account::<T>::insert(to.clone(), block_number, 1);
	}

	pub fn get_file_url_for_meta_data(meta_data: Vec<u8>) -> Vec<u8> {
		let mut ret_val = Vec::<u8>::new();

		if let Some(file) =
			IpfsFileStorage::<T>::iter_values().find(|curr_file| curr_file.meta_data == meta_data)
		{
			ret_val = Self::ipfs_gateway_url_for_cid(&file.cid)
		}

		return ret_val
	}

	pub fn get_file_url_for_cid(cid_bytes: Vec<u8>) -> Vec<u8> {
		let entry = IpfsFileStorage::<T>::try_get(cid_bytes.clone());

		let ret_val = match entry {
			Err(_) => Vec::<u8>::new(),
			Ok(_) => Self::ipfs_gateway_url_for_cid(&cid_bytes),
		};

		return ret_val
	}

	fn ipfs_gateway_url_for_cid(cid: &Vec<u8>) -> Vec<u8> {
		let mut ret_val = b"https://ipfs.io/ipfs/".to_vec();
		ret_val.append(&mut cid.clone());

		return ret_val
	}
}

impl<T: Config> Sellable<T::AccountId, T::BlockNumber> for Pallet<T> {
	// fn origin_owner(block_number: T::BlockNumber, account_id: T::AccountId) -> T::AccountId {
	// 	let content = Knowledge::<T>::get(block_number, account_id)
	// 		.ok_or(Error::<T>::NoContent)
	// 		.unwrap();
	// 	content.owner()
	// }

	fn transfer(block_number: T::BlockNumber, from: T::AccountId, to: T::AccountId) {
		Self::inner_transfer(block_number, from.clone(), to.clone());
	}
}
