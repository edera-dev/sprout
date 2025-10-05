use crate::config::ActionDeclaration;
use crate::context::Context;
use std::rc::Rc;

pub mod chainload;

pub fn execute(context: Rc<Context>, action: &ActionDeclaration) {
    let context = context.finalize().freeze();

    if let Some(chainload) = &action.chainload {
        chainload::chainload(context, chainload);
    } else {
        panic!("unknown action configuration");
    }
}
