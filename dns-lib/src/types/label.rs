use std::{fmt::{Debug, Display}, hash::{Hash, Hasher}};

use tinyvec::{tiny_vec, TinyVec};

use crate::{serde::presentation::parse_chars::{char_token::EscapableChar, non_escaped_to_escaped}, types::ascii::AsciiChar};

use super::ascii::constants::ASCII_PERIOD;


pub trait Label: Hash + PartialEq + Eq {
    const MAX_OCTETS: u8 = 63;
    const MIN_OCTETS: u8 = 0;

    fn octets(&self) -> &[AsciiChar];

    #[inline]
    fn len(&self) -> u16 {
        self.octets().len() as u16
    }

    #[inline]
    fn is_root(&self) -> bool {
        self.octets().is_empty()
    }

    #[inline]
    fn as_case_insensitive_owned(&self) -> CaseInsensitiveOwnedLabel {
        CaseInsensitiveOwnedLabel { octets: self.octets().into() }
    }

    #[inline]
    fn as_case_sensitive_owned(&self) -> CaseSensitiveOwnedLabel {
        CaseSensitiveOwnedLabel { octets: self.octets().into() }
    }

    #[inline]
    fn as_case_insensitive(&self) -> CaseInsensitiveRefLabel {
        CaseInsensitiveRefLabel { octets: &self.octets() }
    }

    #[inline]
    fn as_case_sensitive(&self) -> CaseSensitiveRefLabel {
        CaseSensitiveRefLabel { octets: &self.octets() }
    }

    #[inline]
    fn as_lowercase(&self) -> CaseSensitiveOwnedLabel {
        let mut octets = TinyVec::from(self.octets());
        octets.make_ascii_lowercase();
        CaseSensitiveOwnedLabel { octets }
    }

    #[inline]
    fn as_uppercase(&self) -> CaseSensitiveOwnedLabel {
        let mut octets = TinyVec::from(self.octets());
        octets.make_ascii_uppercase();
        CaseSensitiveOwnedLabel { octets }
    }

    #[inline]
    fn iter_escaped<'a>(&'a self) -> impl Iterator<Item = EscapableChar> + 'a {
        non_escaped_to_escaped::NonEscapedIntoEscapedIter::from(self.octets().iter().map(|character| *character))
            .map(|character| match character {
                EscapableChar::Ascii(ASCII_PERIOD) => EscapableChar::EscapedAscii(ASCII_PERIOD),
                EscapableChar::Ascii(character) => EscapableChar::Ascii(character),
                _ => character,
            })
    }
}

pub trait LabelRef<'a>: Label + Sized {
    fn into_octets(self) -> &'a [AsciiChar];

    #[inline]
    fn into_case_insensitive(self) -> CaseInsensitiveRefLabel<'a> {
        CaseInsensitiveRefLabel { octets: self.into_octets() }
    }

    #[inline]
    fn into_case_sensitive(self) -> CaseSensitiveRefLabel<'a> {
        CaseSensitiveRefLabel { octets: self.into_octets() }
    }
}

pub trait LabelOwned: Label + Sized {
    fn into_octets(self) -> TinyVec<[AsciiChar; 14]>;

    #[inline]
    fn into_case_insensitive_owned(self) -> CaseInsensitiveOwnedLabel {
        CaseInsensitiveOwnedLabel { octets: self.into_octets() }
    }

    #[inline]
    fn into_case_sensitive_owned(self) -> CaseSensitiveOwnedLabel {
        CaseSensitiveOwnedLabel { octets: self.into_octets() }
    }
}

#[inline]
fn hash_case_insensitive<H: Hasher>(label: &impl Label, state: &mut H) {
    let octets = label.octets();
    octets.len().hash(state);
    for character in octets {
        character.to_ascii_lowercase().hash(state);
    }
}

#[inline]
fn hash_case_sensitive<H: Hasher>(label: &impl Label, state: &mut H) {
    label.octets().hash(state);
}

#[inline]
fn eq_case_insensitive(l1: &impl Label, l2: &impl Label) -> bool {
    l1.octets().eq_ignore_ascii_case(l2.octets())
}

#[inline]
fn eq_case_sensitive(l1: &impl Label, l2: &impl Label) -> bool {
    l1.octets().eq(l2.octets())
}

#[derive(Clone, Default)]
pub struct CaseSensitiveRefLabel<'a> {
    pub(super) octets: &'a [AsciiChar],
}

impl<'a> Label for CaseSensitiveRefLabel<'a> {
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }
}

impl<'a> LabelRef<'a> for CaseSensitiveRefLabel<'a> {
    fn into_octets(self) -> &'a [AsciiChar] {
        &self.octets
    }
}

impl<'a> Display for CaseSensitiveRefLabel<'a> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for character in self.iter_escaped() {
            write!(f, "{character}")?;
        }
        Ok(())
    }
}

impl<'a> Debug for CaseSensitiveRefLabel<'a> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CaseSensitiveRefLabel: {self}")
    }
}

impl<'a> Eq for CaseSensitiveRefLabel<'a> {}
impl<'a> PartialEq for CaseSensitiveRefLabel<'a> {
    fn eq(&self, other: &Self) -> bool {
        eq_case_sensitive(self, other)
    }
}
impl<'a> Hash for CaseSensitiveRefLabel<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_case_sensitive(self, state)
    }
}

#[derive(Clone, Default)]
pub struct CaseInsensitiveRefLabel<'a> {
    pub(super) octets: &'a [u8],
}

impl<'a> Label for CaseInsensitiveRefLabel<'a> {
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }
}

impl<'a> LabelRef<'a> for CaseInsensitiveRefLabel<'a> {
    fn into_octets(self) -> &'a [AsciiChar] {
        &self.octets
    }
}

impl<'a> Display for CaseInsensitiveRefLabel<'a> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_case_sensitive())
    }
}

impl<'a> Debug for CaseInsensitiveRefLabel<'a> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CaseInsensitiveRefLabel: {self}")
    }
}

impl<'a> Eq for CaseInsensitiveRefLabel<'a> {}
impl<'a> PartialEq for CaseInsensitiveRefLabel<'a> {
    fn eq(&self, other: &Self) -> bool {
        eq_case_insensitive(self, other)
    }
}
impl<'a> Hash for CaseInsensitiveRefLabel<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_case_insensitive(self, state)
    }
}

#[derive(Clone, Default)]
pub struct CaseSensitiveOwnedLabel {
    // A TinyVec with a length of 14 has a size of 24 bytes. This is the same size as a Vec.
    pub(super) octets: TinyVec<[u8; 14]>,
}

impl CaseSensitiveOwnedLabel {
    #[inline]
    pub fn new_root() -> Self {
        Self { octets: tiny_vec![] }
    }
}

impl Label for CaseSensitiveOwnedLabel {
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }
}

impl LabelOwned for CaseSensitiveOwnedLabel {
    fn into_octets(self) -> TinyVec<[AsciiChar; 14]> {
        self.octets
    }
}

impl Display for CaseSensitiveOwnedLabel {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_case_sensitive())
    }
}

impl Debug for CaseSensitiveOwnedLabel {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CaseSensitiveOwnedLabel: {self}")
    }
}

impl Eq for CaseSensitiveOwnedLabel {}
impl PartialEq for CaseSensitiveOwnedLabel {
    fn eq(&self, other: &Self) -> bool {
        eq_case_sensitive(self, other)
    }
}
impl Hash for CaseSensitiveOwnedLabel {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_case_sensitive(self, state)
    }
}

#[derive(Clone, Default)]
pub struct CaseInsensitiveOwnedLabel {
    // A TinyVec with a length of 14 has a size of 24 bytes. This is the same size as a Vec.
    pub(super) octets: TinyVec<[u8; 14]>,
}

impl CaseInsensitiveOwnedLabel {
    #[inline]
    pub fn new_root() -> Self {
        Self { octets: tiny_vec![] }
    }
}

impl Label for CaseInsensitiveOwnedLabel {
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }
}

impl LabelOwned for CaseInsensitiveOwnedLabel {
    fn into_octets(self) -> TinyVec<[AsciiChar; 14]> {
        self.octets
    }
}

impl Display for CaseInsensitiveOwnedLabel {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_case_sensitive())
    }
}

impl Debug for CaseInsensitiveOwnedLabel {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CaseInsensitiveOwnedLabel: {self}")
    }
}

impl Eq for CaseInsensitiveOwnedLabel {}
impl PartialEq for CaseInsensitiveOwnedLabel {
    fn eq(&self, other: &Self) -> bool {
        eq_case_insensitive(self, other)
    }
}
impl Hash for CaseInsensitiveOwnedLabel {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_case_insensitive(self, state)
    }
}
