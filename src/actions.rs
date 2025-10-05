use crate::context::Context;
use std::rc::Rc;

pub mod chainload;
pub mod print;

pub fn execute(context: Rc<Context>, name: impl AsRef<str>) {
    let Some(action) = context.root().actions().get(name.as_ref()) else {
        panic!("unknown action: {}", name.as_ref());
    };
    let context = context.finalize().freeze();

    if let Some(chainload) = &action.chainload {
        chainload::chainload(context, chainload);
    } else if let Some(print) = &action.print {
        print::print(context, print);
    } else {
        panic!("unknown action configuration");
    }
}
