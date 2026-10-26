//! The fact kinds, the append that validates them, and the append-only record on `redb`.
//! No field is named `state` or `status`, and callers supply every fact's timestamp as `at`: the crate never stamps one of its own.

pub mod append;
pub mod form_table;
pub mod forms;
pub mod readers;
pub mod record;

pub use record::AppendIf;
pub use append::{append, append_raw, kind_count, log_to_json, AppendError, Fact, FactFields, Log};
pub use form_table::{form_row, FormRow, FORM, KINDS};
pub use forms::*;
pub use readers::{last_of_kind, of_criterion};
pub use record::{Record, RecordError};
