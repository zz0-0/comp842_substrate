use jsonrpsee::{
	core::{Error as JsonRpseeError, RpcResult},
	proc_macros::rpc,
	types::error::{CallError, ErrorObject},
};
pub use pallet_knowledge_runtime_api::{
	IpfsApi as TDSIpfsRuntimeApi, KnowledgeApi as TemplateRuntimeApi,
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

#[rpc(client, server)]
pub trait KnowledgeApi<BlockHash> {
	#[method(name = "template_getValue")]
	fn get_value(&self, at: Option<BlockHash>) -> RpcResult<u32>;
}

/// A struct that implements the `KnowledgeApi`.
pub struct TemplatePallet<C, Block> {
	// If you have more generics, no need to TemplatePallet<C, M, N, P, ...>
	// just use a tuple like TemplatePallet<C, (M, N, P, ...)>
	client: Arc<C>,
	_marker: std::marker::PhantomData<Block>,
}

impl<C, Block> TemplatePallet<C, Block> {
	/// Create new `TemplatePallet` instance with the given reference to the client.
	pub fn new(client: Arc<C>) -> Self {
		Self { client, _marker: Default::default() }
	}
}

impl<C, Block> KnowledgeApiServer<<Block as BlockT>::Hash> for TemplatePallet<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: TemplateRuntimeApi<Block>,
{
	fn get_value(&self, at: Option<<Block as BlockT>::Hash>) -> RpcResult<u32> {
		let api = self.client.runtime_api();
		let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

		api.get_value(&at).map_err(runtime_error_into_rpc_err)
	}
}

const RUNTIME_ERROR: i32 = 1;

/// Converts a runtime trap into an RPC error.
fn runtime_error_into_rpc_err(err: impl std::fmt::Debug) -> JsonRpseeError {
	CallError::Custom(ErrorObject::owned(
		RUNTIME_ERROR,
		"Runtime error",
		Some(format!("{:?}", err)),
	))
	.into()
}

#[rpc(client, server)]
pub trait IpfsApi<BlockHash> {
	// #[method(name = "ipfs_getFileURLForCID")]
	// fn get_file_url_for_cid(&self, cid: &str, at: Option<BlockHash>) -> RpcResult<String>;

	#[method(name = "ipfs_getFileURLForMetaData")]
	fn get_file_url_for_meta_data(
		&self,
		meta_data: &str,
		at: Option<BlockHash>,
	) -> RpcResult<String>;
}

impl<C, Block> IpfsApiServer<<Block as BlockT>::Hash> for TDSIpfsPallet<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: TDSIpfsRuntimeApi<Block>,
{
	// fn get_file_url_for_cid(&self, cid: &str, at: Option<<Block as BlockT>::Hash>) ->
	// RpcResult<String> { 	let api = self.client.runtime_api();
	// 	let cid_bytes = cid.as_bytes();
	// 	let cid_vec = sp_std::vec::Vec::from(cid_bytes);

	// 	let at = BlockId::hash(at.unwrap_or_else(||self.client.info().best_hash));
	// 	let result = api.get_file_url_for_cid(&at,
	// 										  cid_vec);

	// 	match result {
	// 		Ok(cid_address_raw) => {
	// 			let cid_address = String::from_utf8(cid_address_raw).unwrap();
	// 			RpcResult::Ok(cid_address)
	// 		}
	// 		Err(api_error) => {
	// 			let error = runtime_error_into_rpc_err(api_error);
	// 			RpcResult::Err(error)
	// 		}
	// 	}
	// }

	fn get_file_url_for_meta_data(
		&self,
		meta_data: &str,
		at: Option<<Block as BlockT>::Hash>,
	) -> RpcResult<String> {
		let api = self.client.runtime_api();
		let meta_data_bytes = meta_data.as_bytes();
		let meta_data_vec = sp_std::vec::Vec::from(meta_data_bytes);

		let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
		let result = api.get_file_url_for_meta_data(&at, meta_data_vec);

		match result {
			Ok(cid_raw) => {
				let cid_address = String::from_utf8(cid_raw).unwrap();
				Ok(cid_address)
			},
			Err(api_error) => {
				let error = runtime_error_into_rpc_err(api_error);
				Err(error)
			},
		}
	}
}
