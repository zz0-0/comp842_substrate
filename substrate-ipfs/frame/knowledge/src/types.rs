use crate::pallet::Config;
use codec::{Decode, Encode};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;
use sp_std::vec::Vec;

pub trait Sellable<AccountId, ResourceId> {
	// fn origin_owner(block_number: ResourceId, account: AccountId) -> AccountId;
	fn transfer(block_number: ResourceId, from: AccountId, to: AccountId);
}

#[derive(RuntimeDebug, Encode, Decode, Eq, PartialEq, Clone, TypeInfo)]
// #[scale_info(skip_type_params(S))] , S: Get<u32>
#[codec(mel_bound())]
pub struct Content<T: Config> {
	pub owner: T::AccountId,
	pub block_number: T::BlockNumber,
	// pub parent: T::BlockNumber,
	// pub children: BoundedVec<T::BlockNumber, S>,
	// pub previous: T::BlockNumber,
	pub title: Vec<u8>,
	pub text: Vec<u8>,
}

impl<T: Config> Content<T> {
	// , S: Get<u32> , S
	pub fn new(
		owner: T::AccountId,
		block_number: T::BlockNumber,
		// parent: T::BlockNumber,
		// children: BoundedVec<T::BlockNumber, S>,
		// previous: T::BlockNumber,
		title: Vec<u8>,
		text: Vec<u8>,
	) -> Self {
		Content {
			owner,
			block_number,
			// parent, children, previous, viewable, editable, ,
			title,
			text,
		}
	}

	pub fn owner(&self) -> T::AccountId {
		self.owner.clone()
	}

	pub fn block_number(&self) -> T::BlockNumber {
		self.block_number.clone()
	}

	pub fn title(&self) -> Vec<u8> {
		self.title.clone()
	}

	pub fn text(&self) -> Vec<u8> {
		self.text.clone()
	}

	pub fn set_title(&mut self, title: Vec<u8>) {
		self.title = title;
	}

	pub fn set_text(&mut self, text: Vec<u8>) {
		self.text = text;
	}
}
