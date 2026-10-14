//! Shared structural editing operations.

mod fields;
mod paths;
mod structures;
#[cfg(test)]
mod tests;

#[cfg(test)]
use fields::add_child;
pub use fields::{
    add_typed_child_at_path, apply_primitive_edit, edit_child_at_path, is_object_child,
    rename_at_path,
};
pub use paths::{find_node, find_node_mut};
pub use structures::{
    DeleteError, delete_selected_structures, paste_structures_at_path, selected_structures,
};
