use specta::{DataType, DefOpts, ObjectField, ObjectType, TypeDefs};

use crate::{typed_command_arg::TypedCommandArg, TypedCommandResult};

#[derive(Debug)]
pub struct CommandDataType {
    pub(crate) name: &'static str,
    pub(crate) input: Option<DataType>,
    pub(crate) result: DataType,
}

pub trait TypedCommand<TMarker> {
    fn to_datatype(
        name: &'static str,
        type_map: &mut TypeDefs,
        fields: &[&'static str],
    ) -> CommandDataType;
}

impl<TResultMarker, TResult: TypedCommandResult<TResultMarker>> TypedCommand<TResultMarker>
    for fn() -> TResult
{
    fn to_datatype(
        name: &'static str,
        type_map: &mut TypeDefs,
        _fields: &[&'static str],
    ) -> CommandDataType {
        CommandDataType {
            name,
            input: None,
            result: TResult::to_datatype(DefOpts {
                parent_inline: false,
                type_map,
            }),
        }
    }
}

impl<
        TArg1Marker,
        TArg1: TypedCommandArg<TArg1Marker>,
        TResultMarker,
        TResult: TypedCommandResult<TResultMarker>,
    > TypedCommand<(TResultMarker, TArg1Marker)> for fn(TArg1) -> TResult
{
    fn to_datatype(
        name: &'static str,
        type_map: &mut TypeDefs,
        fields: &[&'static str],
    ) -> CommandDataType {
        CommandDataType {
            name,
            input: Some(DataType::Object(ObjectType {
                name: "_unreachable_".into(),
                generics: vec![],
                fields: [TArg1::to_datatype(DefOpts {
                    parent_inline: false,
                    type_map,
                })
                .map(|ty| ObjectField {
                    name: fields[0].into(),
                    ty,
                    optional: false,
                })]
                .into_iter()
                .filter_map(|v| v)
                .collect(),
                tag: None,
                type_id: None,
            })),
            result: TResult::to_datatype(DefOpts {
                parent_inline: false,
                type_map,
            }),
        }
    }
}

impl<
        TArg2Marker,
        TArg2: TypedCommandArg<TArg2Marker>,
        TArg1Marker,
        TArg1: TypedCommandArg<TArg1Marker>,
        TResultMarker,
        TResult: TypedCommandResult<TResultMarker>,
    > TypedCommand<(TResultMarker, TArg1Marker, TArg2Marker)> for fn(TArg1, TArg2) -> TResult
{
    fn to_datatype(
        name: &'static str,
        type_map: &mut TypeDefs,
        fields: &[&'static str],
    ) -> CommandDataType {
        CommandDataType {
            name,
            input: Some(DataType::Object(ObjectType {
                name: "_unreachable_".into(),
                generics: vec![],
                fields: [
                    TArg1::to_datatype(DefOpts {
                        parent_inline: false,
                        type_map,
                    })
                    .map(|ty| ObjectField {
                        name: fields[0].into(),
                        ty: ty,
                        optional: false,
                    }),
                    TArg2::to_datatype(DefOpts {
                        parent_inline: false,
                        type_map,
                    })
                    .map(|ty| ObjectField {
                        name: fields[1].into(),
                        ty: ty,
                        optional: false,
                    }),
                ]
                .into_iter()
                .filter_map(|v| v)
                .collect(),
                tag: None,
                type_id: None,
            })),
            result: TResult::to_datatype(DefOpts {
                parent_inline: false,
                type_map,
            }),
        }
    }
}

// TODO: Support up to 16 args.

pub fn export_command_datatype<TMarker, T: TypedCommand<TMarker>>(
    _: T,
    name: &'static str,
    type_map: &mut TypeDefs,
    fields: &[&'static str],
) -> CommandDataType {
    T::to_datatype(name, type_map, fields)
}
