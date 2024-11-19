use std::fmt::Debug;

use crate::{serde::wire::write_wire::WriteWire, types::c_domain_name::CompressionMap};

use super::{to_wire::ToWire, from_wire::FromWire};

pub(crate) fn circular_serde_sanity_test<T>(input: T) where T: Debug + ToWire + FromWire + PartialEq {
    // PART 1: No Compression Map

    // Setup
    let wire = &mut [0_u8; u16::MAX as usize * 2];
    let mut wire = WriteWire::from_bytes(wire);
    let mut compression_map = None;
    let expected_serial_length = input.serial_length();

    // Serialize to the wire format.
    // Verify that the output is Ok, that the serial length is what was expected, and that the
    // compression map remains None.
    let result = input.to_wire_format(&mut wire, &mut compression_map);
    assert!(
        result.is_ok(),
        "The output of to_wire_format() is an error.\n{}\n",
        result.unwrap_err(),
    );
    assert_eq!(
        expected_serial_length, wire.current_len() as u16,
        "The expected serial length did not match the actual serial length.\nExpected Serial Length: {}\nActual Serial Length: {}\n",
        expected_serial_length,
        wire.current_len(),
    );
    assert!(
        compression_map.is_none(),
        "The compression map is not None despite the input being None.",
    );

    // Deserialize to the original format.
    // Verify that the output is Ok, that it is the same as the input, that the calculated serial
    // length is what was expected, and that the wire has been consumed.
    let mut wire = wire.as_read_wire();
    let result = T::from_wire_format(&mut wire);
    assert!(
        result.is_ok(),
        "The output of from_wire_format() is an error.\n{}\n",
        result.unwrap_err(),
    );
    let output = result.unwrap();
    assert!(
        input == output,
        "The output does not match the input record.\nExpected Output:\n{:#?}\nActual Output:\n{:#?}\n",
        input, output,
    );
    assert!(
        wire.is_end_reached(),
        "The wire was not fully consumed during deserialization.\nExpected Offset: {}\nActual Offset: {}\n",
        wire.wire_len(), wire.current_offset(),
    );
    let calculated_serial_length = output.serial_length();
    assert_eq!(
        expected_serial_length, calculated_serial_length,
        "The calculated serial length did not match the actual serial length.\nExpected Serial Length: {}\nActual Serial Length: {}\n",
        expected_serial_length, calculated_serial_length,
    );

    // PART 2: Compression Map

    // Setup
    let wire = &mut [0_u8; u16::MAX as usize * 2];
    let mut wire = WriteWire::from_bytes(wire);
    let mut compression_map = Some(CompressionMap::new());
    let expected_serial_length = input.serial_length();

    // Serialize to the wire format.
    // Verify that the output is Ok and that the serial length is not excessive.
    let result = input.to_wire_format(&mut wire, &mut compression_map);
    assert!(
        result.is_ok(),
        "The output of to_wire_format() is an error.\n{}\n",
        result.unwrap_err(),
    );
    assert!(
        expected_serial_length >= wire.current_len() as u16,
        "The expected serial length was less than the actual serial length.\nExpected Maximum Serial Length: {}\nActual Serial Length: {}\n",
        expected_serial_length,
        wire.current_len(),
    );
    assert!(
        compression_map.is_some(),
        "The compression map is None despite the input being Some(_).",
    );

    // Deserialize to the original format.
    // Verify that the output is Ok, that it is the same as the input, that the calculated serial
    // length is what was expected, and that the wire has been consumed.
    let mut wire = wire.as_read_wire();
    let result = T::from_wire_format(&mut wire);
    assert!(
        result.is_ok(),
        "The output of from_wire_format() is an error.\n{}\n",
        result.unwrap_err(),
    );
    let output = result.unwrap();
    assert!(
        input == output,
        "The output does not match the input record.\nExpected Output:\n{:#?}\nActual Output:\n{:#?}\n",
        input, output,
    );
    assert!(
        wire.is_end_reached(),
        "The wire was not fully consumed during deserialization.\nExpected Offset: {}\nActual Offset: {}\n",
        wire.wire_len(), wire.current_offset(),
    );
    let calculated_serial_length = output.serial_length();
    assert_eq!(
        expected_serial_length, calculated_serial_length,
        "The calculated serial length did not match the actual serial length.\nExpected Serial Length: {}\nActual Serial Length: {}\n",
        expected_serial_length, calculated_serial_length,
    );
}

macro_rules! gen_test_circular_serde_sanity_test {
    ($test_name:ident, $test_case:expr) => {
        #[test]
        fn $test_name() {
            $crate::serde::wire::circular_test::circular_serde_sanity_test($test_case)
        }
    }
}
pub(crate) use gen_test_circular_serde_sanity_test;
