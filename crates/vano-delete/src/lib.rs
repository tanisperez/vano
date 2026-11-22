//! Removing what a scan found: moving items to the system Trash and putting them back
//! ([`trash`]), and deleting them for good ([`delete`]). Blocking calls with no UI, shared by the
//! app and the command line.

pub mod delete;
pub mod linux_trash;
pub mod trash;
