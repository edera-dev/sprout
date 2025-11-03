use serde::{Deserialize, Serialize};

/// Configuration for the chainload action.
pub mod chainload;

/// Configuration for the edera action.
pub mod edera;

/// Configuration for the print action.
pub mod print;

/// Declares an action that sprout can execute.
/// Actions allow configuring sprout's internal runtime mechanisms with values
/// that you can specify via other concepts.
///
/// Actions are the main work that Sprout gets done, like booting Linux.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ActionDeclaration {
    /// Chainload to another EFI application.
    /// This allows you to load any EFI application, either to boot an operating system
    /// or to perform more EFI actions and return to sprout.
    #[serde(default)]
    pub chainload: Option<chainload::ChainloadConfiguration>,
    /// Print a string to the EFI console.
    #[serde(default)]
    pub print: Option<print::PrintConfiguration>,
    /// Boot the Edera hypervisor and the root operating system.
    /// This action is an extension on top of the Xen EFI stub that
    /// is specific to Edera.
    #[serde(default, rename = "edera")]
    pub edera: Option<edera::EderaConfiguration>,
}
