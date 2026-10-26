//! Converters from a runner's own report output to the per-test result file (JSON Lines v1, the
//! A0 spike's `kit/SCHEMA.txt`) [draft]. One line per module; `schema` and `xml` are shared.

pub mod junit;
pub mod libtest_json;
pub mod schema;
pub mod tap;
pub mod test2json;
pub mod trx;
pub mod xml;
