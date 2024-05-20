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

use frame_system::offchain::{AppCrypto, CreateSignedTransaction, SendSignedTransaction, Signer};
use log::info;
use sp_core::offchain::{IpfsRequest, IpfsResponse};

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	use frame_support::{
		codec::{Decode, Encode, MaxEncodedLen},
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

	use pallet_tds_ipfs_core::{
		addresses_to_utf8_safe_bytes, generate_id, ipfs_request, ocw_parse_ipfs_response,
		ocw_process_command,
		storage::{read_cid_data_for_block_number, store_cid_data_for_values},
		types::{IpfsFile, OffchainData},
		CommandRequest, Error as IpfsError, IpfsCommand, TypeEquality,
	};

	use sp_std::str;
	const PROCESSED_COMMANDS: &[u8; 24] = b"ipfs::processed_commands";

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_tds_ipfs_core::Config + scale_info::TypeInfo
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type KnowledgeBlockId: Parameter + AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen;
		type DocumentId: Parameter + AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen;
		// + Deserialize
		// type WeightInfo: WeightInfo;
		#[pallet::constant]
		type MaxLength: Get<u32>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn ipfs_files)]
	pub(super) type IpfsFileStorage<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		Vec<u8>, // key is cid
		IpfsFile,
		ValueQuery,
	>;

	// the whole markdown document
	#[pallet::storage]
	#[pallet::getter(fn document)]
	pub type Document<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::DocumentId,
		Blake2_128Concat,
		T::AccountId,
		Content<T, T::MaxLength>,
		OptionQuery,
	>;

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

	#[pallet::storage]
	#[pallet::getter(fn commands)]
	pub type Commands<T: Config> = StorageValue<_, Vec<CommandRequest<T>>>;

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
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub commands: Vec<CommandRequest<T>>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> GenesisConfig<T> {
			GenesisConfig { commands: Vec::<CommandRequest<T>>::new() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			Commands::<T>::set(Some(Vec::<CommandRequest<T>>::new()));
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn offchain_worker(block_number: T::BlockNumber) {
			if let Err(_err) = Self::print_metadata(&"*** IPFS off-chain worker started with ***") {
				log::error!("IPFS: Error occurred during `print_metadata`");
			}

			if let Err(_err) = Self::ocw_process_command_requests(block_number) {
				log::error!("IPFS: command request");
			}
		}
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
		/// Adds arbitrary bytes to the IPFS repository. The registered `Cid` is printed out in the
		/// logs.
		#[pallet::call_index(0)]
		#[pallet::weight(0)]
		pub fn add_bytes(
			origin: OriginFor<T>,
			received_bytes: Vec<u8>,
			version: u8,
			meta_data: Vec<u8>,
		) -> DispatchResult {
			let requester = ensure_signed(origin)?;
			let block_number = frame_system::Pallet::<T>::block_number();
			store_cid_data_for_values::<T>(block_number, received_bytes, meta_data, false);

			let mut commands = Vec::<IpfsCommand>::new();
			commands.push(IpfsCommand::AddBytes(version));

			let ipfs_command_request = CommandRequest::<T> {
				identifier: generate_id::<T>(),
				requester: requester.clone(),
				ipfs_commands: commands,
			};

			Commands::<T>::append(ipfs_command_request);
			Ok(Self::deposit_event(Event::QueuedDataToAdd(requester.clone(), requester)))
		}

		#[pallet::call_index(1)]
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
					info!("Removing at index {}", index.clone());
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

		// create knowledge blocks with title, text
		// add parent knowledge block id, children knowledge block id, and previous knowledge block
		// id
		#[pallet::call_index(2)]
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
		#[pallet::call_index(3)]
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
		#[pallet::call_index(4)]
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
		#[pallet::call_index(5)]
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

	impl<T: Config> Pallet<T> {
		// transfer the knowledge block to new account
		pub fn inner_transfer(
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

		pub fn get_file_url_for_meta_data(meta_data: Vec<u8>) -> Vec<u8> {
			let mut ret_val = Vec::<u8>::new();

			if let Some(file) = IpfsFileStorage::<T>::iter_values()
				.find(|curr_file| curr_file.meta_data == meta_data)
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

		// Iterate over all of the Active CommandRequests calling them.
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
						_ = Self::signed_callback(
							&command_request,
							callback_response,
							offchain_data,
						);
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

			info!("{}", message);
			info!("IPFS: Is currently connected to {} peers", peers.len());
			if !peers.is_empty() {
				info!("IPFS: Peer Ids: {:?}", str::from_utf8(&addresses_to_utf8_safe_bytes(peers)))
			}

			info!("IPFS: CommandRequest size: {}", Commands::<T>::decode_len().unwrap_or(0));
			Ok(())
		}

		// callback to the on-chain validators to continue processing the CID
		fn signed_callback(
			command_request: &CommandRequest<T>,
			data: Vec<u8>,
			offchain_data: Option<OffchainData>,
		) -> Result<(), IpfsError<T>> {
			let signer = Signer::<T, T::AuthorityId>::all_accounts();

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
						info!("callback sent")
					},
					Err(e) => {
						log::error!("Failed to submit transaction {:?}", e)
					},
				}
			}

			Ok(())
		}

		//
		// - Ideally the callback function can be override by another pallet that is coupled to this
		//   one allowing for custom functionality.
		// - data can be used for callbacks IE add a cid to the signer / uploader.
		// - Validate a connected peer has the CID, and which peer has it etc.

		fn command_callback(command_request: &CommandRequest<T>, data: Vec<u8>) -> Result<(), ()> {
			let contains_cat_bytes = contains_value_of_type_in_vector(
				&IpfsCommand::CatBytes(Vec::<u8>::new()),
				&command_request.ipfs_commands,
			);
			let data_len_exceeded = data.len() > 20;

			if contains_cat_bytes && data_len_exceeded {
				// Avoid excessive data logging
				info!("Received data for cat bytes with length: {:?}", &data.len());
			} else {
				if let Ok(utf8_str) = str::from_utf8(&*data) {
					info!("Received string: {:?}", utf8_str);
				} else {
					info!("Received data: {:?}", data);
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

					IpfsCommand::InsertPin(cid) => Self::deposit_event(Event::InsertedPin(
						command_request.clone().requester,
						cid,
					)),

					IpfsCommand::RemoveBlock(cid) => Self::deposit_event(Event::RemovedBlock(
						command_request.clone().requester,
						cid,
					)),

					IpfsCommand::RemovePin(cid) => Self::deposit_event(Event::RemovedPin(
						command_request.clone().requester,
						cid,
					)),

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

	fn find_value_of_type_in_vector<T: TypeEquality + Clone>(
		value: &T,
		vector: &Vec<T>,
	) -> Option<T> {
		let found_value = vector.iter().find(|curr_value| value.eq_type(*curr_value));

		let ret_val: Option<T> = match found_value {
			Some(value) => Some(value.clone()),
			None => None,
		};

		ret_val
	}

	fn contains_value_of_type_in_vector<T: TypeEquality + Clone>(
		value: &T,
		vector: &Vec<T>,
	) -> bool {
		let ret_val = match find_value_of_type_in_vector(value, vector) {
			Some(_) => true,
			None => false,
		};

		ret_val
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
