#![cfg_attr(not(feature = "std"), no_std)]
//! Pallet provides all core functionality for IPFS node communication and invoking functionality.
//!
//! Such functionality can be used from other pallets by invoking referring IpfsCommand enum
//! which can be passed to the pallet using deposit_event method.
//!
//! See tds-ipfs/src/lib.rs for examples
//!
//! Credits goes to:
//!
//! https://github.com/WunderbarNetwork/substrate
//! https://github.com/rs-ipfs/substrate/
//!

use codec::{Decode, Encode};
pub use pallet::*;

use sp_runtime::{
	offchain::{
		ipfs,
		storage::{MutateStorageError, StorageRetrievalError, StorageValueRef},
	},
	RuntimeDebug,
};

#[cfg(feature = "std")]
use frame_support::serde::{Deserialize, Serialize};

use log::info;
use sp_core::offchain::{Duration, IpfsRequest, IpfsResponse, OpaqueMultiaddr};
use sp_std::{str, vec::Vec};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod storage;
pub mod types;

use frame_support::traits::Randomness;

// Create a "unique" id for each command
//    Note: Nodes on the network will come to the same value for each id.
pub fn generate_id<T: Config>() -> [u8; 32] {
	let payload = (
		T::IpfsRandomness::random(&b"ipfs-request-id"[..]).0,
		<frame_system::Pallet<T>>::block_number(),
	);
	payload.using_encoded(sp_io::hashing::blake2_256)
}

// Process each IPFS `command_request` in the offchain worker
// 1) lock the request for asynchronous processing
// 2) Call each command in CommandRequest.ipfs_commands
//   - Make sure each command is successfully before attempting the next
pub fn ocw_process_command<T: Config>(
	block_number: T::BlockNumber,
	command_request: CommandRequest<T>,
	persistence_key: &[u8; 24],
) -> Result<Vec<IpfsResponse>, Error<T>> {
	// If you wanna add a add a different file system, that would be a correct entry point:
	// - Add a different process_YOUR-FILESYSTEM_command function herer
	// - In addition consider renaming the IpfsCommand to something more generic
	process_ipfs_command(block_number, command_request, persistence_key)
}

// Send a request to the local IPFS node; Can only be called in an offchain worker.
pub fn ipfs_request<T: Config>(request: IpfsRequest) -> Result<IpfsResponse, Error<T>> {
	let ipfs_request =
		ipfs::PendingRequest::new(request).map_err(|_| Error::CannotCreateRequest)?;
	let duration = 1_200;
	ipfs_request
		.try_wait(Some(sp_io::offchain::timestamp().add(Duration::from_millis(duration))))
		.map_err(|_| Error::<T>::RequestTimeout)?
		.map(|req| req.response)
		.map_err(|_error| Error::<T>::RequestFailed)
}

// Parse Each ipfs response resulting in bytes to be used in callback
//   - If multiple responses are found the last response with bytes is returned.
pub fn ocw_parse_ipfs_response<T: Config>(responses: Vec<IpfsResponse>) -> Vec<u8> {
	let mut callback_response = Vec::<u8>::new();

	for response in responses.clone() {
		match response {
			IpfsResponse::CatBytes(bytes_received) => {
				if bytes_received.len() > 1 {
					callback_response = bytes_received
				}
			},
			IpfsResponse::AddBytes(cid) | IpfsResponse::RemoveBlock(cid) => callback_response = cid,

			IpfsResponse::GetClosestPeers(peer_ids) | IpfsResponse::GetProviders(peer_ids) => {
				callback_response = multiple_bytes_to_utf8_safe_bytes(peer_ids)
			},

			IpfsResponse::FindPeer(addresses)
			| IpfsResponse::LocalAddrs(addresses)
			| IpfsResponse::Peers(addresses) => callback_response = addresses_to_utf8_safe_bytes(addresses),

			IpfsResponse::LocalRefs(refs) => {
				callback_response = multiple_bytes_to_utf8_safe_bytes(refs)
			},
			IpfsResponse::Addrs(_) => {},
			IpfsResponse::BitswapStats { .. } => {},
			IpfsResponse::Identity(_, _) => {},
			IpfsResponse::Success => {},
		}
	}
	callback_response
}

// Convert a vector of addresses into a comma separated utf8 safe vector of bytes
pub fn addresses_to_utf8_safe_bytes(addresses: Vec<OpaqueMultiaddr>) -> Vec<u8> {
	multiple_bytes_to_utf8_safe_bytes(addresses.iter().map(|addr| addr.0.clone()).collect())
}

// Flatten a Vector of bytes into a comma seperated utf8 safe vector of bytes
pub fn multiple_bytes_to_utf8_safe_bytes(response: Vec<Vec<u8>>) -> Vec<u8> {
	let mut bytes = Vec::<u8>::new();

	for res in response {
		match str::from_utf8(&res) {
			Ok(str) => {
				if bytes.len() == 0 {
					bytes = Vec::from(str.as_bytes());
				} else {
					bytes = [bytes, Vec::from(str.as_bytes())].join(", ".as_bytes());
				}
			},
			_ => {},
		}
	}

	bytes
}

fn process_ipfs_command<T: Config>(
	block_number: T::BlockNumber,
	command_request: CommandRequest<T>,
	persistence_key: &[u8; 24],
) -> Result<Vec<IpfsResponse>, Error<T>> {
	let acquire_lock = acquire_command_request_lock::<T>(block_number, &command_request);

	match acquire_lock {
		Ok(_block) => {
			let mut result = Vec::<IpfsResponse>::new();

			for command in command_request.clone().ipfs_commands {
				let command_result = match command {
					IpfsCommand::ConnectTo(ref address) => {
						match ipfs_request::<T>(IpfsRequest::Connect(OpaqueMultiaddr(
							address.clone(),
						))) {
							Ok(IpfsResponse::Success) => Ok(result.push(IpfsResponse::Success)),
							_ => Err(Error::<T>::RequestFailed),
						}
					},

					IpfsCommand::DisconnectFrom(ref address) => {
						match ipfs_request::<T>(IpfsRequest::Disconnect(OpaqueMultiaddr(
							address.clone(),
						))) {
							Ok(IpfsResponse::Success) => Ok(result.push(IpfsResponse::Success)),
							_ => Err(Error::<T>::RequestFailed),
						}
					},

					IpfsCommand::AddBytes(ref version) => {
						let bytes_to_add: Vec<u8>;

						if let Ok(data) = storage::read_cid_for_block_number::<T>(block_number) {
							bytes_to_add = data.data.clone();
						} else {
							return Err(Error::<T>::RequestFailed);
						}

						match ipfs_request::<T>(IpfsRequest::AddBytes(
							bytes_to_add,
							version.clone(),
						)) {
							Ok(IpfsResponse::AddBytes(cid)) => {
								// submit_transaction(data);
								Ok(result.push(IpfsResponse::AddBytes(cid)))
							},
							_ => Err(Error::<T>::RequestFailed),
						}
					},

					IpfsCommand::CatBytes(ref cid) => {
						match ipfs_request::<T>(IpfsRequest::CatBytes(cid.clone())) {
							Ok(IpfsResponse::CatBytes(bytes_received)) => {
								Ok(result.push(IpfsResponse::CatBytes(bytes_received)))
							},
							_ => Err(Error::<T>::RequestFailed),
						}
					},

					IpfsCommand::InsertPin(ref cid) => {
						match ipfs_request::<T>(IpfsRequest::InsertPin(cid.clone(), false)) {
							Ok(IpfsResponse::Success) => Ok(result.push(IpfsResponse::Success)),
							_ => Err(Error::<T>::RequestFailed),
						}
					},

					IpfsCommand::RemovePin(ref cid) => {
						match ipfs_request::<T>(IpfsRequest::RemovePin(cid.clone(), false)) {
							Ok(IpfsResponse::Success) => Ok(result.push(IpfsResponse::Success)),
							_ => Err(Error::<T>::RequestFailed),
						}
					},

					IpfsCommand::RemoveBlock(ref _cid) => {
						// TODO: TDS add Cid again!
						// match ipfs_request::<T>(IpfsRequest::RemoveBlock(cid.clone())) {
						match ipfs_request::<T>(IpfsRequest::RemoveBlock()) {
							Ok(IpfsResponse::Success) => Ok(result.push(IpfsResponse::Success)),
							_ => Err(Error::<T>::RequestFailed),
						}
					},

					IpfsCommand::FindPeer(ref peer_id) => {
						match ipfs_request::<T>(IpfsRequest::FindPeer(peer_id.clone())) {
							Ok(IpfsResponse::FindPeer(addresses)) => {
								Ok(result.push(IpfsResponse::FindPeer(addresses)))
							},
							_ => Err(Error::<T>::RequestFailed),
						}
					},

					IpfsCommand::GetProviders(ref cid) => {
						match ipfs_request::<T>(IpfsRequest::GetProviders(cid.clone())) {
							Ok(IpfsResponse::GetProviders(peer_ids)) => {
								Ok(result.push(IpfsResponse::GetProviders(peer_ids)))
							},
							_ => Err(Error::<T>::RequestFailed),
						}
					},
				};

				match command_result {
					Ok(_) => log::debug!("Command successfully processed: {:?}", command),
					Err(_) => log::debug!("Command failed: {:?}", command),
				}
			}

			match processed_commands::<T>(&command_request, persistence_key) {
				Ok(_) => log::debug!("IPFS command request successfully processed"),
				Err(_) => log::debug!("IPFS command request failed"),
			}

			Ok(result)
		},
		_ => Err(Error::<T>::FailedToAcquireLock),
	}
}

// Using the CommandRequest<T>.identifier we can attempt to create a lock via StorageValueRef,
// leaving behind a block number of when the lock was formed.
fn acquire_command_request_lock<T: Config>(
	block_number: T::BlockNumber,
	command_request: &CommandRequest<T>,
) -> Result<T::BlockNumber, MutateStorageError<T::BlockNumber, Error<T>>> {
	let storage = StorageValueRef::persistent(&command_request.identifier);

	storage.mutate(|command_identifier: Result<Option<T::BlockNumber>, StorageRetrievalError>| {
		match command_identifier {
			Ok(Some(block)) =>
				if block_number != block {
					info!("Lock failed, lock was not in current block. block_number: {:?}, block: {:?}",block_number, block );
					Err(Error::<T>::FailedToAcquireLock)
				} else {
					Ok(block)
				},
			_ => {
				info!("IPFS: Acquired lock!");
				Ok(block_number)
			},
		}
	})
}

// Store a list of command identifiers to remove the lock in a following block
fn processed_commands<T: Config>(
	command_request: &CommandRequest<T>,
	persistence_key: &[u8; 24],
) -> Result<Vec<[u8; 32]>, MutateStorageError<Vec<[u8; 32]>, ()>> {
	let processed_commands = StorageValueRef::persistent(persistence_key);

	processed_commands.mutate(
		|processed_commands: Result<Option<Vec<[u8; 32]>>, StorageRetrievalError>| {
			match processed_commands {
				Ok(Some(mut commands)) => {
					commands.push(command_request.identifier);

					Ok(commands)
				},
				_ => {
					let mut res = Vec::<[u8; 32]>::new();
					res.push(command_request.identifier);

					Ok(res)
				},
			}
		},
	)
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type IpfsRandomness: frame_support::traits::Randomness<Self::Hash, Self::BlockNumber>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	// Commands for interacting with IPFS

	// Connection Commands:
	// - ConnectTo(OpaqueMultiaddr)
	// - DisconnectFrom(OpaqueMultiaddr)

	// Data Commands:
	// - AddBytes(Vec<u8>)
	// - CatBytes(Vec<u8>)
	// - InsertPin(Vec<u8>)
	// - RemoveBlock(Vec<u8>)
	// - RemovePin(Vec<u8>)

	//  Dht Commands:
	//  - FindPeer(Vec<u8>)
	// - GetProviders(Vec<u8>)
	#[derive(PartialEq, Eq, Clone, Debug, Encode, Decode, TypeInfo)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	pub enum IpfsCommand {
		// Connection Commands
		ConnectTo(Vec<u8>),
		DisconnectFrom(Vec<u8>),

		// Data Commands
		AddBytes(u8),
		CatBytes(Vec<u8>),
		InsertPin(Vec<u8>),
		RemoveBlock(Vec<u8>),
		RemovePin(Vec<u8>),

		// DHT Commands
		FindPeer(Vec<u8>),
		GetProviders(Vec<u8>),
	}

	pub trait TypeEquality<Rhs = Self>
	where
		Rhs: ?Sized,
	{
		fn eq_type(&self, other: &Rhs) -> bool;
	}

	impl TypeEquality for IpfsCommand {
		fn eq_type(&self, other: &IpfsCommand) -> bool {
			sp_core::sp_std::mem::discriminant(self) == sp_core::sp_std::mem::discriminant(other)
		}
	}

	// CommandRequest is used for issuing requests to an ocw that has IPFS within its runtime.

	//   - identifier: [u8; 32]
	//   - requester: T::AccountId
	//   - ipfs_commands Vec<IpfsCommand>
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo, Default)]
	#[scale_info(skip_type_params(T))]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	pub struct CommandRequest<T: Config> {
		pub identifier: [u8; 32],
		pub requester: T::AccountId,
		pub ipfs_commands: Vec<IpfsCommand>,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {}

	// Errors inform users that something went wrong.
	// - CannotCreateRequest,
	// - RequestTimeout,
	// - RequestFailed,
	// - FailedToAcquireLock,
	#[pallet::error]
	pub enum Error<T> {
		CannotCreateRequest,
		RequestTimeout,
		RequestFailed,
		FailedToAcquireLock,
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {}

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
		fn build(&self) {}
	}
}
