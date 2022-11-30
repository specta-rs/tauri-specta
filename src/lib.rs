mod collate_types;
mod export_openapi;
mod export_ts;
pub use collate_types::*;
pub use export_openapi::*;
pub use export_ts::*;
use specta::{function::FunctionDataType, DataTypeFrom};

#[derive(DataTypeFrom)]
pub struct Commands(pub Vec<FunctionDataType>);

#[doc(hidden)]
pub mod internal {
    pub use paste::paste;
}
