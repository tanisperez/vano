//! What the computer offers to scan, for the home screen and the command line: the connected
//! disks ([`drives`]) and, on Windows, the WSL 2 distributions ([`wsl`]). No UI and no texts:
//! the callers name things in their own language.

pub mod drives;
pub mod wsl;

pub use drives::{Drive, Media, list_drives};
