#[macro_export]
macro_rules! collate_types {
    ($($command:ident),*) => {{
        let mut type_map = ::specta::TypeDefs::default();
        (
            vec![
                $(
                    ::specta::export_command_datatype(
                        $command as $crate::internal::paste! { [<__specta__cmd__ $command>]!(@signature) },
                        $crate::internal::paste! { [<__specta__cmd__ $command>]!(@name) },
                        &mut type_map,
                        $crate::internal::paste! { [<__specta__cmd__ $command>]!(@arg_names) }
                    )
                ),*
            ],
            type_map,
        )
    }};
}
