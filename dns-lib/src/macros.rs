/// Counts the number of expressions and returns the total as a usize during
/// compile time.
#[macro_export]
macro_rules! count_expressions {
    // This first pattern is an internal implementation detail. It matches any
    // expression and replaces it with the unit type `()`.
    (@replace $_e:expr) => {()};
    ($($expression:expr),* $(,)?) => {<[()]>::len(&[$($crate::count_expressions!(@replace $expression)),*])};
}

/// Sums the results of expressions and returns the total during compile time.
#[macro_export]
macro_rules! sum_expressions {
    ($int_ty:ty; $($expression:expr),* $(,)?) => {{
        const TOTAL: $int_ty = 0 $( + $expression )*;
        TOTAL
    }};
}

/// Given a set of arrays, concatenates all the arrays into a single array at
/// compile time. The order of elements in the resultant array is the same as
/// the order in which they were specified in the arguments.
///
/// Note that the arrays provided as arguments can be fixed-size arrays
/// (e.g., `[u8; 10]`), references to fixed-size arrays (e.g., `&[u8; 10]`), or
/// const slices (e.g., `&[u8]`).
#[macro_export]
macro_rules! concat_arrays {
    ($default:expr; $element_ty:ty; $($array:expr),* $(,)?) => {{
        const CONCATENATED_ARRAY_LEN: usize = 0 $( + $array.len() )*;
        const CONCATENATED_ARRAY: [$element_ty; CONCATENATED_ARRAY_LEN] = {
            let mut concatenated_array = [$default; CONCATENATED_ARRAY_LEN];
            let mut concatenated_array_index = 0;
            $(
                let source_array = $array;
                let mut source_index = 0;
                while source_index < source_array.len() {
                    concatenated_array[concatenated_array_index] = source_array[source_index];
                    source_index += 1;
                    concatenated_array_index += 1;
                }
            )*
            concatenated_array
        };
        CONCATENATED_ARRAY
    }};
}

#[macro_export]
macro_rules! ref_domain {
    ($($labels:expr),+ $(,)?) => {{
        const TOTAL_LABELS: usize = $crate::count_expressions!($($labels),+);
        const TOTAL_OCTETS: usize = (TOTAL_LABELS * $crate::types::domain_name::LENGTH_OCTET_WIDTH)
            + $crate::sum_expressions!(usize; $($labels.as_bytes().len()),+);
        const OCTETS_BUFFER: [u8; TOTAL_OCTETS] = $crate::concat_arrays!(
            0; u8;
            $(
                [$labels.as_bytes().len() as u8],
                $labels.as_bytes(),
            )*
        );
        const LENGTH_OCTETS_BUFFER: [u8; TOTAL_LABELS] = [
            $($labels.as_bytes().len() as u8),*
        ];

        const _: () = $crate::types::domain_name::assert_domain_name_invariants(
            &OCTETS_BUFFER,
            &LENGTH_OCTETS_BUFFER
        );
        // # Safety
        //
        // This macro works from a list of labels, encoded as bytes (which are
        // allowed to exceed the range of ASCII characters). This ensures that
        // each label is a valid wire encoding.
        //
        // >  - The total number of labels must be less than `MAX_LABELS` (128).
        // >  - The total length of this field cannot exceed `MAX_OCTETS` (256)
        // >    bytes.
        // >  - No single label may exceed a length of `MAX_LABEL_OCTETS` (63)
        // >    bytes (not including the length octet).
        // >  - Only the last label may be a root label.
        // >
        // > The `length_octets` must contain the length octets that appear in
        // > `octets` in the same order that they appear in `octets`.
        //
        // All safety checks are performed by `assert_invariants()`, after all
        // expressions have been evaluated. Any inconsistencies caused by
        // evaluating the expressions more than once will be caught by those
        // checks.
        unsafe {
            $crate::types::domain_name::DomainSlice::from_raw_parts(
                &OCTETS_BUFFER,
                &LENGTH_OCTETS_BUFFER
            )
        }
    }};
}

#[macro_export]
macro_rules! domain {
    ($($labels:expr),+ $(,)?) => {
        $crate::ref_domain![$($labels),*].to_domain_vec()
    };
}

#[macro_export]
macro_rules! ref_label {
    ($label:expr $(,)?) => {{
        const OCTETS_BUFFER: &[u8] = $label.as_bytes();

        const _: () = $crate::types::label::assert_domain_name_label_invariants(OCTETS_BUFFER);
        // # Safety
        //
        // > The `octets` must be a valid non-compressed wire-encoded domain name
        // > label, excluding the leading length octet.
        //
        // This macro works from a label, encoded as bytes (which are allowed to
        // exceed the range of ASCII characters). This ensures that the label is
        // a valid wire encoding.
        //
        // > The label may not exceed a length of `MAX_LABEL_OCTETS` (63) bytes
        // > (not including the length octet).
        //
        // All safety checks are performed by `assert_invariants()`, after all
        // expressions have been evaluated
        unsafe { $crate::types::label::RefLabel::from_raw_parts(OCTETS_BUFFER) }
    }};
}

#[macro_export]
macro_rules! label {
    ($label:expr $(,)?) => {
        <$crate::types::label::RefLabel as $crate::types::label::Label>::as_owned(
            $crate::ref_label!($label),
        )
    };
}
