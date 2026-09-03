#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_field_names)]
pub struct BoundedEventShapeValidator {
    pub(super) max_total_size_bytes: usize,
    pub(super) max_depth: usize,
    pub(super) max_object_keys: usize,
    pub(super) max_array_len: usize,
    pub(super) max_string_len: usize,
}
