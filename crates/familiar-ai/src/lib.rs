//! The familiar-ai binary's library. A hook call parses Claude Code's event in `input`,
//! catches the record up in `door`, and answers in `decide`; the rules live in `familiar-ai-rules`.

mod commands;
pub mod decide;
pub mod door;
pub mod input;
pub mod teach;

pub use crate::commands::{run, run_with, Outcome, STOPPED_WHILE_DECIDING};
pub use crate::door::{commits, FAMILIAR_FOLDER, PLAN_FILE, RECORD_FILE, TENANT};
pub use crate::input::{hook_input, stdin_has_something, Launching, Surroundings, wants_stdin, STDIN_WAIT};
