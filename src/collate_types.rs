#[macro_export]
macro_rules! collate_types {
    ($($command:ident),*) => {{
        let mut type_map = ::specta::TypeDefs::default();
        (
            $crate::Commands(vec![
                $(::specta::fn_datatype!(&mut type_map, $command)),*
            ]),
            type_map,
        )
    }};
}
