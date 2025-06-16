use std::{borrow::Borrow, fmt::{Debug, Display}, hash::{Hash, Hasher}, marker::PhantomData, ops::Deref};

use case_sensitivity::CaseSensitivity;
use tinyvec::{tiny_vec, TinyVec};

use crate::{serde::presentation::parse_chars::{char_token::EscapableChar, non_escaped_to_escaped}, types::ascii::AsciiChar};

use super::ascii::constants::ASCII_PERIOD;


#[derive(Default)]
pub struct CaseSensitive;
#[derive(Default)]
pub struct CaseInsensitive;

impl Display for CaseSensitive {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CaseSensitive")
    }
}

impl Display for CaseInsensitive {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CaseInsensitive")
    }
}

pub(crate) mod case_sensitivity {
    use std::fmt::Display;

    use super::{CaseInsensitive, CaseSensitive};

    pub trait CaseSensitivity: Display + Default {}

    impl CaseSensitivity for CaseSensitive {}
    impl CaseSensitivity for CaseInsensitive {}
}


pub struct OwnedLabel<C: CaseSensitivity> {
    case: PhantomData<C>,
    // A TinyVec with a length of 14 has a size of 24 bytes. This is the same size as a Vec.
    octets: TinyVec<[AsciiChar; 14]>,
}

#[repr(transparent)]
pub struct RefLabel<C: CaseSensitivity> {
    case: PhantomData<C>,
    octets: [AsciiChar]
}

pub trait Label<C: CaseSensitivity> {
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
    fn as_owned(&self) -> OwnedLabel<C> {
        OwnedLabel::from_octets(self.octets().into())
    }

    #[inline]
    fn case_sensitive_ref(&self) -> &RefLabel<CaseSensitive> {
        RefLabel::from_octets(self.octets())
    }

    #[inline]
    fn case_insensitive_ref(&self) -> &RefLabel<CaseInsensitive> {
        RefLabel::from_octets(self.octets())
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

impl<C: CaseSensitivity> Clone for OwnedLabel<C> {
    #[inline]
    fn clone(&self) -> Self {
        Self { case: self.case.clone(), octets: self.octets.clone() }
    }
}

impl<C: CaseSensitivity> Label<C> for OwnedLabel<C> {
    #[inline]
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }
}
impl<C: CaseSensitivity> Label<C> for RefLabel<C> {
    #[inline]
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }
}
impl<C: CaseSensitivity> Label<C> for &RefLabel<C> {
    #[inline]
    fn octets(&self) -> &[AsciiChar] {
        &self.octets
    }
}

impl<C: CaseSensitivity> OwnedLabel<C> {
    #[inline]
    pub(super) fn into_octets(self) -> TinyVec<[AsciiChar; 14]> {
        self.octets
    }
}

impl<C: CaseSensitivity> Deref for OwnedLabel<C> {
    type Target = RefLabel<C>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        RefLabel::from_octets(&self.octets)
    }
}

impl<T, C: CaseSensitivity> AsRef<T> for OwnedLabel<C>
where
    <OwnedLabel<C> as Deref>::Target: AsRef<T>
{
    fn as_ref(&self) -> &T {
        self.deref().as_ref()
    }
}

impl<C1: CaseSensitivity, C2: CaseSensitivity> AsRef<RefLabel<C2>> for RefLabel<C1> {
    #[inline]
    fn as_ref(&self) -> &RefLabel<C2> {
        RefLabel::from_octets(self.octets())
    }
}

impl<C: CaseSensitivity> Borrow<RefLabel<C>> for OwnedLabel<C> {
    #[inline]
    fn borrow(&self) -> &RefLabel<C> {
        &self
    }
}

impl<C: CaseSensitivity> Display for OwnedLabel<C> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self)
    }
}

impl<C: CaseSensitivity> Display for RefLabel<C> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for character in self.iter_escaped() {
            write!(f, "{character}")?;
        }
        Ok(())
    }
}

impl<C: CaseSensitivity> Debug for OwnedLabel<C> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}OwnedLabel: {self}", C::default())
    }
}

impl<C: CaseSensitivity> Debug for RefLabel<C> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}RefLabel: {self}", C::default())
    }
}

impl Eq for OwnedLabel<CaseSensitive> {}
impl Eq for RefLabel<CaseSensitive> {}
impl Eq for OwnedLabel<CaseInsensitive> {}
impl Eq for RefLabel<CaseInsensitive> {}

impl PartialEq<OwnedLabel<CaseSensitive>> for OwnedLabel<CaseSensitive> {
    #[inline]
    fn eq(&self, other: &OwnedLabel<CaseSensitive>) -> bool {
        self.deref().eq(other.deref())
    }
}
impl PartialEq<RefLabel<CaseSensitive>> for OwnedLabel<CaseSensitive> {
    #[inline]
    fn eq(&self, other: &RefLabel<CaseSensitive>) -> bool {
        self.deref().eq(other)
    }
}
impl PartialEq<OwnedLabel<CaseSensitive>> for RefLabel<CaseSensitive> {
    #[inline]
    fn eq(&self, other: &OwnedLabel<CaseSensitive>) -> bool {
        self.eq(other.deref())
    }
}
impl PartialEq<RefLabel<CaseSensitive>> for RefLabel<CaseSensitive> {
    #[inline]
    fn eq(&self, other: &RefLabel<CaseSensitive>) -> bool {
        self.octets().eq(other.octets())
    }
}
impl PartialEq<OwnedLabel<CaseInsensitive>> for OwnedLabel<CaseInsensitive> {
    #[inline]
    fn eq(&self, other: &OwnedLabel<CaseInsensitive>) -> bool {
        self.deref().eq(other.deref())
    }
}
impl PartialEq<RefLabel<CaseInsensitive>> for OwnedLabel<CaseInsensitive> {
    #[inline]
    fn eq(&self, other: &RefLabel<CaseInsensitive>) -> bool {
        self.deref().eq(other)
    }
}
impl PartialEq<OwnedLabel<CaseInsensitive>> for RefLabel<CaseInsensitive> {
    #[inline]
    fn eq(&self, other: &OwnedLabel<CaseInsensitive>) -> bool {
        self.eq(other.deref())
    }
}
impl PartialEq<RefLabel<CaseInsensitive>> for RefLabel<CaseInsensitive> {
    #[inline]
    fn eq(&self, other: &RefLabel<CaseInsensitive>) -> bool {
        self.octets().eq_ignore_ascii_case(other.octets())
    }
}

impl<C: CaseSensitivity> Hash for OwnedLabel<C> where <OwnedLabel<C> as Deref>::Target: Hash {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state);
    }
}
impl Hash for RefLabel<CaseInsensitive> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        let octets = self.octets();
        octets.len().hash(state);
        for character in octets {
            character.to_ascii_lowercase().hash(state);
        }
    }
}
impl Hash for RefLabel<CaseSensitive> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.octets().hash(state);
    }
}

impl<C: CaseSensitivity> OwnedLabel<C> {
    pub fn new_root() -> Self {
        Self { case: PhantomData, octets: tiny_vec![] }
    }

    pub(super) fn from_octets(octets: TinyVec<[AsciiChar; 14]>) -> Self {
        OwnedLabel { case: PhantomData, octets }
    }
}

static ROOT_LABEL: &'static [AsciiChar] = &[];

// TODO: The unsafe blocks for the ref labels are based on code in the standard library. Need to go through and make sure I am upholding the safety guarantees in this particular case.

impl<C: CaseSensitivity> RefLabel<C> {
    pub fn new_root() -> &'static Self {
        Self::from_octets(&ROOT_LABEL)
    }

    pub(super) fn from_octets(octets: &[AsciiChar]) -> &Self {
        unsafe { &*(octets as *const [AsciiChar] as *const RefLabel<C>) }
    }
}
