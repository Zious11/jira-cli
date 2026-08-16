pub mod board;
pub mod bulk;
pub mod changelog;
pub mod component;
pub mod editmeta;
pub mod issue;
pub mod project;
pub mod sprint;
pub mod team;
pub mod user;
pub mod worklog;

pub use board::*;
pub use bulk::*;
pub use changelog::*;
// `component` is NOT glob-re-exported here: the full-resource
// `component::Component` (BC-8, id: String required) and the embedded
// `issue::Component` (BC-2.3.040, id: Option<String>) share the same
// type name. Consumers must use `crate::types::jira::component::Component`
// to access the full-resource type (BC-2.3.040 Precondition 1 — do not
// conflate).
pub use editmeta::{AllowedValue, EditMeta, EditMetaField, EditMetaFieldSchema};
pub use issue::*;
pub use project::*;
pub use sprint::*;
pub use team::*;
pub use user::User;
pub use worklog::*;
