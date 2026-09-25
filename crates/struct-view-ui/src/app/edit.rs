//! Tree editing operations used by the graphical interface.

mod fields;
mod paths;
mod structures;
#[cfg(test)]
mod tests;

#[cfg(test)]
use fields::add_child;
pub(super) use fields::{
    add_typed_child_at_path, apply_primitive_edit, edit_child_at_path, is_object_child,
};
pub(super) use paths::{find_node, find_node_mut};
pub(super) use structures::{
    DeleteError, delete_selected_structures, paste_structures_at_path, selected_structures,
};
