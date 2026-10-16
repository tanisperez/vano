//! Vano's domain model, with no IO: the scanned file [`tree`], the [`category`] of each node,
//! the squarified [`treemap`] layout and byte [`size`] units.

pub mod category;
pub mod persist;
pub mod size;
pub mod tree;
pub mod treemap;

pub use category::Category;
pub use tree::{EntryData, Kind, NO_NODE, Node, NodeId, ROOT, Tree};
