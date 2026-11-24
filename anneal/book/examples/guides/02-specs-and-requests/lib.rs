pub fn apply_permission_mask(requested: u64, allowed_mask: u64) -> u64 {
    requested & allowed_mask
}
