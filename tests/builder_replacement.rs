//! Integration tests for replacing a builder's commands or events.

#![cfg(feature = "derive")]
#![allow(dead_code, clippy::unwrap_used)]

use std::{any::TypeId, collections::BTreeSet, io, path::Path};

use serde::Serialize;
use specta::{Type, Types};
use tauri::test::MockRuntime;
use tauri_specta::{
    Builder, BuilderConfiguration, Event, LanguageExt, collect_commands, collect_events,
};

#[derive(Serialize, Type)]
struct SharedPayload(String);

#[derive(Serialize, Type)]
struct OldCommandPayload(OldCommandField, SharedPayload, ExplicitType);

#[derive(Serialize, Type)]
struct OldCommandField(String);

#[derive(Serialize, Type)]
struct ExplicitType(String);

#[tauri::command]
fn old_command() -> OldCommandPayload {
    OldCommandPayload(
        OldCommandField(String::new()),
        SharedPayload(String::new()),
        ExplicitType(String::new()),
    )
}

#[tauri::command]
fn current_command() -> SharedPayload {
    SharedPayload(String::new())
}

#[derive(Type, Event)]
struct CurrentEvent(SharedPayload);

#[derive(Type, Event)]
struct OldEvent(OldEventField);

#[derive(Type)]
struct OldEventField(String);

#[derive(Type, Event)]
#[tauri_specta(event_name = "current-event")]
struct MergedEvent(bool);

#[derive(Type, Event)]
struct ExtraEvent(bool);

struct Inspect<F>(F);

impl<F: FnOnce(&BuilderConfiguration)> LanguageExt for Inspect<F> {
    type Error = io::Error;

    fn export(self, cfg: &BuilderConfiguration, _: &Path) -> Result<(), Self::Error> {
        (self.0)(cfg);
        Ok(())
    }
}

fn names(cfg: &BuilderConfiguration) -> BTreeSet<&str> {
    cfg.types
        .into_unsorted_iter()
        .filter(|ty| ty.module_path == module_path!())
        .map(|ty| ty.name.as_ref())
        .collect()
}

#[test]
fn replacing_commands_drops_only_the_old_command_types() {
    let mut explicit = Types::default().register::<SharedPayload>();
    explicit.iter_mut(|ty| {
        if ty.name == "SharedPayload" {
            ty.name = "ExplicitSharedPayload".into();
        }
    });

    let original = Builder::<MockRuntime>::new()
        .commands(collect_commands![old_command])
        .types(&explicit)
        .events(collect_events![CurrentEvent])
        .typ::<ExplicitType>();
    let replaced = original
        .clone()
        .commands(collect_commands![current_command]);

    original
        .export(
            Inspect(|cfg: &BuilderConfiguration| {
                assert_eq!(cfg.commands[0].name, "old_command");
                assert!(names(cfg).contains("OldCommandPayload"));
                assert!(names(cfg).contains("SharedPayload"));
            }),
            "",
        )
        .unwrap();

    replaced
        .export(
            Inspect(|cfg: &BuilderConfiguration| {
                assert_eq!(cfg.commands[0].name, "current_command");
                assert!(cfg.events.contains_key(CurrentEvent::NAME));
                assert_eq!(cfg.types.roots().len(), 2);
                assert_eq!(
                    names(cfg),
                    BTreeSet::from(["CurrentEvent", "ExplicitSharedPayload", "ExplicitType"])
                );
            }),
            "",
        )
        .unwrap();
}

#[test]
fn replacing_events_keeps_commands_and_honors_event_merge() {
    let builder = Builder::<MockRuntime>::new()
        .events(collect_events![OldEvent])
        .commands(collect_commands![current_command])
        .events(collect_events![CurrentEvent, ExtraEvent].merge(collect_events![MergedEvent]));

    builder
        .export(
            Inspect(|cfg: &BuilderConfiguration| {
                assert_eq!(cfg.commands[0].name, "current_command");
                assert_eq!(cfg.events.len(), 2);
                assert_eq!(
                    cfg.events[CurrentEvent::NAME].0,
                    TypeId::of::<MergedEvent>()
                );
                assert_eq!(
                    names(cfg),
                    BTreeSet::from(["ExtraEvent", "MergedEvent", "SharedPayload"])
                );
            }),
            "",
        )
        .unwrap();
}
