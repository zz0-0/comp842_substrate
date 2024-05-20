use crate::pallet::Config;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	pallet_prelude::{Get, RuntimeDebug},
	BoundedVec,
};
use scale_info::TypeInfo;

pub trait Sellable<AccountId, ResourceId> {
	fn origin_owner(knowledge_block_id: ResourceId, account: AccountId) -> AccountId;
	fn transfer(knowledge_block_id: ResourceId, from: AccountId, to: AccountId);
}

#[derive(RuntimeDebug, Encode, Decode, Eq, PartialEq, Clone, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(S))]
#[codec(mel_bound())]
pub struct Content<T: Config, S: Get<u32>> {
	pub owner: T::AccountId,
	pub knowledge_block_id: T::KnowledgeBlockId,
	// pub parent: T::KnowledgeBlockId,
	// pub children: BoundedVec<T::KnowledgeBlockId, S>,
	// pub previous: T::KnowledgeBlockId,
	pub title: BoundedVec<u8, S>,
	pub text: BoundedVec<u8, S>,
}

impl<T: Config, S: Get<u32>> Content<T, S> {
	pub fn new(
		owner: T::AccountId,
		knowledge_block_id: T::KnowledgeBlockId,
		// parent: T::KnowledgeBlockId,
		// children: BoundedVec<T::KnowledgeBlockId, S>,
		// previous: T::KnowledgeBlockId,
		title: BoundedVec<u8, S>,
		text: BoundedVec<u8, S>,
	) -> Self {
		Content {
			owner,
			knowledge_block_id,
			// parent, children, previous, viewable, editable, ,
			title,
			text,
		}
	}

	pub fn owner(&self) -> T::AccountId {
		self.owner.clone()
	}

	pub fn knowledge_block_id(&self) -> T::KnowledgeBlockId {
		self.knowledge_block_id.clone()
	}

	pub fn title(&self) -> BoundedVec<u8, S> {
		self.title.clone()
	}

	pub fn text(&self) -> BoundedVec<u8, S> {
		self.text.clone()
	}

	pub fn set_title(&mut self, title: BoundedVec<u8, S>) {
		self.title = title;
	}

	pub fn set_text(&mut self, text: BoundedVec<u8, S>) {
		self.text = text;
	}
}
