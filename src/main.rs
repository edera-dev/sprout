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

    let mut final_entries = Vec::new();
    for (context, entry) in all_entries {
        let mut context = context.fork();
        context.insert(&entry.values);
        let context = context.finalize().freeze();

        final_entries.push((context, entry));
    }

    println!("Boot Entries:");
    for (index, (context, entry)) in final_entries.iter().enumerate() {
        let title = context.stamp(&entry.title);
        println!("  Entry {}: {}", index + 1, title);
    }

    // let mut input = String::new();
    // std::io::stdin().read_line(&mut input).expect("failed to read line");
    // let input = input.trim();
    // let Some(index) = input.parse::<usize>().ok().and_then(|value| if value > final_entries.len() {
    //     None
    // } else {
    //     Some(value)
    // }) else {
    //     eprintln!("invalid entry number");
    //     continue;
    // };
    let index = 1;

    let (context, entry) = &final_entries[index - 1];

    for action in &entry.actions {
        let Some(action) = context.root().actions().get(action) else {
            panic!("unknown action: {}", action);
        };
        actions::execute(context.clone(), action);
    }
}
