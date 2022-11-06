use std::future::Future;

use serde::Serialize;
use specta::{DataType, DefOpts, Type};

pub trait TypedCommandResult<TMarker> {
    fn to_datatype(opts: DefOpts) -> DataType;
}

pub enum TypedCommandResultSerialize {}
impl<T: Serialize + Type> TypedCommandResult<TypedCommandResultSerialize> for T {
    fn to_datatype(opts: DefOpts) -> DataType {
        T::reference(opts, &[])
    }
}

pub struct TypedCommandResultResult<TMarker>(TMarker);
impl<TMarker, T: TypedCommandResult<TMarker>, E>
    TypedCommandResult<TypedCommandResultResult<TMarker>> for Result<T, E>
{
    fn to_datatype(opts: DefOpts) -> DataType {
        T::to_datatype(opts)
    }
}

pub struct TypedCommandResultFuture<TMarker>(TMarker);
impl<TMarker, T: TypedCommandResult<TMarker>, TFut: Future<Output = T>>
    TypedCommandResult<TypedCommandResultFuture<TMarker>> for TFut
{
    fn to_datatype(opts: DefOpts) -> DataType {
        T::to_datatype(opts)
    }
}
