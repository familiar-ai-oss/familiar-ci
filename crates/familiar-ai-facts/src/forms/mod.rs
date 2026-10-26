//! Each fact kind's fields as a typed struct, grouped by what the facts are about.
//! The `FORM` table registers the same fields for the untyped append.

mod anchor;
mod intent;
mod proof;
mod review;
mod runs;
mod work;

pub use anchor::*;
pub use intent::*;
pub use proof::*;
pub use review::*;
pub use runs::*;
pub use work::*;
