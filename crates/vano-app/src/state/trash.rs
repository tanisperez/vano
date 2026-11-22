//! The system Trash and permanent deletion live in `vano-delete`, shared with the command line.
//! This module adds what only the UI needs: the translation keys that name the Trash.

pub use vano_delete::trash::*;

/// The translation key of a text that names the Trash. Windows calls it the Recycle Bin, and the
/// word cannot just be swapped in (in Galician even the gender changes), so those texts have a
/// `.windows` variant. `state/locale.rs` tests that no such text skips this.
macro_rules! trash_key {
    ($key:literal) => {
        if cfg!(windows) {
            concat!($key, ".windows")
        } else {
            $key
        }
    };
}
pub(crate) use trash_key;
