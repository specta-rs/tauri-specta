// TODO: Be aware this will just crash if you throw Rust generics at it because OpenAPI doesn't have them

use std::{fs, io, path::Path};

use openapiv3::{
    Components, Info, MediaType, Operation, PathItem, Paths, ReferenceOr, RequestBody, Responses,
    StatusCode,
};
use specta::{openapi::to_openapi, TypeDefs};

use crate::Commands;

pub fn export_to_openapi(
    (commands, type_map): (Commands, TypeDefs),
    export_path: impl AsRef<Path>,
) -> Result<(), io::Error> {
    let schema = openapiv3::OpenAPI {
        openapi: "3.0.3".into(),
        info: Info {
            title: "Tauri Specta API".into(),
            version: "0.0.1".into(),
            ..Default::default()
        },
        servers: vec![],
        paths: Paths {
            paths: commands
                .0
                .iter()
                .map(|cmd| {
                    (
                        cmd.name.to_string(),
                        ReferenceOr::Item(PathItem {
                            get: Some(Operation {
                                operation_id: Some(cmd.name.to_string()),
                                parameters: vec![],
                                // TODO: to_openapi(&cmd.input)
                                request_body: Some(ReferenceOr::Item(RequestBody {
                                    content: [(
                                        "application/json".into(),
                                        MediaType {
                                            schema: cmd.input.as_ref().map(|v| to_openapi(v)),
                                            ..Default::default()
                                        },
                                    )]
                                    .iter()
                                    .cloned()
                                    .collect(),
                                    ..Default::default()
                                })),
                                responses: Responses {
                                    responses: [(
                                        StatusCode::Code(200),
                                        ReferenceOr::Item(openapiv3::Response {
                                            content: [(
                                                "application/json".to_string(),
                                                MediaType {
                                                    schema: Some(to_openapi(&cmd.result)),
                                                    ..Default::default()
                                                },
                                            )]
                                            .into_iter()
                                            .collect(),
                                            ..Default::default()
                                        }),
                                    )]
                                    .into_iter()
                                    .collect(),
                                    ..Default::default()
                                },
                                ..Default::default()
                            }),
                            ..Default::default()
                        }),
                    )
                })
                .collect(),
            ..Default::default()
        },
        components: Some(Components {
            schemas: type_map
                .iter()
                .map(|(k, v)| {
                    println!("{:?}", v);
                    (k.to_string(), to_openapi(v))
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };

    let doc = serde_json::to_vec(&schema).unwrap();
    fs::write(export_path, doc)?;

    Ok(())
}
