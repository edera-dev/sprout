use crate::actions;
use crate::context::SproutContext;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct PhasesConfiguration {
    #[serde(default)]
    pub early: Vec<PhaseConfiguration>,
    #[serde(default)]
    pub startup: Vec<PhaseConfiguration>,
    #[serde(default)]
    pub late: Vec<PhaseConfiguration>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct PhaseConfiguration {
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub values: BTreeMap<String, String>,
}

pub fn phase(context: Rc<SproutContext>, phase: &[PhaseConfiguration]) -> anyhow::Result<()> {
    for item in phase {
        let mut context = context.fork();
        context.insert(&item.values);
        let context = context.freeze();

        for action in item.actions.iter() {
            actions::execute(context.clone(), action)
                .context(format!("unable to execute action '{}'", action))?;
        }
    }
    Ok(())
}
