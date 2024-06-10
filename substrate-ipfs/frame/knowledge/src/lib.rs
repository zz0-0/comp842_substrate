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

pub mod crypto {
	use sp_core::sr25519::Signature as Sr25519Signature;
	use sp_runtime::{
		app_crypto::{app_crypto, sr25519},
		traits::Verify,
		MultiSignature, MultiSigner,
	};

	app_crypto!(sr25519, sp_core::crypto::key_types::IPFS);
	pub struct TestAuthId;

	impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
		type RuntimeAppPublic = Public;
		type GenericPublic = sr25519::Public;
		type GenericSignature = sr25519::Signature;
	}

	// Implemented for mock runtime in tests
	impl frame_system::offchain::AppCrypto<<Sr25519Signature as Verify>::Signer, Sr25519Signature>
		for TestAuthId
	{
		type RuntimeAppPublic = Public;
		type GenericPublic = sr25519::Public;
		type GenericSignature = sr25519::Signature;
	}
}

#[frame_support::pallet]

pub mod pallet {
	use super::*;

	use frame_support::pallet_prelude::*;
	use frame_system::{offchain::CreateSignedTransaction, pallet_prelude::*};

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
		generate_id,
		storage::{
			offchain_storage_data_for_key, read_cid_for_block_number, store_cid_data_for_values,
		},
		CommandRequest, IpfsCommand,
	};
	// use scale_info::prelude::vec::Vec;
	use pallet_tds_ipfs_core::types::IpfsFile;
	use sp_std::{str, vec, vec::Vec};

	use frame_system::offchain::{AppCrypto, Signer, SubmitTransaction};
	use pallet_tds_ipfs_core::storage::offchain_data_key;
	use sp_core::crypto::KeyTypeId;
	use sp_io::offchain_index;
	use sp_runtime::offchain::storage::StorageValueRef;

	pub const KEY_TYPE: KeyTypeId = sp_core::crypto::key_types::IPFS;
	pub const PROCESSED_COMMANDS: &[u8; 24] = b"ipfs::processed_commands";

	#[pallet::config]
	pub trait Config:
		frame_system::Config
		// + scale_info::TypeInfo
		+ pallet_tds_ipfs_core::Config
		+ pallet_tds_ipfs::Config
		+ CreateSignedTransaction<Call<Self>>
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type AuthorityId: AppCrypto<Self::Public, Self::Signature>;
		// type Call: From<Call<Self>>;
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
	#[pallet::storage]
	#[pallet::getter(fn knowledge)]
	pub type Knowledge<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::BlockNumber,
		Blake2_128Concat,
		T::AccountId,
		// Content<T, T::MaxLength>,
		Vec<T::BlockNumber>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn ipfs_files)]
	pub(super) type IpfsFileStorage<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		Vec<u8>, // key is cid
		IpfsFile,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn ipfs_data)]
	pub type IpfsData<T: Config> =
		StorageMap<_, Blake2_128Concat, T::BlockNumber, OffchainData, OptionQuery>;

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

		ConnectionRequested(T::AccountId),
		DisconnectedRequested(T::AccountId),
		QueuedDataToAdd(T::AccountId, T::AccountId),
		QueuedDataToCat(T::AccountId),
		QueuedDataToPin(T::AccountId),
		QueuedDataToRemove(T::AccountId),
		QueuedDataToUnpin(T::AccountId),
		FindPeerIssued(T::AccountId),
		FindProvidersIssued(T::AccountId),
		OcwCallback(T::AccountId),

		// Requester, IpfsConnectionAddress
		ConnectedTo(T::AccountId, Vec<u8>),
		// Requester, IpfsDisconnectionAddress
		DisconnectedFrom(T::AccountId, Vec<u8>),
		// Requester, Cid
		AddedCid(T::AccountId, Vec<u8>),
		// Requester, Cid, Bytes
		CatBytes(T::AccountId, Vec<u8>, Vec<u8>),
		// Requester, Cid
		InsertedPin(T::AccountId, Vec<u8>),
		// Requester, Cid
		RemovedPin(T::AccountId, Vec<u8>),
		// Requester, Cid
		RemovedBlock(T::AccountId, Vec<u8>),
		// Requester, ListOfPeers
		FoundPeers(T::AccountId, Vec<u8>),
		// Requester, Cid, Providers
		CidProviders(T::AccountId, Vec<u8>, Vec<u8>),

		OffchainRequest {
			block_number: T::BlockNumber,
		},
		DataStored {
			block_number: T::BlockNumber,
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

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn offchain_worker(block_number: T::BlockNumber) {
			let key = offchain_data_key::<T>(block_number);
			let storage_ref = StorageValueRef::persistent(&key);

			if let Ok(Some(block_number)) = storage_ref.get::<T::BlockNumber>() {
				if let Ok(cid_data) = read_cid_for_block_number::<T>(block_number) {
					log::info!(
						"Off-chain data: title = {:?}, text = {:?}",
						cid_data.meta_data,
						cid_data.data
					);

					// Submit a signed transaction to store the data on-chain
					let call =
						Call::store_offchain_data { block_number, cid_data: cid_data.clone() };

					// if let Err(e) =
					// 	SubmitTransaction::<T, T::Call>::submit_unsigned_transaction(call.into())
					// {
					// 	log::error!("Failed to submit unsigned transaction: {:?}", e);
					// }
					SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
						.map_err(|_| {
							log::error!("Failed in offchain_unsigned_tx");
						});
				} else {
					log::error!(
						"Failed to read off-chain data for block number: {:?}",
						block_number
					);
				}
			} else {
				log::error!("No block number found in off-chain storage for key: {:?}", key);
			}
		}
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

			log::info!("Creating content with block number: {:?}", block_number);

			Account::<T>::insert(from.clone(), block_number, 1);
			Knowledge::<T>::insert(block_number, from.clone(), vec![block_number]);

			log::info!("Stored in Account: {:?}", Account::<T>::get(from.clone(), block_number));
			log::info!(
				"Stored in Knowledge: {:?}",
				Knowledge::<T>::get(block_number, from.clone())
			);

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

			let key = offchain_data_key::<T>(block_number);

			offchain_index::set(&key, &block_number.encode());

			log::info!(
				"Stored block number {:?} in off-chain storage with key {:?}",
				block_number,
				key
			);

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

			// let mut cid = Vec::<u8>::new();

			// if let Some(file) = IpfsFileStorage::<T>::iter_values()
			// 	.find(|curr_file| curr_file.meta_data == title.clone())
			// {
			// cid = file.cid;

			// Self::deposit_event(Event::<T>::Created {
			// 	creator: from.clone(),
			// 	block_number,
			// 	title: title.clone(),
			// 	text: file.cid.clone(),
			// });
			// }

			// Self::deposit_event(Event::<T>::Created {
			// 	creator: from.clone(),
			// 	block_number,
			// 	title: title.clone(),
			// 	text: cid.clone(),
			// });

			// IpfsCommand::RemovePin(cid.clone());

			// let block_number = frame_system::Pallet::<T>::block_number();

			// Account::<T>::insert(block_number, from.clone(), 1);

			let from = ensure_signed(origin)?;

			let new_block_number = frame_system::Pallet::<T>::block_number();

			store_cid_data_for_values::<T>(
				new_block_number,
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

			let mut block_numbers = Knowledge::<T>::get(block_number, from.clone())
				.ok_or(Error::<T>::NoContent)
				.unwrap();
			block_numbers.push(new_block_number.clone());

			Knowledge::<T>::mutate(block_number, from.clone(), |k| {
				// k.as_mut().unwrap().append(new_block_number.clone());
				*k = Some(block_numbers);
			});

			let data = read_cid_for_block_number::<T>(block_number)
				.map_err(|_| Error::<T>::KnowledgeNotExist)?;

			Self::deposit_event(Event::<T>::Modified {
				owner: from.clone(),
				creator: from.clone(),
				block_number,
				title: data.meta_data.clone(),
				text: data.data.clone(),
			});

			// let url = Self::get_file_url_for_meta_data(title.clone());

			Ok(())
		}

		// get content using knowledge block id
		#[pallet::call_index(3)]
		#[pallet::weight(0)]
		pub fn get_content(origin: OriginFor<T>, block_number: T::BlockNumber) -> DispatchResult {
			let from = ensure_signed(origin)?;

			// ensure!(
			// 	Knowledge::<T>::get(block_number, from.clone()).is_some(),
			// 	Error::<T>::NoContent
			// );

			// let content = Knowledge::<T>::get(block_number, from.clone())
			// 	.ok_or(Error::<T>::NoContent)
			// 	.unwrap();

			// if let Some(block_numbers) = Knowledge::<T>::get(block_number, from.clone()) {
			// 	if let Some(&latest_block_number) = block_numbers.last() {
			// 		if let Ok(data) = read_cid_for_block_number::<T>(latest_block_number) {
			// 			Self::deposit_event(Event::<T>::Got {
			// 				owner: from.clone(),
			// 				block_number,
			// 				title: data.meta_data.clone(),
			// 				text: data.data.clone(),
			// 			});
			// 			return Ok(());
			// 		}
			// 	}
			// }
			// Err(Error::<T>::KnowledgeNotExist.into())

			log::info!("Getting content with block number: {:?}", block_number);

			let key = offchain_data_key::<T>(block_number);

			offchain_index::set(&key, &block_number.encode());

			// if let Ok(data) = read_cid_for_block_number::<T>(block_number) {
			// 	Self::deposit_event(Event::<T>::Got {
			// 		owner: from.clone(),
			// 		block_number,
			// 		title: data.meta_data.clone(),
			// 		text: data.data.clone(),
			// 	});
			// 	log::info!(
			// 		"Content retrieved successfully: title = {:?}, text = {:?}",
			// 		data.meta_data,
			// 		data.data
			// 	);
			// } else {
			// 	log::error!("Content not found for block number: {:?}", block_number);
			// 	return Err(Error::<T>::KnowledgeNotExist.into())
			// }

			Ok(())
		}

		// based on knowledge block id, get all the knowledge block owners
		#[pallet::call_index(4)]
		#[pallet::weight(0)]
		pub fn get_knowledge_owners(
			origin: OriginFor<T>,
			block_number: T::BlockNumber,
		) -> DispatchResult {
			let from = ensure_signed(origin)?;

			// ensure!(
			// 	Knowledge::<T>::get(block_number, from.clone()).is_some(),
			// 	Error::<T>::NoContent
			// );

			let owners = Knowledge::<T>::iter_prefix(&block_number).map(|k| k.0).collect();

			Self::deposit_event(Event::<T>::Owners {
				requestor: from.clone(),
				block_number,
				owners,
			});

			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(0)]
		pub fn ocw_callback(
			origin: OriginFor<T>,
			identifier: [u8; 32],
			data: Vec<u8>,
			offchain_data: Option<OffchainData>,
		) -> DispatchResult {
			let signer = ensure_signed(origin)?;
			let mut callback_command: Option<CommandRequest<T>> = None;

			Commands::<T>::mutate(|command_requests| {
				let mut commands = command_requests.clone().unwrap();

				if let Some(index) = commands.iter().position(|cmd| cmd.identifier == identifier) {
					log::info!("Removing at index {}", index.clone());
					callback_command = Some(commands.swap_remove(index).clone());
				};

				*command_requests = Some(commands);
			});

			Self::deposit_event(Event::OcwCallback(signer));
			Self::handle_data_for_ocw_callback(
				data.clone(),
				offchain_data.clone(),
				callback_command.clone(),
			);

			match Self::command_callback(&callback_command.unwrap(), data.clone()) {
				Ok(_) => Ok(()),
				Err(_) => Err(DispatchError::Corruption),
			}
		}

		#[pallet::call_index(6)]
		#[pallet::weight(0)]
		pub fn store_offchain_data(
			origin: OriginFor<T>,
			block_number: T::BlockNumber,
			cid_data: OffchainData,
		) -> DispatchResult {
			let _ = ensure_signed(origin)?;

			IpfsData::<T>::insert(block_number, cid_data);

			Self::deposit_event(Event::<T>::DataStored { block_number });

			Ok(())
		}
	}
}

use pallet_tds_ipfs::Commands;
// use pallet_tds_ipfs_core::{
// 	storage::read_cid_data_for_block_number, types::OffchainData, CommandRequest, IpfsCommand,
// };
use frame_system::offchain::{SendSignedTransaction, Signer};
use pallet_tds_ipfs_core::{
	addresses_to_utf8_safe_bytes, generate_id, ipfs_request, ocw_parse_ipfs_response,
	ocw_process_command,
	storage::{read_cid_data_for_block_number, store_cid_data_for_values},
	types::{IpfsFile, OffchainData},
	CommandRequest, Error as IpfsError, IpfsCommand, TypeEquality,
};
use sp_core::offchain::{IpfsRequest, IpfsResponse};
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

	fn handle_data_for_ocw_callback(
		data: Vec<u8>,
		offchain_data: Option<OffchainData>,
		callback_command: Option<CommandRequest<T>>,
	) {
		if let Some(cmd_request) = callback_command.clone() {
			if contains_value_of_type_in_vector(
				&IpfsCommand::AddBytes(0),
				&cmd_request.ipfs_commands,
			) {
				Self::handle_add_bytes_completed(data.clone(), offchain_data.clone())
			}
		} else {
			return
		}
	}

	fn handle_add_bytes_completed(cid: Vec<u8>, offchain_data: Option<OffchainData>) {
		if let Some(offchain_data_unwrap) = offchain_data {
			let file = IpfsFile::new(cid, offchain_data_unwrap.meta_data);
			Self::store_ipfs_file_info(file)
		}
	}

	fn store_ipfs_file_info(ipfs_file: IpfsFile) {
		IpfsFileStorage::<T>::insert(ipfs_file.cid.clone(), ipfs_file.clone())
	}

	fn ocw_process_command_requests(block_number: T::BlockNumber) -> Result<(), Error<T>> {
		let commands: Vec<CommandRequest<T>> =
			Commands::<T>::get().unwrap_or(Vec::<CommandRequest<T>>::new());

		for command_request in commands {
			if contains_value_of_type_in_vector(
				&IpfsCommand::AddBytes(0),
				&command_request.ipfs_commands,
			) {
				log::info!("IPFS CALL: ocw_process_command_requests for Add Bytes");
			}

			let offchain_data: Option<OffchainData> =
				match read_cid_data_for_block_number::<T>(block_number) {
					Ok(data) => data,
					Err(_) => None,
				};

			match ocw_process_command::<T>(
				block_number,
				command_request.clone(),
				PROCESSED_COMMANDS,
			) {
				Ok(responses) => {
					let callback_response = ocw_parse_ipfs_response::<T>(responses);
					_ = Self::signed_callback(&command_request, callback_response, offchain_data);
				},
				Err(e) => match e {
					IpfsError::<T>::RequestFailed => {
						log::error!("IPFS: failed to perform a request")
					},
					_ => {},
				},
			}
		}

		Ok(())
	}

	// Output the current state of IPFS worker
	fn print_metadata(message: &str) -> Result<(), IpfsError<T>> {
		let peers = if let IpfsResponse::Peers(peers) = ipfs_request::<T>(IpfsRequest::Peers)? {
			peers
		} else {
			Vec::new()
		};

		log::info!("{}", message);
		log::info!("IPFS: Is currently connected to {} peers", peers.len());
		if !peers.is_empty() {
			log::info!("IPFS: Peer Ids: {:?}", str::from_utf8(&addresses_to_utf8_safe_bytes(peers)))
		}

		log::info!("IPFS: CommandRequest size: {}", Commands::<T>::decode_len().unwrap_or(0));
		Ok(())
	}

	fn signed_callback(
		command_request: &CommandRequest<T>,
		data: Vec<u8>,
		offchain_data: Option<OffchainData>,
	) -> Result<(), IpfsError<T>> {
		let signer = Signer::<T, <T as pallet::Config>::AuthorityId>::all_accounts();

		if !signer.can_sign() {
			log::error!("*** IPFS *** ---- No local accounts available. Consider adding one via `author_insertKey` RPC.");
			return Err(IpfsError::<T>::RequestFailed)?
		}

		let results = signer.send_signed_transaction(|_account| Call::ocw_callback {
			identifier: command_request.identifier,
			data: data.clone(),
			offchain_data: offchain_data.clone(),
		});

		if contains_value_of_type_in_vector(
			&IpfsCommand::AddBytes(0),
			&command_request.ipfs_commands,
		) {
			log::info!("IPFS CALL: signed_callback for Add Bytes");
		}

		for (_account, result) in &results {
			match result {
				Ok(()) => {
					log::info!("callback sent")
				},
				Err(e) => {
					log::error!("Failed to submit transaction {:?}", e)
				},
			}
		}

		Ok(())
	}

	fn command_callback(command_request: &CommandRequest<T>, data: Vec<u8>) -> Result<(), ()> {
		let contains_cat_bytes = contains_value_of_type_in_vector(
			&IpfsCommand::CatBytes(Vec::<u8>::new()),
			&command_request.ipfs_commands,
		);
		let data_len_exceeded = data.len() > 20;

		if contains_cat_bytes && data_len_exceeded {
			// Avoid excessive data logging
			log::info!("Received data for cat bytes with length: {:?}", &data.len());
		} else {
			if let Ok(utf8_str) = str::from_utf8(&*data) {
				log::info!("Received string: {:?}", utf8_str);
			} else {
				log::info!("Received data: {:?}", data);
			}
		}

		for command in command_request.clone().ipfs_commands {
			match command {
				IpfsCommand::ConnectTo(address) => Self::deposit_event(Event::ConnectedTo(
					command_request.clone().requester,
					address,
				)),

				IpfsCommand::DisconnectFrom(address) => Self::deposit_event(
					Event::DisconnectedFrom(command_request.clone().requester, address),
				),
				IpfsCommand::AddBytes(_) => Self::deposit_event(Event::AddedCid(
					command_request.clone().requester,
					data.clone(),
				)),

				IpfsCommand::CatBytes(cid) => Self::deposit_event(Event::CatBytes(
					command_request.clone().requester,
					cid,
					data.clone(),
				)),

				IpfsCommand::InsertPin(cid) =>
					Self::deposit_event(Event::InsertedPin(command_request.clone().requester, cid)),

				IpfsCommand::RemoveBlock(cid) =>
					Self::deposit_event(Event::RemovedBlock(command_request.clone().requester, cid)),

				IpfsCommand::RemovePin(cid) =>
					Self::deposit_event(Event::RemovedPin(command_request.clone().requester, cid)),

				IpfsCommand::FindPeer(_) => Self::deposit_event(Event::FoundPeers(
					command_request.clone().requester,
					data.clone(),
				)),

				IpfsCommand::GetProviders(cid) => Self::deposit_event(Event::CidProviders(
					command_request.clone().requester,
					cid,
					data.clone(),
				)),
			}
		}

		Ok(())
	}
}

fn find_value_of_type_in_vector<T: TypeEquality + Clone>(value: &T, vector: &Vec<T>) -> Option<T> {
	let found_value = vector.iter().find(|curr_value| value.eq_type(*curr_value));

	let ret_val: Option<T> = match found_value {
		Some(value) => Some(value.clone()),
		None => None,
	};

	ret_val
}

fn contains_value_of_type_in_vector<T: TypeEquality + Clone>(value: &T, vector: &Vec<T>) -> bool {
	let ret_val = match find_value_of_type_in_vector(value, vector) {
		Some(_) => true,
		None => false,
	};

	ret_val
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
