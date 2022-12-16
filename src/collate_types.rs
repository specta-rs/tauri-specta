#[macro_export]
macro_rules! collate_types {
    ($($command:path),*) => {{
        let mut type_map = ::specta::TypeDefs::default();
        (
            vec![
                $(::specta::fn_datatype!(type_map, $command)),*
            ],
            type_map,
        )
    }};
}
