//! The decision layer: the event's decision from the rules crate (`decision`), the restrictions the
//! binary adds beside it (`restrict`), the judge's gate (`seal`), the picker (`picker`), the
//! investigator (`investigator`) and what is told after (`tell`). The `hook` command that runs them is
//! `hook.rs`. A decision is returned, never stored.

pub mod lens;
pub mod decision;
mod hook;
pub mod hook_output;
pub mod investigator;
pub mod picker;
mod restrict;
pub mod seal;
pub mod tell;

pub(crate) use hook::run_hook;
