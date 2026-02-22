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
    /// Get the slice of bytes that represent the underlying label, not
    /// including the length octet.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_label, types::label::Label};
    ///
    /// let label = ref_label!("foo");
    /// assert_eq!(label.octets(), &[b'f', b'o', b'o']);
    /// ```
    fn octets(&self) -> &[AsciiChar];

    /// Get the number of bytes that a label contains, not including the length
    /// octet. This is the same as getting the length of `Label::octets()`
    /// although this provides a stronger type guarantee on the integer range.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_label, types::label::Label};
    ///
    /// let label = ref_label!("foo");
    /// assert_eq!(label.len(), 3);
    /// assert_eq!(label.octets().len(), 3);
    /// ```
    fn len(&self) -> u8 {
        self.octets().len() as u8
    }

    /// Returns `true` if a label contains any bytes not including the length
    /// octet or `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_label, types::label::Label};
    ///
    /// assert!(ref_label!("").is_empty());
    ///
    /// assert!(!ref_label!("a").is_empty());
    /// assert!(!ref_label!("ab").is_empty());
    /// assert!(!ref_label!("foo").is_empty());
    /// ```
    fn is_empty(&self) -> bool {
        self.octets().is_empty()
    }

    /// Returns `true` if a label represents a root label or `false` otherwise.
    /// Since a root label is one which has a length octet of zero, it is
    /// guaranteed that `Label::is_root()` and `Label::is_empty()` return the
    /// same result.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_label, types::label::Label};
    ///
    /// assert!(ref_label!("").is_root());
    ///
    /// assert!(!ref_label!("a").is_root());
    /// assert!(!ref_label!("ab").is_root());
    /// assert!(!ref_label!("foo").is_root());
    /// ```
    fn is_root(&self) -> bool {
        self.is_empty()
    }

    /// Returns `true` if a label does not contain any uppercase ASCII
    /// characters or `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_label, types::label::Label};
    ///
    /// assert!(ref_label!("").is_lowercase());
    /// assert!(ref_label!("a 1&").is_lowercase());
    ///
    /// assert!(!ref_label!("A").is_lowercase());
    /// ```
    fn is_lowercase(&self) -> bool {
        self.octets()
            .iter()
            .all(|character| !character.is_ascii_uppercase())
    }

    /// Returns `true` if a label does not contain any lowercase ASCII
    /// characters or `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_label, types::label::Label};
    ///
    /// assert!(ref_label!("").is_uppercase());
    /// assert!(ref_label!("A 1&").is_uppercase());
    ///
    /// assert!(!ref_label!("a").is_uppercase());
    /// ```
    fn is_uppercase(&self) -> bool {
        self.octets()
            .iter()
            .all(|character| !character.is_ascii_lowercase())
    }

    /// Borrows the `Label` as an immutable slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{label, ref_label, types::label::{CaseSensitive, Label, OwnedLabel, RefLabel}};
    ///
    /// let owned_label: OwnedLabel = label!("foo");
    /// let ref_label: &RefLabel = ref_label!("foo");
    ///
    /// assert_eq!(CaseSensitive(owned_label.as_ref_label()), CaseSensitive(ref_label));
    /// ```
    fn as_ref_label(&self) -> &RefLabel {
        RefLabel::from_octets(self.octets())
    }

    /// Creates a `OwnedLabel` from `self`'s octets. This always allocates a new
    /// label, even if `self` is an owned label.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{label, ref_label, types::label::{CaseSensitive, Label, OwnedLabel, RefLabel}};
    ///
    /// let owned_label: OwnedLabel = label!("foo");
    /// let ref_label: &RefLabel = ref_label!("foo");
    ///
    /// assert_eq!(CaseSensitive(ref_label.as_owned()), CaseSensitive(&owned_label));
    /// assert_eq!(CaseSensitive(ref_label.as_owned()), CaseSensitive(&owned_label));
    /// ```
    fn as_owned(&self) -> OwnedLabel {
        OwnedLabel::from_octets(self.octets().into())
    }

    /// Creates a `OwnedLabel` from `self`'s octets. This may re-use the current
    /// allocation if `self` is an `OwnedLabel`, although it may allocate a new
    /// label like `Label::as_owned()` does.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{label, types::label::{CaseSensitive, Label, OwnedLabel}};
    ///
    /// let label_a: OwnedLabel = label!("foo");
    /// let label_b: OwnedLabel = label!("foo");
    ///
    /// assert_eq!(CaseSensitive(label_a.into_owned()), CaseSensitive(&label_b));
    /// // Unlike `Label::as_owned()`, calling `Label::into_owned()` again would
    /// // result in a compile-time error because `label_a` was moved.
    /// //assert_eq!(CaseSensitive(label_a.into_owned()), CaseSensitive(&label_b));
    /// ```
    fn into_owned(self) -> OwnedLabel
    where
        Self: Sized,
    {
        self.as_owned()
    }

    /// Borrows the `Label` as an immutable slice that when compared using
    /// `PartialEq`, uses a case-sensitive form of equality.
    ///
    /// This is especially useful when comparing `OwnedLabel` or `RefLabel`
    /// types because they do not implement `PartialEq`.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_label, types::label::Label};
    ///
    /// assert_eq!(
    ///     ref_label!("FOO").as_case_sensitive(),
    ///     ref_label!("FOO").as_case_sensitive(),
    /// );
    ///
    /// assert_ne!(
    ///     ref_label!("FOO").as_case_sensitive(),
    ///     ref_label!("foo").as_case_sensitive(),
    /// );
    /// assert_ne!(
    ///     ref_label!("FOO").as_case_sensitive(),
    ///     ref_label!("bar").as_case_sensitive(),
    /// );
    /// ```
    fn as_case_sensitive(&self) -> &CaseSensitive<RefLabel> {
        self.as_ref_label().borrow()
    }

    /// Borrows the `Label` as an immutable slice that when compared using
    /// `PartialEq`, uses a case-insensitive form of equality.
    ///
    /// This is especially useful when comparing `OwnedLabel` or `RefLabel`
    /// types because they do not implement `PartialEq`.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_label, types::label::Label};
    ///
    /// assert_eq!(
    ///     ref_label!("FOO").as_case_insensitive(),
    ///     ref_label!("FOO").as_case_insensitive(),
    /// );
    /// assert_eq!(
    ///     ref_label!("FOO").as_case_insensitive(),
    ///     ref_label!("foo").as_case_insensitive(),
    /// );
    ///
    /// assert_ne!(
    ///     ref_label!("FOO").as_case_insensitive(),
    ///     ref_label!("bar").as_case_insensitive(),
    /// );
    /// ```
    fn as_case_insensitive(&self) -> &CaseInsensitive<RefLabel> {
        self.as_ref_label().borrow()
    }

    /// Iterates over the bytes that make up the label, not including the length
    /// octet, and returns the escaped form of each.
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

pub trait MutLabel: Debug {
    /// Get the mutable slice of bytes that represent the underlying label, not
    /// including the length octet.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{label, ref_label, types::label::{CaseSensitive, MutLabel}};
    ///
    /// let mut label = label!("foo");
    /// assert_eq!(label.octets_mut(), &mut [b'f', b'o', b'o']);
    ///
    /// label.octets_mut()[0] = b'b';
    /// assert_eq!(CaseSensitive(label), CaseSensitive(ref_label!("boo")));
    /// ```
    fn octets_mut(&mut self) -> &mut [AsciiChar];

    /// Borrows the `MutLabel` as a mutable slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{label, ref_label, types::label::{CaseSensitive, MutLabel}};
    ///
    /// let mut label = label!("foo");
    /// label.as_ref_mut_label().make_uppercase();
    /// assert_eq!(CaseSensitive(label), CaseSensitive(ref_label!("FOO")));
    /// ```
    fn as_ref_mut_label(&mut self) -> &mut RefLabel {
        RefLabel::from_octets_mut(self.octets_mut())
    }

    /// Converts the body of the label to ASCII uppercase.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{label, ref_label, types::label::{CaseSensitive, MutLabel}};
    ///
    /// let mut label = label!("foo");
    /// label.make_uppercase();
    /// assert_eq!(
    ///     CaseSensitive(label),
    ///     CaseSensitive(ref_label!("FOO")),
    /// );
    /// ```
    fn make_uppercase(&mut self) {
        self.octets_mut().make_ascii_uppercase();
    }

    /// Converts the body of the label to ASCII lowercase.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{label, ref_label, types::label::{CaseSensitive, MutLabel}};
    ///
    /// let mut label = label!("FOO");
    /// label.make_lowercase();
    /// assert_eq!(
    ///     CaseSensitive(label),
    ///     CaseSensitive(ref_label!("foo")),
    /// );
    /// ```
    fn make_lowercase(&mut self) {
        self.octets_mut().make_ascii_lowercase();
    }
}

macro_rules! impl_label {
    ($($label_type:ty)*; {$($pre_self:tt)*} self {$($post_self:tt)*}) => {
        impl<L: Label + ?Sized> Label for $($label_type)* {
            fn octets(&self) -> &[AsciiChar] {
                ($($pre_self)* self $($post_self)*).octets()
            }

            fn len(&self) -> u8 {
                ($($pre_self)* self $($post_self)*).len()
            }

            fn is_empty(&self) -> bool {
                ($($pre_self)* self $($post_self)*).is_empty()
            }

            fn is_root(&self) -> bool {
                ($($pre_self)* self $($post_self)*).is_root()
            }

            fn is_lowercase(&self) -> bool {
                ($($pre_self)* self $($post_self)*).is_lowercase()
            }

            fn is_uppercase(&self) -> bool {
                ($($pre_self)* self $($post_self)*).is_uppercase()
            }

            fn as_ref_label(&self) -> &RefLabel {
                ($($pre_self)* self $($post_self)*).as_ref_label()
            }

            fn as_owned(&self) -> OwnedLabel {
                ($($pre_self)* self $($post_self)*).as_owned()
            }

            fn as_case_sensitive(&self) -> &CaseSensitive<RefLabel> {
                ($($pre_self)* self $($post_self)*).as_case_sensitive()
            }

            fn as_case_insensitive(&self) -> &CaseInsensitive<RefLabel> {
                ($($pre_self)* self $($post_self)*).as_case_insensitive()
            }

            fn iter_escaped<'a>(&'a self) -> impl Iterator<Item = EscapableChar> + 'a {
                ($($pre_self)* self $($post_self)*).iter_escaped()
            }
        }
    };
}
impl_label!(&L;     {*}   self {});
impl_label!(&mut L; {&**} self {});

impl<T: MutLabel + ?Sized> MutLabel for &mut T {
    fn octets_mut(&mut self) -> &mut [AsciiChar] {
        (*self).octets_mut()
    }

    fn as_ref_mut_label(&mut self) -> &mut RefLabel {
        (*self).as_ref_mut_label()
    }

    fn make_uppercase(&mut self) {
        (*self).make_uppercase()
    }

    fn make_lowercase(&mut self) {
        (*self).make_lowercase()
    }
}

#[derive(Clone, Debug)]
pub struct OwnedLabel {
    /// A TinyVec with a length of 14 has a size of 24 bytes. This is the same
    /// size as a Vec.
    octets: TinyVec<[AsciiChar; 14]>,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct RefLabel {
    octets: [AsciiChar],
}

impl OwnedLabel {
    pub fn new() -> Self {
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
    pub const ROOT: &RefLabel = Self::from_octets(&[]);

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

impl MutLabel for OwnedLabel {
    fn octets_mut(&mut self) -> &mut [AsciiChar] {
        &mut self.octets
    }
}
impl MutLabel for RefLabel {
    fn octets_mut(&mut self) -> &mut [AsciiChar] {
        &mut self.octets
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
    T: ?Sized,
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
    T: ?Sized,
    <Self as Deref>::Target: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.deref().as_ref()
    }
}

impl Display for OwnedLabel {
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

macro_rules! impl_borrow_as_case_ref_label {
    ($sensitivity:ident; $($impl_tt:tt)*) => {
        impl $($impl_tt)* {
            fn borrow(&self) -> &$sensitivity<RefLabel> {
                self.as_ref_label().borrow()
            }
        }
    };
}

/// The caller still needs to manually implement `PartialEq` and `Hash`.
/// Everything else is shared.
macro_rules! impl_case_sensitivity {
    ($sensitivity:ident) => {
        #[derive(Debug)]
        #[repr(transparent)]
        pub struct $sensitivity<L: ?Sized>(pub L);

        impl_label!($sensitivity<L>; {} self {.0});

        impl<L: Label + ?Sized> Deref for $sensitivity<L> {
            type Target = L;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<L: Label + ?Sized, T: ?Sized> AsRef<T> for $sensitivity<L>
        where
            <Self as Deref>::Target: AsRef<T>,
        {
            fn as_ref(&self) -> &T {
                self.deref().as_ref()
            }
        }

        impl<L: Label + Clone + ?Sized> Clone for $sensitivity<L> {
            fn clone(&self) -> Self {
                Self(self.0.clone())
            }
        }
        impl<L: Label + Copy + ?Sized> Copy for $sensitivity<L> {}

        impl<L: Label + Display + ?Sized> Display for $sensitivity<L> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", &self.0)
            }
        }

        impl Borrow<$sensitivity<RefLabel>> for RefLabel {
            fn borrow(&self) -> &$sensitivity<RefLabel> {
                // TODO: The unsafe blocks for the ref labels are based on code in the
                //       standard library. Need to go through and make sure I am
                //       upholding the safety guarantees in this particular case.
                unsafe { &*(&self.octets as *const [AsciiChar] as *const $sensitivity<RefLabel>) }
            }
        }
        impl_borrow_as_case_ref_label!($sensitivity; Borrow<$sensitivity<RefLabel>> for &RefLabel);
        impl_borrow_as_case_ref_label!($sensitivity; Borrow<$sensitivity<RefLabel>> for &mut RefLabel);
        impl_borrow_as_case_ref_label!($sensitivity; Borrow<$sensitivity<RefLabel>> for OwnedLabel);
        impl_borrow_as_case_ref_label!($sensitivity; Borrow<$sensitivity<RefLabel>> for &OwnedLabel);
        impl_borrow_as_case_ref_label!($sensitivity; Borrow<$sensitivity<RefLabel>> for &mut OwnedLabel);
        impl_borrow_as_case_ref_label!($sensitivity; <L: Label> Borrow<$sensitivity<RefLabel>> for $sensitivity<L>);

        impl<L: Label + ?Sized> Eq for $sensitivity<L> {}
        impl<L1: Label + ?Sized, L2: Label + ?Sized> PartialEq<$sensitivity<L1>> for &$sensitivity<L2> {
            fn eq(&self, other: &$sensitivity<L1>) -> bool {
                (*self).eq(other)
            }
        }
        impl<L1: Label + ?Sized, L2: Label + ?Sized> PartialEq<$sensitivity<L1>>
            for &mut $sensitivity<L2>
        {
            fn eq(&self, other: &$sensitivity<L1>) -> bool {
                (&**self).eq(other)
            }
        }

        impl<'a> From<&'a RefLabel> for &'a $sensitivity<RefLabel> {
            fn from(value: &'a RefLabel) -> Self {
                value.borrow()
            }
        }
        impl<'a> From<&'a mut RefLabel> for &'a $sensitivity<RefLabel> {
            fn from(value: &'a mut RefLabel) -> Self {
                (&*value).borrow()
            }
        }
        impl From<OwnedLabel> for $sensitivity<OwnedLabel> {
            fn from(value: OwnedLabel) -> Self {
                $sensitivity(value)
            }
        }
        impl<'a> From<&'a OwnedLabel> for $sensitivity<&'a OwnedLabel> {
            fn from(value: &'a OwnedLabel) -> Self {
                $sensitivity(value)
            }
        }
        // TODO: this implementation requires transmutation. Not sure about the
        //       safety on this one.
        //
        //impl<'a> From<&'a OwnedLabel> for &'a $sensitivity<OwnedLabel> {
        //    fn from(value: &'a OwnedLabel) -> Self {
        //        todo!()
        //    }
        //}
    }
}

impl_case_sensitivity!(CaseSensitive);
impl<L1: Label + ?Sized, L2: Label + ?Sized> PartialEq<CaseSensitive<L1>> for CaseSensitive<L2> {
    fn eq(&self, other: &CaseSensitive<L1>) -> bool {
        self.0.octets().eq(other.0.octets())
    }
}
impl<L: Label + ?Sized> Hash for CaseSensitive<L> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.octets().hash(state);
    }
}

impl_case_sensitivity!(CaseInsensitive);
impl<L1: Label + ?Sized, L2: Label + ?Sized> PartialEq<CaseInsensitive<L1>>
    for CaseInsensitive<L2>
{
    fn eq(&self, other: &CaseInsensitive<L1>) -> bool {
        self.0.octets().eq_ignore_ascii_case(other.0.octets())
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

#[cfg(test)]
mod test {
    use rstest::rstest;

    use crate::{
        label, ref_label,
        types::{
            ascii::AsciiChar, domain_name::MAX_LABEL_OCTETS, label::{Label, MutLabel, RefLabel}
        },
    };

    /// Implements `Label` using the default implementations instead of the
    /// underlying specialized implementation. This allows us to verify the
    /// default implementations using specialized types.
    #[derive(Debug)]
    struct DefaultLabel<L: ?Sized>(L);
    impl<L: Label + ?Sized> Label for DefaultLabel<L> {
        fn octets(&self) -> &[AsciiChar] {
            self.0.octets()
        }
    }
    impl<L: MutLabel + ?Sized> MutLabel for DefaultLabel<L> {
        fn octets_mut(&mut self) -> &mut [AsciiChar] {
            self.0.octets_mut()
        }
    }

    macro_rules! repeat_for_label_types {
        (
            #[rstest]
            $(
                #[case(
                    label!( $label_body:literal )
                    $(, $remaining_args:expr)*
                    $(,)?
                )]
            )+
            fn $($def_fn:tt)+
        ) => {
            #[rstest]
            $(
                #[case(
                    label!( $label_body ),
                    $($remaining_args),*
                )]
                #[case(
                    ref_label!( $label_body ),
                    $($remaining_args),*
                )]
                #[case(
                    DefaultLabel(label!( $label_body )),
                    $($remaining_args),*
                )]
                #[case(
                    DefaultLabel(ref_label!( $label_body )),
                    $($remaining_args),*
                )]
            )+
            fn $($def_fn)+
        };
    }

    fn assert_label_properties_match(l1: impl Label, l2: impl Label) {
        assert_eq!(l1.octets(), l2.octets());
        assert_eq!(l1.len(), l2.len());
        assert_eq!(l1.is_empty(), l2.is_empty());
        assert_eq!(l1.is_root(), l2.is_root());
        assert_eq!(l1.is_lowercase(), l2.is_lowercase());
        assert_eq!(l1.is_uppercase(), l2.is_uppercase());
    }

    macro_rules! property_test {
        (@octets $($label_body:literal, $property_value:expr),+ $(,)?) => {
            repeat_for_label_types!(
                #[rstest]
                $( #[case(label!($label_body), $property_value)] )+
                fn octets(#[case] label: impl Label, #[case] expected_octets: &[u8]) {
                    assert_eq!(label.octets(), expected_octets);
                }

            );
        };
        (@len $($label_body:literal, $property_value:expr),+ $(,)?) => {
            repeat_for_label_types!(
                #[rstest]
                $( #[case(label!($label_body), $property_value)] )+
                fn len(#[case] label: impl Label, #[case] expected_len: u8) {
                    assert_eq!(label.len(), expected_len);
                    assert_eq!(label.octets().len(), usize::from(expected_len));
                }
            );
        };
        (@is_empty $($label_body:literal, $property_value:expr),+ $(,)?) => {
            repeat_for_label_types!(
                #[rstest]
                $( #[case(label!($label_body), $property_value)] )+
                fn is_empty(#[case] label: impl Label, #[case] expected_is_empty: bool) {
                    assert_eq!(
                        label.is_empty(),
                        expected_is_empty,
                        "{} is {}expected to be an empty label",
                        label.as_ref_label(),
                        if expected_is_empty { "" } else { "not " }
                    );
                }
            );
        };
        (@is_root $($label_body:literal, $property_value:expr),+ $(,)?) => {
            repeat_for_label_types!(
                #[rstest]
                $( #[case(label!($label_body), $property_value)] )+
                fn is_root(#[case] label: impl Label, #[case] expected_is_root: bool) {
                    assert_eq!(
                        label.is_root(),
                        expected_is_root,
                        "{} is {}expected to be the root label",
                        label.as_ref_label(),
                        if expected_is_root { "" } else { "not " }
                    );
                }
            );
        };
        (@is_lowercase $($label_body:literal, $property_value:expr),+ $(,)?) => {
            repeat_for_label_types!(
                #[rstest]
                $( #[case(label!($label_body), $property_value)] )+
                fn is_lowercase(#[case] label: impl Label, #[case] expected_lowercase: bool) {
                    assert_eq!(
                        label.is_lowercase(),
                        expected_lowercase,
                        "{} is {}expected to be lowercase",
                        label.as_ref_label(),
                        if expected_lowercase { "" } else { "not " }
                    );
                }
            );
        };
        (@is_uppercase $($label_body:literal, $property_value:expr),+ $(,)?) => {
            repeat_for_label_types!(
                #[rstest]
                $( #[case(label!($label_body), $property_value)] )+
                fn is_uppercase(#[case] label: impl Label, #[case] expected_uppercase: bool) {
                    assert_eq!(
                        label.is_uppercase(),
                        expected_uppercase,
                        "{} is {}expected to be uppercase",
                        label.as_ref_label(),
                        if expected_uppercase { "" } else { "not " }
                    );
                }
            );
        };
        (@as $call:ident $($label_body:literal),+ $(,)?) => {
            // Test that properties don't change when converted into some other
            // concrete type or a slice is taken.
            repeat_for_label_types!(
                #[rstest]
                $( #[case(label!($label_body), label!($label_body))] )+
                fn $call(#[case] label: impl Label, #[case] expected_label: impl Label) {
                    assert_label_properties_match(label.$call(), expected_label);
                }
            );
        };
        (@as_default_impl $($label_body:literal),+ $(,)?) => {
            // Test that properties of concrete type match that of the default
            // implementation.
            repeat_for_label_types!(
                #[rstest]
                $( #[case(label!($label_body), DefaultLabel(ref_label!($label_body)))] )+
                fn as_default_impl(#[case] label: impl Label, #[case] expected_label: impl Label) {
                    assert_label_properties_match(label, expected_label);
                }
            );
        };
        (@case_sensitivity $($label_body:literal, $is_lowercase_value:expr, $is_uppercase_value:expr),+ $(,)?) => {
            repeat_for_label_types!(
                #[rstest]
                $( #[case(label!($label_body), label!($label_body), $is_lowercase_value, $is_uppercase_value)] )+
                fn test_case_sensitivity(
                    #[case] l1: impl Label,
                    #[case] l2: impl Label,
                    #[case] is_lowercase: bool,
                    #[case] is_uppercase: bool
                ) {
                    assert_eq!(l1.as_case_sensitive(), l2.as_case_sensitive());
                    assert_eq!(l1.as_case_insensitive(), l2.as_case_insensitive());

                    let mut lower = l2.as_owned();
                    let mut upper = l2.as_owned();
                    lower.make_lowercase();
                    upper.make_uppercase();
                    assert_eq!(l1.as_case_insensitive(), lower.as_case_insensitive());
                    assert_eq!(l1.as_case_insensitive(), upper.as_case_insensitive());

                    match (is_lowercase, is_uppercase) {
                        (true, true) => {
                            assert_eq!(l1.as_case_sensitive(), lower.as_case_sensitive());
                            assert_eq!(l1.as_case_sensitive(), upper.as_case_sensitive());
                        },
                        (true, false) => {
                            assert_eq!(l1.as_case_sensitive(), lower.as_case_sensitive());
                            assert_ne!(l1.as_case_sensitive(), upper.as_case_sensitive());
                        },
                        (false, true) => {
                            assert_ne!(l1.as_case_sensitive(), lower.as_case_sensitive());
                            assert_eq!(l1.as_case_sensitive(), upper.as_case_sensitive());
                        },
                        (false, false) => {
                            assert_ne!(l1.as_case_sensitive(), lower.as_case_sensitive());
                            assert_ne!(l1.as_case_sensitive(), upper.as_case_sensitive());
                        },
                    }
                }
            );
        };
        (
            $(
                label!( $label_body:literal ): {
                    octets: $octets_value:expr,
                    len: $len_value:expr,
                    is_empty: $is_empty_value:expr,
                    is_root: $is_root_value:expr,
                    is_lowercase: $is_lowercase_value:expr,
                    is_uppercase: $is_uppercase_value:expr
                    $(,)?
                }
            ),+
            $(,)?
        ) => {
            property_test!(@octets       $($label_body, $octets_value      ),+);
            property_test!(@len          $($label_body, $len_value         ),+);
            property_test!(@is_empty     $($label_body, $is_empty_value    ),+);
            property_test!(@is_root      $($label_body, $is_root_value     ),+);
            property_test!(@is_lowercase $($label_body, $is_lowercase_value),+);
            property_test!(@is_uppercase $($label_body, $is_uppercase_value),+);

            property_test!(@as as_ref_label        $($label_body),+);
            property_test!(@as as_owned            $($label_body),+);
            property_test!(@as into_owned          $($label_body),+);
            property_test!(@as as_case_sensitive   $($label_body),+);
            property_test!(@as as_case_insensitive $($label_body),+);

            property_test!(@as_default_impl $($label_body),+);
            property_test!(@case_sensitivity $($label_body, $is_lowercase_value, $is_uppercase_value),+);
        };
    }

    property_test!(
        label!(""): {
            octets: &[],
            len: 0,
            is_empty: true,
            is_root: true,
            is_lowercase: true,
            is_uppercase: true,
        },
        label!("a"): {
            octets: &[b'a'],
            len: 1,
            is_empty: false,
            is_root: false,
            is_lowercase: true,
            is_uppercase: false,
        },
        label!("A"): {
            octets: &[b'A'],
            len: 1,
            is_empty: false,
            is_root: false,
            is_lowercase: false,
            is_uppercase: true,
        },
        label!(" "): {
            octets: &[b' '],
            len: 1,
            is_empty: false,
            is_root: false,
            is_lowercase: true,
            is_uppercase: true,
        },
        label!("\t"): {
            octets: &[b'\t'],
            len: 1,
            is_empty: false,
            is_root: false,
            is_lowercase: true,
            is_uppercase: true,
        },
        label!("\0"): {
            octets: &[b'\0'],
            len: 1,
            is_empty: false,
            is_root: false,
            is_lowercase: true,
            is_uppercase: true,
        },
        label!("ab"): {
            octets: &[b'a', b'b'],
            len: 2,
            is_empty: false,
            is_root: false,
            is_lowercase: true,
            is_uppercase: false,
        },
        label!("AB"): {
            octets: &[b'A', b'B'],
            len: 2,
            is_empty: false,
            is_root: false,
            is_lowercase: false,
            is_uppercase: true,
        },
        label!("Ab"): {
            octets: &[b'A', b'b'],
            len: 2,
            is_empty: false,
            is_root: false,
            is_lowercase: false,
            is_uppercase: false,
        },
        label!("aB"): {
            octets: &[b'a', b'B'],
            len: 2,
            is_empty: false,
            is_root: false,
            is_lowercase: false,
            is_uppercase: false,
        },
        label!("foo"): {
            octets: &[b'f', b'o', b'o'],
            len: 3,
            is_empty: false,
            is_root: false,
            is_lowercase: true,
            is_uppercase: false,
        },
        label!("Foo"): {
            octets: &[b'F', b'o', b'o'],
            len: 3,
            is_empty: false,
            is_root: false,
            is_lowercase: false,
            is_uppercase: false,
        },
        label!("FOO"): {
            octets: &[b'F', b'O', b'O'],
            len: 3,
            is_empty: false,
            is_root: false,
            is_lowercase: false,
            is_uppercase: true,
        },
        label!("\tab"): {
            octets: &[b'\t', b'a', b'b'],
            len: 3,
            is_empty: false,
            is_root: false,
            is_lowercase: true,
            is_uppercase: false,
        },
        label!("a\tb"): {
            octets: &[b'a', b'\t', b'b'],
            len: 3,
            is_empty: false,
            is_root: false,
            is_lowercase: true,
            is_uppercase: false,
        },
        label!("ab\t"): {
            octets: &[b'a', b'b', b'\t'],
            len: 3,
            is_empty: false,
            is_root: false,
            is_lowercase: true,
            is_uppercase: false,
        },
        label!("\0ab"): {
            octets: &[b'\0', b'a', b'b'],
            len: 3,
            is_empty: false,
            is_root: false,
            is_lowercase: true,
            is_uppercase: false,
        },
        label!("a\0b"): {
            octets: &[b'a', b'\0', b'b'],
            len: 3,
            is_empty: false,
            is_root: false,
            is_lowercase: true,
            is_uppercase: false,
        },
        label!("ab\0"): {
            octets: &[b'a', b'b', b'\0'],
            len: 3,
            is_empty: false,
            is_root: false,
            is_lowercase: true,
            is_uppercase: false,
        },
        label!("abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz01234567890"): {
            octets: &[
                b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j',
                b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't',
                b'u', b'v', b'w', b'x', b'y', b'z', b'a', b'b', b'c', b'd',
                b'e', b'f', b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n',
                b'o', b'p', b'q', b'r', b's', b't', b'u', b'v', b'w', b'x',
                b'y', b'z', b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7',
                b'8', b'9', b'0'
            ],
            len: 63,
            is_empty: false,
            is_root: false,
            is_lowercase: true,
            is_uppercase: false,
        },
        label!("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ01234567890"): {
            octets: &[
                b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j',
                b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't',
                b'u', b'v', b'w', b'x', b'y', b'z', b'A', b'B', b'C', b'D',
                b'E', b'F', b'G', b'H', b'I', b'J', b'K', b'L', b'M', b'N',
                b'O', b'P', b'Q', b'R', b'S', b'T', b'U', b'V', b'W', b'X',
                b'Y', b'Z', b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7',
                b'8', b'9', b'0'
            ],
            len: 63,
            is_empty: false,
            is_root: false,
            is_lowercase: false,
            is_uppercase: false,
        },
        label!("ABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZ01234567890"): {
            octets: &[
                b'A', b'B', b'C', b'D', b'E', b'F', b'G', b'H', b'I', b'J',
                b'K', b'L', b'M', b'N', b'O', b'P', b'Q', b'R', b'S', b'T',
                b'U', b'V', b'W', b'X', b'Y', b'Z', b'A', b'B', b'C', b'D',
                b'E', b'F', b'G', b'H', b'I', b'J', b'K', b'L', b'M', b'N',
                b'O', b'P', b'Q', b'R', b'S', b'T', b'U', b'V', b'W', b'X',
                b'Y', b'Z', b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7',
                b'8', b'9', b'0'
            ],
            len: 63,
            is_empty: false,
            is_root: false,
            is_lowercase: false,
            is_uppercase: true,
        },
    );

    #[test]
    #[should_panic]
    fn from_octets_panics() {
        let bytes = &[0; (MAX_LABEL_OCTETS as usize).checked_add(1).unwrap()];
        RefLabel::from_octets(bytes);
    }

    #[test]
    #[should_panic]
    fn from_octets_mut_panics() {
        let bytes = &mut [0; (MAX_LABEL_OCTETS as usize).checked_add(1).unwrap()];
        RefLabel::from_octets_mut(bytes);
    }

    /// A type that implements `Label` but which exceeds the maximum octet limit
    /// for labels. This lets us test that illegal conversions are prevented by
    /// the default implementation.
    #[derive(Debug)]
    struct InvalidLabel {
        octets: [u8; (MAX_LABEL_OCTETS as usize).checked_add(1).unwrap()]
    }
    impl InvalidLabel {
        pub fn new() -> Self {
            Self {
                octets: [0; (MAX_LABEL_OCTETS as usize).checked_add(1).unwrap()]
            }
        }
    }
    impl Label for InvalidLabel {
        fn octets(&self) -> &[AsciiChar] {
            &self.octets
        }
    }

    #[test]
    #[should_panic]
    fn as_ref_label_panics() {
        InvalidLabel::new().as_ref_label();
    }

    #[test]
    #[should_panic]
    fn as_owned_panics() {
        InvalidLabel::new().as_owned();
    }

    #[test]
    #[should_panic]
    fn into_owned_panics() {
        InvalidLabel::new().into_owned();
    }

    #[test]
    #[should_panic]
    fn as_case_sensitive_panics() {
        InvalidLabel::new().as_case_sensitive();
    }

    #[test]
    #[should_panic]
    fn as_case_insensitive_panics() {
        InvalidLabel::new().as_case_insensitive();
    }
}
