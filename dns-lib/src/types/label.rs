use std::{borrow::Borrow, fmt::{Debug, Display}, hash::{Hash, Hasher}, ops::Deref};

use tinyvec::{tiny_vec, TinyVec};

use crate::{serde::presentation::parse_chars::{char_token::EscapableChar, non_escaped_to_escaped}, types::ascii::AsciiChar};

use super::ascii::constants::ASCII_PERIOD;


#[derive(Clone)]
pub struct CaseSensitiveLabel {
    // A TinyVec with a length of 14 has a size of 24 bytes. This is the same size as a Vec.
    octets: TinyVec<[AsciiChar; 14]>,
}

#[derive(Clone)]
pub struct CaseInsensitiveLabel {
    // A TinyVec with a length of 14 has a size of 24 bytes. This is the same size as a Vec.
    octets: TinyVec<[AsciiChar; 14]>,
}

#[repr(transparent)]
pub struct CaseSensitiveLabelRef {
    octets: [AsciiChar]
}

#[repr(transparent)]
pub struct CaseInsensitiveLabelRef {
    octets: [AsciiChar]
}

pub trait Label {
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
    fn as_owned_case_sensitive(&self) -> CaseSensitiveLabel {
        CaseSensitiveLabel::from_octets(self.octets().into())
    }

    #[inline]
    fn as_owned_case_insensitive(&self) -> CaseInsensitiveLabel {
        CaseInsensitiveLabel::from_octets(self.octets().into())
    }

    #[inline]
    fn as_case_sensitive(&self) -> &CaseSensitiveLabelRef {
        CaseSensitiveLabelRef::from_octets(self.octets())
    }

    #[inline]
    fn as_case_insensitive(&self) -> &CaseInsensitiveLabelRef {
        CaseInsensitiveLabelRef::from_octets(self.octets())
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

pub trait OwnedLabel: Label {
    fn into_octets(self) -> TinyVec<[AsciiChar; 14]>;
}

impl Label for CaseSensitiveLabel {
    #[inline]
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }
}

impl Label for CaseInsensitiveLabel {
    #[inline]
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }
}

impl Label for CaseSensitiveLabelRef {
    #[inline]
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }
}

impl Label for CaseInsensitiveLabelRef {
    #[inline]
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }
}

impl Label for &CaseSensitiveLabel {
    #[inline]
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }
}

impl Label for &CaseInsensitiveLabel {
    #[inline]
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }
}

impl Label for &CaseSensitiveLabelRef {
    #[inline]
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }
}

impl Label for &CaseInsensitiveLabelRef {
    #[inline]
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }
}

impl OwnedLabel for CaseSensitiveLabel {
    #[inline]
    fn into_octets(self) -> TinyVec<[AsciiChar; 14]> {
        self.octets
    }
}

impl OwnedLabel for CaseInsensitiveLabel {
    #[inline]
    fn into_octets(self) -> TinyVec<[AsciiChar; 14]> {
        self.octets
    }
}

impl Deref for CaseSensitiveLabel {
    type Target = CaseSensitiveLabelRef;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_case_sensitive()
    }
}

impl Deref for CaseInsensitiveLabel {
    type Target = CaseInsensitiveLabelRef;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_case_insensitive()
    }
}

impl AsRef<CaseSensitiveLabelRef> for CaseSensitiveLabel {
    #[inline]
    fn as_ref(&self) -> &CaseSensitiveLabelRef {
        &self
    }
}

impl AsRef<CaseInsensitiveLabelRef> for CaseInsensitiveLabel {
    #[inline]
    fn as_ref(&self) -> &CaseInsensitiveLabelRef {
        &self
    }
}

impl AsRef<CaseSensitiveLabelRef> for CaseSensitiveLabelRef {
    #[inline]
    fn as_ref(&self) -> &CaseSensitiveLabelRef {
        &self
    }
}

impl AsRef<CaseInsensitiveLabelRef> for CaseInsensitiveLabelRef {
    #[inline]
    fn as_ref(&self) -> &CaseInsensitiveLabelRef {
        &self
    }
}

impl Borrow<CaseSensitiveLabelRef> for CaseSensitiveLabel {
    #[inline]
    fn borrow(&self) -> &CaseSensitiveLabelRef {
        &self
    }
}

impl Borrow<CaseInsensitiveLabelRef> for CaseInsensitiveLabel {
    #[inline]
    fn borrow(&self) -> &CaseInsensitiveLabelRef {
        &self
    }
}

impl Display for CaseSensitiveLabel {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_case_sensitive())
    }
}

impl Display for CaseInsensitiveLabel {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_case_sensitive())
    }
}

impl Display for CaseSensitiveLabelRef {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for character in self.iter_escaped() {
            write!(f, "{character}")?;
        }
        Ok(())
    }
}

impl Display for CaseInsensitiveLabelRef {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_case_sensitive())
    }
}

impl Debug for CaseSensitiveLabel {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CaseSensitiveLabel: {self}")
    }
}

impl Debug for CaseInsensitiveLabel {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CaseInsensitiveLabel: {self}")
    }
}

impl Debug for CaseSensitiveLabelRef {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CaseSensitiveLabelRef: {self}")
    }
}

impl Debug for CaseInsensitiveLabelRef {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CaseInsensitiveLabelRef: {self}")
    }
}

#[inline]
fn hash_case_insensitive<L: Label + ?Sized, H: Hasher>(label: &L, state: &mut H) {
    let octets = label.octets();
    octets.len().hash(state);
    for character in octets {
        character.to_ascii_lowercase().hash(state);
    }
}

#[inline]
fn hash_case_sensitive<L: Label + ?Sized, H: Hasher>(label: &L, state: &mut H) {
    label.octets().hash(state);
}

#[inline]
fn eq_case_insensitive<L1: Label + ?Sized, L2: Label + ?Sized>(l1: &L1, l2: &L2) -> bool {
    l1.octets().eq_ignore_ascii_case(l2.octets())
}

#[inline]
fn eq_case_sensitive<L1: Label + ?Sized, L2: Label + ?Sized>(l1: &L1, l2: &L2) -> bool {
    l1.octets().eq(l2.octets())
}

impl Eq for CaseSensitiveLabel {}
impl<T: Label> PartialEq<T> for CaseSensitiveLabel {
    #[inline]
    fn eq(&self, other: &T) -> bool {
        self.as_ref().eq(other)
    }
}
impl Hash for CaseSensitiveLabel {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state)
    }
}

impl Eq for CaseInsensitiveLabel {}
impl<T: Label> PartialEq<T> for CaseInsensitiveLabel {
    #[inline]
    fn eq(&self, other: &T) -> bool {
        self.as_ref().eq(other)
    }
}
impl Hash for CaseInsensitiveLabel {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state)
    }
}

impl Eq for CaseSensitiveLabelRef {}
impl PartialEq for CaseSensitiveLabelRef {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        eq_case_sensitive(self, other)
    }
}
impl<T: Label> PartialEq<T> for CaseSensitiveLabelRef {
    #[inline]
    fn eq(&self, other: &T) -> bool {
        eq_case_sensitive(self, other)
    }
}
impl Hash for CaseSensitiveLabelRef {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_case_sensitive(self, state)
    }
}

impl Eq for CaseInsensitiveLabelRef {}
impl PartialEq for CaseInsensitiveLabelRef {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        eq_case_insensitive(self, other)
    }
}
impl<T: Label> PartialEq<T> for CaseInsensitiveLabelRef {
    #[inline]
    fn eq(&self, other: &T) -> bool {
        eq_case_insensitive(self, other)
    }
}
impl Hash for CaseInsensitiveLabelRef {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_case_insensitive(self, state)
    }
}

impl CaseSensitiveLabel {
    pub fn new_root() -> Self {
        Self { octets: tiny_vec![] }
    }

    pub(super) fn from_octets(octets: TinyVec<[AsciiChar; 14]>) -> Self {
        CaseSensitiveLabel { octets }
    }
}

impl CaseInsensitiveLabel {
    pub fn new_root() -> Self {
        Self { octets: tiny_vec![] }
    }

    pub(super) fn from_octets(octets: TinyVec<[AsciiChar; 14]>) -> Self {
        CaseInsensitiveLabel { octets }
    }
}

static ROOT_LABEL: &'static [AsciiChar] = &[];

// TODO: The unsafe blocks for the ref labels are based on code in the standard library. Need to go through and make sure I am upholding the safety guarantees in this particular case.

impl CaseSensitiveLabelRef {
    pub fn new_root() -> &'static Self {
        Self::from_octets(&ROOT_LABEL)
    }

    pub(super) fn from_octets(octets: &[AsciiChar]) -> &Self {
        unsafe { &*(octets as *const [AsciiChar] as *const CaseSensitiveLabelRef) }
    }
}

impl CaseInsensitiveLabelRef {
    pub fn new_root() -> &'static Self {
        Self::from_octets(&ROOT_LABEL)
    }

    pub(super) fn from_octets(octets: &[AsciiChar]) -> &Self {
        unsafe { &*(octets as *const [AsciiChar] as *const CaseInsensitiveLabelRef) }
    }
}
