// Copyright 2026 The Fuchsia Authors
//
// Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

//! Representative vertical verification slice of `zerocopy`'s core layout
//! primitives and unsafe abstractions (`src/wrappers.rs`, `src/util/mod.rs`,
//! and `src/pointer/ptr.rs`) verified end-to-end by Anneal and the Lean 4
//! kernel.

#![no_std]
#![allow(dead_code)]

use core::marker::PhantomData;

/// Marker trait for types with no alignment requirement
/// (`align_of::<Self>() == 1`).
#[anneal_spec::unsafe_trait]
pub unsafe trait Unaligned {}

/// Marker trait for types for which the all-zero byte pattern is bit-valid.
#[anneal_spec::unsafe_trait]
pub unsafe trait FromZeros {}

/// Marker trait for types for which every initialized byte pattern is
/// bit-valid.
#[anneal_spec::unsafe_trait]
pub unsafe trait FromBytes: FromZeros {}

/// Marker trait for types without uninitialized padding bytes.
#[anneal_spec::unsafe_trait]
pub unsafe trait IntoBytes {}

/// Trait for types with a runtime bit-validity predicate.
#[anneal_spec::unsafe_trait]
pub unsafe trait TryFromBytes {}

/// A `#[repr(C)]` union used by `transmute_unchecked` and
/// `Unalign::into_inner` for size-preserving byte-level transmutation
/// (`zerocopy/src/util/macros.rs`).
#[repr(C)]
pub union Transmute<Src: Copy, Dst: Copy> {
    pub src: Src,
    pub dst: Dst,
}

macro_rules! unsafe_transmute_via_union {
    ($src:expr, $SrcTy:ty, $DstTy:ty) => {{
        let u = Transmute::<$SrcTy, $DstTy> { src: $src };
        // SAFETY: `Transmute<$SrcTy, $DstTy>` is a `#[repr(C)]` union whose
        // active `src` field has the same byte size as `$DstTy`.
        unsafe { core::mem::transmute::<Transmute<$SrcTy, $DstTy>, $DstTy>(u) }
    }};
}

/// Wrapper type removing alignment requirements from `T`
/// (`align_of::<Unalign<T>>() == 1`, `zerocopy/src/wrappers.rs`).
#[repr(C, packed)]
pub struct Unalign<T>(pub T);

impl Unalign<u16> {
    /// Constructs a new `Unalign<u16>` wrapping `val`.
    pub const fn new(val: u16) -> Unalign<u16> {
        Unalign(val)
    }

    /// Consumes `self` and returns the wrapped `u16` via `Transmute`.
    pub const fn into_inner(self) -> u16 {
        // SAFETY: `Unalign<u16>` is `#[repr(C, packed)]` over `u16` and has
        // the exact same 2-byte representation as `[u8; 2]` and `u16`.
        let bytes: [u8; 2] =
            unsafe { core::mem::transmute::<Unalign<u16>, [u8; 2]>(self) };
        unsafe_transmute_via_union!(bytes, [u8; 2], u16)
    }

    /// Reads the wrapped `u16` by value from `&self` without requiring 2-byte
    /// alignment on the outer wrapper.
    pub const fn get(&self) -> u16 {
        let local_copy = Unalign(0x1234u16);
        let _borrow: &Unalign<u16> = &local_copy;
        let Unalign(val) = *self;
        val
    }

    /// Returns a shared reference to the wrapped `u16` when the caller
    /// guarantees that `self` satisfies `u16`'s 2-byte alignment requirement.
    pub const unsafe fn deref_unchecked(&self) -> &u16 {
        // SAFETY: The caller guarantees 2-byte alignment for `self`, and
        // `Unalign<u16>` has the same 2-byte layout as `u16` at offset 0.
        unsafe { core::mem::transmute::<&Unalign<u16>, &u16>(self) }
    }
}

impl Unalign<u8> {
    /// Returns a shared reference to the wrapped `u8` (`Unalign<u8>` and `u8`
    /// both have alignment 1).
    pub const fn deref(&self) -> &u8 {
        // SAFETY: `u8` has alignment 1, so `self` is always properly aligned
        // and points to the initialized `u8` field at offset 0.
        unsafe { core::mem::transmute::<&Unalign<u8>, &u8>(self) }
    }
}

/// Transmutes a `u16` into its 2-byte little-endian array `[u8; 2]` via
/// `#[repr(C)] union Transmute` (`zerocopy/src/util/mod.rs`).
pub const unsafe fn transmute_unchecked(src: u16) -> [u8; 2] {
    unsafe_transmute_via_union!(src, u16, [u8; 2])
}

/// Rounds `n` down to the largest multiple of power-of-two `align` less than
/// or equal to `n` (`n & !(align - 1)`, `zerocopy/src/util/mod.rs`).
pub const fn round_down_to_next_multiple_of_alignment(
    n: usize,
    align: usize,
) -> usize {
    let mask = align - 1;
    n & !mask
}

/// Computes the trailing padding bytes needed after `len` bytes to reach the
/// next multiple of power-of-two `align` (`!(len.wrapping_sub(1)) & (align - 1)`,
/// `zerocopy/src/util/mod.rs`).
pub const fn padding_needed_for(len: usize, align: usize) -> usize {
    !(len.wrapping_sub(1)) & (align - 1)
}

/// EtherType protocol identifier restricted to IPv4 (`0x0800`) or IPv6
/// (`0x86DD`).
#[anneal_spec::invariant(self.0 == 0x0800 || self.0 == 0x86DD)]
#[repr(C)]
pub struct EtherType(pub u16);

impl EtherType {
    /// Runtime bit-validity check mirroring `TryFromBytes::is_bit_valid` for
    /// `EtherType` (`0x0800` IPv4 or `0x86DD` IPv6).
    pub const fn is_valid_bit_pattern(raw: u16) -> bool {
        raw == 0x0800 || raw == 0x86DD
    }
}

/// Unaligned network packet header composed of three `Unalign<u16>` fields
/// (`zerocopy/src/wrappers.rs`).
#[anneal_spec::invariant(self.payload_len.0 <= 1500)]
#[repr(C)]
pub struct PacketHeader {
    pub src_port: Unalign<u16>,
    pub dst_port: Unalign<u16>,
    pub payload_len: Unalign<u16>,
}

impl PacketHeader {
    /// Updates `src_port` and `dst_port` via simultaneous mutable borrows of
    /// disjoint struct fields while preserving `payload_len`.
    pub const fn set_ports(
        mut self,
        src_port: u16,
        dst_port: u16,
    ) -> PacketHeader {
        let src_ref: &mut Unalign<u16> = &mut self.src_port;
        let dst_ref: &mut Unalign<u16> = &mut self.dst_port;
        *src_ref = Unalign(src_port);
        *dst_ref = Unalign(dst_port);
        let _keep_src_live: u16 = src_ref.0;
        self
    }

    /// Computes the total packet length (`6 + payload_len`) for a validated
    /// `PacketHeader` (`payload_len <= 1500`) by reading `self.payload_len`
    /// through a shared sub-place borrow `&self.payload_len`.
    pub const fn total_packet_len(self) -> u16 {
        let payload_ref: &Unalign<u16> = &self.payload_len;
        let payload: u16 = payload_ref.0;
        6u16 + payload
    }
}

// SAFETY: `PacketHeader` is `#[repr(C)]` and all fields are `Unalign<u16>`
// (`align == 1`), so `PacketHeader` has alignment 1.
#[anneal_spec::unsafe_impl(true)]
unsafe impl Unaligned for PacketHeader {}

// SAFETY: Every field of `PacketHeader` is `Unalign<u16>`, for which the
// all-zero byte pattern `[0u8; 6]` is valid.
#[anneal_spec::unsafe_impl(true)]
unsafe impl FromZeros for PacketHeader {}

// SAFETY: Every 6-byte initialized byte sequence is bit-valid for three
// packed `Unalign<u16>` fields with zero padding.
#[anneal_spec::unsafe_impl(true)]
unsafe impl FromBytes for PacketHeader {}

// SAFETY: `PacketHeader` has size 6 (`3 * 2`) and zero inter-field or trailing
// padding bytes.
#[anneal_spec::unsafe_impl(true)]
unsafe impl IntoBytes for PacketHeader {}

// SAFETY: `PacketHeader` is bit-valid for any 6 initialized bytes and its
// structural `TryFromBytes` candidate check succeeds on all initialized inputs.
#[anneal_spec::unsafe_impl(true)]
unsafe impl TryFromBytes for PacketHeader {}

/// Borrowed pointer wrapper modeling `zerocopy::Ptr<'a, T>`
/// (`zerocopy/src/pointer/ptr.rs`).
#[repr(transparent)]
pub struct Ptr<'a, T: ?Sized + 'a> {
    pub ptr: *const T,
    pub _marker: PhantomData<&'a T>,
}

impl<'a> Ptr<'a, PacketHeader> {
    /// Converts `self` into a shared reference `&'a PacketHeader` when the
    /// pointer carries valid non-null provenance and alignment.
    pub const unsafe fn as_ref(self) -> &'a PacketHeader {
        let raw: *const PacketHeader = self.ptr;
        // SAFETY: The caller guarantees that `self.ptr` is non-null, aligned,
        // and points to a valid `PacketHeader` for lifetime `'a`.
        unsafe {
            core::mem::transmute::<*const PacketHeader, &'a PacketHeader>(raw)
        }
    }
}

impl<'a> Ptr<'a, [u8]> {
    /// Extracts the element count metadata from a wide slice pointer
    /// `*const [u8]` (`zerocopy/src/pointer/inner.rs`).
    pub const fn slice_len(self) -> usize {
        self.ptr.len()
    }
}
