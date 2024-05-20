// use crate::Config::Currency;
use crate::pallet::{BalanceOf, Config};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;

#[derive(Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[codec(mel_bound())]
pub struct SaleData<T: Config> {
	pub price: BalanceOf<T>,
}

impl<T: Config> Default for SaleData<T> {
	fn default() -> Self {
		Self { price: Default::default() }
	}
}

#[derive(Encode, Decode, Eq, PartialEq, Debug, Clone, TypeInfo, MaxEncodedLen, Default)]
#[scale_info(skip_type_params(S))]
#[codec(mel_bound())]
pub struct Permit {
	pub viewable: bool,
	pub editable: bool,
	pub sellable: bool,
}

impl Permit {
	pub fn new(viewable: bool, editable: bool, sellable: bool) -> Self {
		Permit { viewable, editable, sellable }
	}

	pub fn viewable(&self) -> bool {
		self.viewable.clone()
	}

	pub fn editable(&self) -> bool {
		self.editable.clone()
	}

	pub fn sellable(&self) -> bool {
		self.sellable.clone()
	}
}
