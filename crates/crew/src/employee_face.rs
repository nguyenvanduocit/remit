//! An employee's face (ADR-0093): a mark the server derives from who the employee is, so the owner knows a lineage at a
//! glance in every chain, the tree and the inbox. The shape and the hue come from the lineage (the template it was hired
//! from), so siblings read as one family; the accessory comes from the employee itself, so two siblings side by side are
//! told apart. It is derived from ids that never change, so it is the same on every read and nothing stores it. Pure.

use model::Employee;
use sha2::{Digest, Sha256};
use wire::app::{Face, FaceAccessory, FaceShape};

const SHAPES: [FaceShape; 6] = [
    FaceShape::Circle,
    FaceShape::Square,
    FaceShape::Hexagon,
    FaceShape::Triangle,
    FaceShape::Diamond,
    FaceShape::Drop,
];

const ACCESSORIES: [FaceAccessory; 6] = [
    FaceAccessory::Antenna,
    FaceAccessory::Halo,
    FaceAccessory::Visor,
    FaceAccessory::Spark,
    FaceAccessory::Leaf,
    FaceAccessory::Bow,
];

/// The first eight bytes of the SHA-256 of s, as a number: stable across builds and platforms, unlike std's hasher.
fn digest(s: &str) -> u64 {
    let sum = Sha256::digest(s.as_bytes());
    let mut b = [0u8; 8];
    b.copy_from_slice(sum.get(..8).unwrap_or(&[0; 8]));
    u64::from_be_bytes(b)
}

/// The item of xs that n picks.
fn pick<T: Copy>(xs: &[T; 6], n: u64) -> T {
    let [first, ..] = *xs;
    let i = usize::try_from(n % 6).unwrap_or_default();
    xs.get(i).copied().unwrap_or(first)
}

/// Employee e's face.
pub(crate) fn face_of(e: &Employee) -> Face {
    let lineage = digest(&e.template);
    Face {
        shape: pick(&SHAPES, lineage),
        hue: i64::try_from((lineage >> 8) % 360).unwrap_or_default(),
        accessory: pick(&ACCESSORIES, digest(&e.id)),
    }
}
