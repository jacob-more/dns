/// Asserts that a constant boolean expression is `true` at compile time.
///
/// This will invoke the [`panic!`] macro if the provided expression cannot be
/// evaluated to `true` at compile time.
///
/// # Uses
///
/// Unsafe code may rely on `const_assert!` to enforce compile-time invariants
/// that, if violated could lead to unsafety.
///
/// Other use-cases of `const_assert!` include testing and enforcing
/// compile-time invariants in safe code (whose violation cannot result in
/// unsafety).
///
/// # Custom Messages
///
/// This macro has a second form, where a custom panic message can be provided
/// without arguments for formatting. Format argument are supported in theory
/// but have not been stabilized in `const` contexts.
///
/// # Examples
///
/// ```
/// use static_assertions::const_assert;
///
/// // the panic message for these assertions is the stringified value of the
/// // expression given.
/// const_assert!(true);
///
/// const fn some_const_computation() -> bool { true } // a very simple function
///
/// const_assert!(some_const_computation());
///
/// // assert with a custom message
/// const X: bool = true;
/// const_assert!(X, "X wasn't true!");
///
/// const A: u8 = 3;
/// const B: u8 = 27;
/// const_assert!(A + B == 30, "We are testing addition with A and B");
/// ```
///
/// ```compile_fail
/// use static_assertions::const_assert;
///
/// // `const_assert!` causes a compilation error because the input expression
/// // doesn't evaluate to `true`.
/// const_assert!(false);
/// ```
///
/// ```compile_fail
/// use static_assertions::const_assert;
///
/// // `const_assert!` causes a compilation error because the input expression
/// // doesn't evaluate to `true`.
/// const_assert!(false, "We are making an assertion based on a boolean");
/// ```
///
/// ```compile_fail
/// use static_assertions::const_assert;
///
/// const fn some_const_computation() -> bool { false }
///
/// // `const_assert!` causes a compilation error because the input expression
/// // doesn't evaluate to `true`.
/// const_assert!(some_const_computation());
/// ```
///
/// ```compile_fail
/// use static_assertions::const_assert;
///
/// const A: u8 = 1;
/// const B: u8 = 27;
/// // `const_assert!` causes a compilation error because the input expression
/// // doesn't evaluate to `true`.
/// const_assert!(A + B == 30);
/// ```
#[macro_export]
macro_rules! const_assert {
    // Implementation Note:
    //
    // This macro can technically be implemented as just one case,
    //
    // ```
    // ($($tt:tt)*) => {
    //     const _: () = ::std::assert!($($tt)*);
    // };
    // ```
    //
    // The one-case implementation should always work, even if the underlying
    // `assert!` macro's arguments were changed. However, explicitly writing out
    // each of the cases to match `assert!`'s cases helps to generate more
    // helpful documentation AND enforces that the arguments must be `const`
    // expressions.
    ($expression:expr $(,)?) => {
        const _: () = ::std::assert!(const { $expression });
    };
    ($expression:expr, $($arg:tt)+) => {
        const _: () = ::std::assert!(const { $expression }, $($arg)+);
    };
}

/// Asserts that two constant expressions are equal to each other (using
/// [`PartialEq`]) at compile time.
///
/// This will invoke the [`panic!`] macro if the provided expressions don't
/// evaluate to equal values at compile time.
///
/// Like [`const_assert!`], this macro has a second form, where a custom
/// panic message can be provided.
///
/// # Examples
///
/// ```
/// use static_assertions::const_assert_eq;
///
/// const A: u8 = 3;
/// const B: u8 = 1 + 2;
/// const_assert_eq!(A, B);
///
/// const_assert_eq!(A, B, "we are testing addition with A and B");
/// ```
///
/// ```compile_fail
/// use static_assertions::const_assert_eq;
///
/// const A: u8 = 3;
/// const B: u8 = 2;
/// // `A` evaluates to 2 and `B` evaluates to 3, but the assertion checks that
/// // the two expressions evaluate to equivalent values. So, this assertion
/// // causes a compile-time error.
/// const_assert_eq!(A, B);
/// ```
///
/// ```compile_fail
/// use static_assertions::const_assert_eq;
///
/// const A: u8 = 3;
/// const B: u8 = 2;
/// // `A` evaluates to 2 and `B` evaluates to 3, but the assertion checks that
/// // the two expressions evaluate to equivalent values. So, this assertion
/// // causes a compile-time error.
/// const_assert_eq!(A, B, "we are testing that A and B are equal");
/// ```
#[macro_export]
macro_rules! const_assert_eq {
    // Implementation Note:
    //
    // As of the time of implementation (May 2025), this implementation cannot
    // rely on `assert_eq!` because that macro doesn't work in `const` code.
    // Instead, we make use of `==` to check for equality. Unfortunately, the
    // message that gets displayed when there is an error doesn't print the left
    // or right values.
    //
    // There is an issue in the rust-lang repository related to this (see
    // https://github.com/rust-lang/rust/issues/119826), but as noted in the
    // discussion under the issue, supporting `assert_eq!` in `const` contexts
    // is blocked on some of the macro's internal string manipulation not
    // working in `const` contexts.
    ($left:expr, $right:expr $(,)?) => {
        const _: () = ::std::assert!(const { $left } == const { $right });
    };
    ($left:expr, $right:expr, $($arg:tt)+) => {
        const _: () = ::std::assert!(
            const { $left } == const { $right },
            $($arg)+
        );
    };
}

/// Asserts that two constant expressions are not equal to each other (using
/// [`PartialEq`]) at compile time.
///
/// This will invoke the [`panic!`] macro if the provided expressions evaluate
/// to equal values at compile time.
///
/// Like [`const_assert!`], this macro has a second form, where a custom
/// panic message can be provided.
///
/// # Examples
///
/// ```
/// use static_assertions::const_assert_ne;
///
/// const A: u8 = 3;
/// const B: u8 = 2;
/// const_assert_ne!(A, B);
///
/// const_assert_ne!(A, B, "we are testing that A and B are not equal");
/// ```
///
/// ```compile_fail
/// use static_assertions::const_assert_ne;
///
/// const A: u8 = 3;
/// const B: u8 = 1 + 2;
/// // Both `A` and `B` evaluate to 3, but the following asserts that `A` and
/// // `B` are // *not* the same. So, this assertion causes a compile-time
/// // error.
/// const_assert_ne!(A, B);
/// ```
///
/// ```compile_fail
/// use static_assertions::const_assert_ne;
///
/// const A: u8 = 3;
/// const B: u8 = 1 + 2;
/// // Both `A` and `B` evaluate to 3, but the following asserts that `A` and
/// // `B` are // *not* the same. So, this assertion causes a compile-time
/// // error.
/// const_assert_ne!(A, B, "we are testing addition with A and B");
/// ```
#[macro_export]
macro_rules! const_assert_ne {
    // Implementation Note:
    //
    // As of the time of implementation (May 2025), this implementation cannot
    // rely on `assert_ne!` because that macro doesn't work in `const` code.
    // Instead, we make use of `!=` to check for non-equality. Unfortunately,
    // the message that gets displayed when there is an error doesn't print the
    // left or right values.
    //
    // There is an issue in the rust-lang repository related to this (see
    // https://github.com/rust-lang/rust/issues/119826), but as noted in the
    // discussion under the issue, supporting `assert_ne!` in `const` contexts
    // is blocked on some of the macro's internal string manipulation not
    // working in `const` contexts.
    ($left:expr, $right:expr $(,)?) => {
        const _: () = ::std::assert!(const { $left } != const { $right });
    };
    ($left:expr, $right:expr, $($arg:tt)+) => {
        const _: () = ::std::assert!(
            const { $left } != const { $right },
            $($arg)+
        );
    };
}
