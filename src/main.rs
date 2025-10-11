#![feature(uefi_std)]

use crate::config::PhaseConfiguration;
use crate::context::{RootContext, SproutContext};
use anyhow::{Context, Result, bail};
use log::info;
use std::rc::Rc;

pub mod actions;
pub mod config;
pub mod context;
pub mod generators;
pub mod setup;
pub mod utils;

fn phase(context: Rc<SproutContext>, phase: &[PhaseConfiguration]) -> Result<()> {
    for item in phase {
        let mut context = context.fork();
        context.insert(&item.values);
        let context = context.freeze();

        for action in item.actions.iter() {
            actions::execute(context.clone(), action)
                .context(format!("failed to execute action '{}'", action))?;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    setup::init()?;

    let config = config::load()?;

    if config.version != config::latest_version() {
        bail!("unsupported configuration version: {}", config.version);
    }

    let mut root = RootContext::new();
    root.actions_mut().extend(config.actions.clone());

    let mut context = SproutContext::new(root);
    context.insert(&config.values);
    let context = context.freeze();

    phase(context.clone(), &config.phases.startup)?;

    let mut all_entries = Vec::new();

    for (_name, entry) in config.entries {
        all_entries.push((context.clone(), entry));
    }

    for (_name, generator) in config.generators {
        let context = context.fork().freeze();

        for entry in generators::generate(context.clone(), &generator)? {
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

    info!("entries:");
    for (index, (context, entry)) in final_entries.iter().enumerate() {
        let title = context.stamp(&entry.title);
        info!("  entry {}: {}", index + 1, title);
    }

    let index = 1;

    let (context, entry) = &final_entries[index - 1];

    for action in &entry.actions {
        let action = context.stamp(action);
        actions::execute(context.clone(), &action)
            .context(format!("failed to execute action '{}'", action))?;
    }
    Ok(())
}
