use serde::Deserialize;
use specta::{DataType, DefOpts, Type};
use tauri::{AppHandle, Runtime, State, Window};

pub trait TypedCommandArg<TMarker> {
    fn to_datatype(opts: DefOpts) -> Option<DataType>;
}

pub enum TypedCommandArgWindowMarker {}
impl<R: Runtime> TypedCommandArg<TypedCommandArgWindowMarker> for Window<R> {
    fn to_datatype(_: DefOpts) -> Option<DataType> {
        None
    }
}

pub enum TypedCommandArgStateMarker {}
impl<'r, T: Send + Sync + 'static> TypedCommandArg<TypedCommandArgStateMarker> for State<'r, T> {
    fn to_datatype(_: DefOpts) -> Option<DataType> {
        None
    }
}

pub enum TypedCommandArgAppHandleMarker {}
impl<R: Runtime> TypedCommandArg<TypedCommandArgAppHandleMarker> for AppHandle<R> {
    fn to_datatype(_: DefOpts) -> Option<DataType> {
        None
    }
}

pub enum TypedCommandArgDeserializeMarker {}
impl<'de, T: Deserialize<'de> + Type> TypedCommandArg<TypedCommandArgDeserializeMarker> for T {
    fn to_datatype(opts: DefOpts) -> Option<DataType> {
        Some(T::reference(opts, &[]))
    }
}
