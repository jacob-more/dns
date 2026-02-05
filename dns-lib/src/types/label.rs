use std::{
    borrow::Borrow,
    fmt::{Debug, Display},
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut},
};

use tinyvec::{TinyVec, tiny_vec};

use crate::{
    serde::presentation::parse_chars::{char_token::EscapableChar, non_escaped_to_escaped},
    types::ascii::AsciiChar,
};

use super::ascii::constants::ASCII_PERIOD;

/// Assert all invariants required to ensure that the raw parts of a domain name
/// label are correct and that it is safe to pass those arguments to
/// `RefLabel::from_raw_parts()`.
///
/// This function is `const`. This means that it can be called at compile time
/// in a `const` setting where it will cause compilation to fail if the
/// invariants are not met.
///
/// # Panics
///
/// This function will panic if any required invariant to ensure that it is safe
/// to use the arguments with `RefLabel::from_raw_parts()` are not met.
pub const fn assert_domain_name_label_invariants(octets: &[u8]) {
    assert!(
        octets.len() <= (crate::types::domain_name::MAX_LABEL_OCTETS as usize),
        "domain name label specified must be valid but it exceeds MAX_LABEL_OCTETS",
    )
}

pub trait Label: Debug {
    fn octets(&self) -> &[AsciiChar];

    fn len(&self) -> u16 {
        self.octets().len() as u16
    }

    fn is_empty(&self) -> bool {
        self.octets().is_empty()
    }

    fn is_root(&self) -> bool {
        self.is_empty()
    }

    fn as_ref_label(&self) -> &RefLabel {
        RefLabel::from_octets(self.octets())
    }

    fn as_owned(&self) -> OwnedLabel {
        OwnedLabel::from_octets(self.octets().into())
    }

    fn into_owned(self) -> OwnedLabel
    where
        Self: Sized,
    {
        self.as_owned()
    }

    fn as_case_sensitive(&self) -> &CaseSensitive<RefLabel> {
        self.as_ref_label().borrow()
    }

    fn as_case_insensitive(&self) -> &CaseInsensitive<RefLabel> {
        self.as_ref_label().borrow()
    }

    fn iter_escaped<'a>(&'a self) -> impl Iterator<Item = EscapableChar> + 'a {
        non_escaped_to_escaped::NonEscapedIntoEscapedIter::from(self.octets().iter().copied()).map(
            |character| match character {
                EscapableChar::Ascii(ASCII_PERIOD) => EscapableChar::EscapedAscii(ASCII_PERIOD),
                EscapableChar::Ascii(character) => EscapableChar::Ascii(character),
                _ => character,
            },
        )
    }
}

#[derive(Clone, Debug)]
pub struct OwnedLabel {
    // A TinyVec with a length of 14 has a size of 24 bytes. This is the same size as a Vec.
    octets: TinyVec<[AsciiChar; 14]>,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct RefLabel {
    octets: [AsciiChar],
}

impl OwnedLabel {
    pub fn new_root() -> Self {
        Self {
            octets: tiny_vec![],
        }
    }

    // `pub(super)` to keep the internal implementation using `TinyVec` out of
    // the external API.
    pub(super) fn from_octets(octets: TinyVec<[AsciiChar; 14]>) -> Self {
        assert_domain_name_label_invariants(&octets);

        OwnedLabel { octets }
    }
}

impl RefLabel {
    pub const fn new_root() -> &'static Self {
        Self::from_octets(&[])
    }

    /// Creates a reference to `RefLabel` from a slice of bytes.
    ///
    /// # Panics
    ///
    /// The `octets` must be a valid non-compressed wire-encoded domain name
    /// label, excluding the leading length octet.
    ///
    /// The label may not exceed a length of `MAX_LABEL_OCTETS` (63) bytes (not
    /// including the length octet).
    pub const fn from_octets(octets: &[AsciiChar]) -> &Self {
        assert_domain_name_label_invariants(octets);

        // Safety: The call to `assert_domain_name_label_invariants()` verifies
        // that all invariants required to create a valid label are upheld and
        // panics if they are not.
        unsafe { Self::from_raw_parts(octets) }
    }

    /// Creates a mutable reference to `RefLabel` from a slice of bytes.
    ///
    /// # Panics
    ///
    /// The `octets` must be a valid non-compressed wire-encoded domain name
    /// label, excluding the leading length octet.
    ///
    /// The label may not exceed a length of `MAX_LABEL_OCTETS` (63) bytes (not
    /// including the length octet).
    pub const fn from_octets_mut(octets: &mut [AsciiChar]) -> &mut Self {
        assert_domain_name_label_invariants(octets);

        // Safety: The call to `assert_domain_name_label_invariants()` verifies
        // that all invariants required to create a valid label are upheld and
        // panics if they are not.
        unsafe { Self::from_raw_parts_mut(octets) }
    }

    /// Create a `RefLabel` from its raw components.
    ///
    /// # Safety
    ///
    /// The `octets` must be a valid non-compressed wire-encoded domain name
    /// label, excluding the leading length octet.
    ///
    /// The label may not exceed a length of `MAX_LABEL_OCTETS` (63) bytes (not
    /// including the length octet).
    ///
    /// See RFC 1035 for details about this encoding scheme used for the
    /// `octets`.
    pub const unsafe fn from_raw_parts(octets: &[AsciiChar]) -> &Self {
        // TODO: The unsafe blocks for the ref labels are based on code in the
        //       standard library. Need to go through and make sure I am
        //       upholding the safety guarantees in this particular case.
        unsafe { &*(octets as *const [AsciiChar] as *const RefLabel) }
    }

    /// Create a `RefLabel` from its raw components.
    ///
    /// # Safety
    ///
    /// The `octets` must be a valid non-compressed wire-encoded domain name
    /// label, excluding the leading length octet.
    ///
    /// The label may not exceed a length of `MAX_LABEL_OCTETS` (63) bytes (not
    /// including the length octet).
    ///
    /// See RFC 1035 for details about this encoding scheme used for the
    /// `octets`.
    pub const unsafe fn from_raw_parts_mut(octets: &mut [AsciiChar]) -> &mut Self {
        // TODO: The unsafe blocks for the ref labels are based on code in the
        //       standard library. Need to go through and make sure I am
        //       upholding the safety guarantees in this particular case.
        unsafe { &mut *(octets as *mut [AsciiChar] as *mut RefLabel) }
    }
}

impl Label for OwnedLabel {
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }

    fn as_ref_label(&self) -> &RefLabel {
        &*self
    }

    fn as_owned(&self) -> OwnedLabel {
        self.clone()
    }

    fn into_owned(self) -> OwnedLabel {
        self
    }
}
impl Label for &OwnedLabel {
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }

    fn as_ref_label(&self) -> &RefLabel {
        &*self
    }

    fn as_owned(&self) -> OwnedLabel {
        (*self).clone()
    }
}
impl Label for &mut OwnedLabel {
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }

    fn as_ref_label(&self) -> &RefLabel {
        &*self
    }

    fn as_owned(&self) -> OwnedLabel {
        (*self).clone()
    }
}
impl Label for RefLabel {
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }

    fn as_ref_label(&self) -> &RefLabel {
        self
    }

    // TODO: implement `as_owned()` using unsafe to skip invariant check on
    //       `octets` since they should be already valid.
}
impl Label for &RefLabel {
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }

    fn as_ref_label(&self) -> &RefLabel {
        *self
    }

    fn as_owned(&self) -> OwnedLabel {
        (**self).as_owned()
    }
}
impl Label for &mut RefLabel {
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }

    fn as_ref_label(&self) -> &RefLabel {
        *self
    }

    fn as_owned(&self) -> OwnedLabel {
        (**self).as_owned()
    }
}

impl Deref for OwnedLabel {
    type Target = RefLabel;

    fn deref(&self) -> &Self::Target {
        // TODO: use unsafe variant `from_raw_parts()` since &self should always
        //       be a valid label.
        RefLabel::from_octets(&self.octets)
    }
}
impl DerefMut for OwnedLabel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // TODO: use unsafe variant `from_raw_parts_mut()` since &self should
        //       always be a valid label.
        RefLabel::from_octets_mut(&mut self.octets)
    }
}
impl Deref for RefLabel {
    type Target = [AsciiChar];

    fn deref(&self) -> &Self::Target {
        &self.octets
    }
}
impl DerefMut for RefLabel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.octets
    }
}

impl AsRef<OwnedLabel> for OwnedLabel {
    fn as_ref(&self) -> &OwnedLabel {
        self
    }
}
impl<T> AsRef<T> for OwnedLabel
where
    <Self as Deref>::Target: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.deref().as_ref()
    }
}
impl AsRef<RefLabel> for RefLabel {
    fn as_ref(&self) -> &RefLabel {
        self
    }
}
impl<T> AsRef<T> for RefLabel
where
    <Self as Deref>::Target: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.deref().as_ref()
    }
}

impl Borrow<CaseInsensitive<RefLabel>> for OwnedLabel {
    fn borrow(&self) -> &CaseInsensitive<RefLabel> {
        self.deref().borrow()
    }
}
impl Borrow<CaseSensitive<RefLabel>> for OwnedLabel {
    fn borrow(&self) -> &CaseSensitive<RefLabel> {
        self.deref().borrow()
    }
}
impl Borrow<CaseInsensitive<RefLabel>> for &OwnedLabel {
    fn borrow(&self) -> &CaseInsensitive<RefLabel> {
        (*self).deref().borrow()
    }
}
impl Borrow<CaseSensitive<RefLabel>> for &OwnedLabel {
    fn borrow(&self) -> &CaseSensitive<RefLabel> {
        (*self).deref().borrow()
    }
}
impl Borrow<CaseInsensitive<RefLabel>> for RefLabel {
    fn borrow(&self) -> &CaseInsensitive<RefLabel> {
        // TODO: The unsafe blocks for the ref labels are based on code in the
        //       standard library. Need to go through and make sure I am
        //       upholding the safety guarantees in this particular case.
        unsafe { &*(&self.octets as *const [AsciiChar] as *const CaseInsensitive<RefLabel>) }
    }
}
impl Borrow<CaseSensitive<RefLabel>> for RefLabel {
    fn borrow(&self) -> &CaseSensitive<RefLabel> {
        // TODO: The unsafe blocks for the ref labels are based on code in the
        //       standard library. Need to go through and make sure I am
        //       upholding the safety guarantees in this particular case.
        unsafe { &*(&self.octets as *const [AsciiChar] as *const CaseSensitive<RefLabel>) }
    }
}
impl Borrow<CaseInsensitive<RefLabel>> for &RefLabel {
    fn borrow(&self) -> &CaseInsensitive<RefLabel> {
        (*self).borrow()
    }
}
impl Borrow<CaseSensitive<RefLabel>> for &RefLabel {
    fn borrow(&self) -> &CaseSensitive<RefLabel> {
        (*self).borrow()
    }
}

impl Display for OwnedLabel
where
    <OwnedLabel as Deref>::Target: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.deref())
    }
}
impl Display for RefLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for character in self.iter_escaped() {
            write!(f, "{character}")?;
        }
        Ok(())
    }
}

impl From<&RefLabel> for OwnedLabel {
    fn from(value: &RefLabel) -> Self {
        value.as_owned()
    }
}
impl<'a> From<&'a OwnedLabel> for &'a RefLabel {
    fn from(value: &'a OwnedLabel) -> Self {
        value.deref()
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct CaseSensitive<L: Label + ?Sized>(pub L);

#[derive(Debug)]
#[repr(transparent)]
pub struct CaseInsensitive<L: Label + ?Sized>(pub L);

impl<L: Label + ?Sized> Label for CaseSensitive<L> {
    fn octets(&self) -> &[AsciiChar] {
        self.0.octets()
    }

    fn len(&self) -> u16 {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn is_root(&self) -> bool {
        self.0.is_root()
    }

    fn as_ref_label(&self) -> &RefLabel {
        self.0.as_ref_label()
    }

    fn as_owned(&self) -> OwnedLabel {
        self.0.as_owned()
    }

    fn as_case_sensitive(&self) -> &CaseSensitive<RefLabel> {
        self.0.as_case_sensitive()
    }

    fn as_case_insensitive(&self) -> &CaseInsensitive<RefLabel> {
        self.0.as_case_insensitive()
    }

    fn iter_escaped<'a>(&'a self) -> impl Iterator<Item = EscapableChar> + 'a {
        self.0.iter_escaped()
    }
}
impl<L: Label + ?Sized> Label for CaseInsensitive<L> {
    fn octets(&self) -> &[AsciiChar] {
        self.0.octets()
    }

    fn len(&self) -> u16 {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn is_root(&self) -> bool {
        self.0.is_root()
    }

    fn as_ref_label(&self) -> &RefLabel {
        self.0.as_ref_label()
    }

    fn as_owned(&self) -> OwnedLabel {
        self.0.as_owned()
    }

    fn as_case_sensitive(&self) -> &CaseSensitive<RefLabel> {
        self.0.as_case_sensitive()
    }

    fn as_case_insensitive(&self) -> &CaseInsensitive<RefLabel> {
        self.0.as_case_insensitive()
    }

    fn iter_escaped<'a>(&'a self) -> impl Iterator<Item = EscapableChar> + 'a {
        self.0.iter_escaped()
    }
}

impl<L: Label + ?Sized> Deref for CaseSensitive<L> {
    type Target = L;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<L: Label + ?Sized> Deref for CaseInsensitive<L> {
    type Target = L;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<L: Label + ?Sized, T: ?Sized> AsRef<T> for CaseInsensitive<L>
where
    <Self as Deref>::Target: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.deref().as_ref()
    }
}
impl<L: Label + ?Sized, T: ?Sized> AsRef<T> for CaseSensitive<L>
where
    <Self as Deref>::Target: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.deref().as_ref()
    }
}

impl<L: Label + ?Sized> Eq for CaseSensitive<L> {}
impl<L: Label + ?Sized> Eq for CaseInsensitive<L> {}

impl<L1: Label + ?Sized, L2: Label + ?Sized> PartialEq<CaseSensitive<L1>> for CaseSensitive<L2> {
    fn eq(&self, other: &CaseSensitive<L1>) -> bool {
        self.0.octets().eq(other.0.octets())
    }
}
impl<L1: Label + ?Sized, L2: Label + ?Sized> PartialEq<CaseInsensitive<L1>>
    for CaseInsensitive<L2>
{
    fn eq(&self, other: &CaseInsensitive<L1>) -> bool {
        self.0.octets().eq_ignore_ascii_case(other.0.octets())
    }
}

impl<L: Label + ?Sized> Hash for CaseSensitive<L> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.octets().hash(state);
    }
}
impl<L: Label + ?Sized> Hash for CaseInsensitive<L> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let octets = self.0.octets();
        octets.len().hash(state);
        for character in octets {
            character.to_ascii_lowercase().hash(state);
        }
    }
}

impl<L: Label + Clone + ?Sized> Clone for CaseSensitive<L> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<L: Label + Clone + ?Sized> Clone for CaseInsensitive<L> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<L: Label + Copy + ?Sized> Copy for CaseSensitive<L> {}
impl<L: Label + Copy + ?Sized> Copy for CaseInsensitive<L> {}

impl<L: Label> Borrow<CaseInsensitive<RefLabel>> for CaseInsensitive<L> {
    fn borrow(&self) -> &CaseInsensitive<RefLabel> {
        self.as_ref_label().borrow()
    }
}
impl<L: Label> Borrow<CaseSensitive<RefLabel>> for CaseSensitive<L> {
    fn borrow(&self) -> &CaseSensitive<RefLabel> {
        self.as_ref_label().borrow()
    }
}

impl<L: Label + Display + ?Sized> Display for CaseSensitive<L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0)
    }
}
impl<L: Label + Display + ?Sized> Display for CaseInsensitive<L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0)
    }
}

impl<'a> From<&'a RefLabel> for &'a CaseSensitive<RefLabel> {
    fn from(value: &'a RefLabel) -> Self {
        value.borrow()
    }
}
impl<'a> From<&'a RefLabel> for &'a CaseInsensitive<RefLabel> {
    fn from(value: &'a RefLabel) -> Self {
        value.borrow()
    }
}
impl From<OwnedLabel> for CaseSensitive<OwnedLabel> {
    fn from(value: OwnedLabel) -> Self {
        CaseSensitive(value)
    }
}
impl From<OwnedLabel> for CaseInsensitive<OwnedLabel> {
    fn from(value: OwnedLabel) -> Self {
        CaseInsensitive(value)
    }
}
