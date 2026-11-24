pub fn sanitize_flags(flags: u64, force_mask: u64, allow_mask: u64) -> u64 {
    (flags | force_mask) & allow_mask
}
