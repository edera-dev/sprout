#![feature(uefi_std)]

use crate::config::PhaseConfiguration;
use crate::context::{Context, RootContext};
use std::rc::Rc;

pub mod actions;
pub mod config;
pub mod context;
pub mod generators;
pub mod setup;
pub mod utils;

fn phase(context: Rc<Context>, phase: &[PhaseConfiguration]) {
    for item in phase {
        let mut context = context.fork();
        context.insert(&item.values);
        let context = context.freeze();

        for action in item.actions.iter() {
            let Some(action) = context.root().actions().get(action) else {
                panic!("unknown action: {}", action);
            };

            actions::execute(context.clone(), action);
        }
    }
}

fn main() {
    setup::init();

    let config = config::load();
    let mut root = RootContext::new();
    root.actions_mut().extend(config.actions.clone());

    let mut context = Context::new(root);
    context.insert(&config.values);
    let context = context.freeze();

    phase(context.clone(), &config.phases.startup);

    let mut all_entries = Vec::new();

    for (_name, entry) in config.entries {
        all_entries.push((context.clone(), entry));
    }

    for (_name, generator) in config.generators {
        let context = context.fork().freeze();

        for entry in generators::generate(context.clone(), &generator) {
            all_entries.push(entry);
        }
    }

    println!("{} entries", all_entries.len());
    for (index, (context, entry)) in all_entries.iter().enumerate() {
        let mut context = context.fork();
        context.insert(&entry.values);
        let context = context.finalize().freeze();

        println!("Entry {}:", index + 1);
        println!("  Title: {}", entry.title);
        println!("  Actions: {:?}", entry.actions);
        println!("  Values: {:?}", context.all_values());
    }

    loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}
