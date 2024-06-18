#[cfg(test)]
mod built_in_primitives_test {
    macro_rules! test_int_to_wire {
        ($test_name:ident, $integer:ident, $integer_byte_count:ident) => {
            #[cfg(test)]
            mod $test_name {
                use crate::serde::{wire::{compression_map::CompressionMap, to_wire::ToWire, write_wire::WriteWire}, const_byte_counts::*};

                #[test]
                fn per_byte_test() {
                    for u8_value in 0..u8::MAX {
                        for i in 0..($integer_byte_count as usize) {
                            let input = (u8_value as $integer) << (i * 8);
                            let mut expected = [0].repeat($integer_byte_count as usize);
                            expected[($integer_byte_count as usize) - 1 - i] = u8_value;

                            let mut write_wire_buffer = [0].repeat($integer_byte_count as usize);
                            let mut write_wire = WriteWire::from_bytes(&mut write_wire_buffer);
                            let mut compression_map = Some(CompressionMap::new());
                            let output = input.to_wire_format(&mut write_wire, &mut compression_map);

                            assert!(output.is_ok());
                            assert_eq!(expected.as_slice(), write_wire.current());
                        }
                    }
                }

                #[test]
                fn max_test() {
                    let input = <$integer>::MAX;
                    let expected = <$integer>::MAX.to_be_bytes();

                    let mut write_wire_buffer = [0].repeat($integer_byte_count as usize);
                    let mut write_wire = WriteWire::from_bytes(&mut write_wire_buffer);
                    let mut compression_map = Some(CompressionMap::new());
                    let output = input.to_wire_format(&mut write_wire, &mut compression_map);

                    assert!(output.is_ok());
                    assert_eq!(expected.as_slice(), write_wire.current());
                }

                #[test]
                fn zero_test() {
                    let input = 0 as $integer;
                    let expected = (0 as $integer).to_be_bytes();

                    let mut write_wire_buffer = [0].repeat($integer_byte_count as usize);
                    let mut write_wire = WriteWire::from_bytes(&mut write_wire_buffer);
                    let mut compression_map = Some(CompressionMap::new());
                    let output = input.to_wire_format(&mut write_wire, &mut compression_map);

                    assert!(output.is_ok());
                    assert_eq!(expected.as_slice(), write_wire.current());
                }

                #[test]
                fn min_test() {
                    let input = <$integer>::MIN;
                    let expected = <$integer>::MIN.to_be_bytes();

                    let mut write_wire_buffer = [0].repeat($integer_byte_count as usize);
                    let mut write_wire = WriteWire::from_bytes(&mut write_wire_buffer);
                    let mut compression_map = Some(CompressionMap::new());
                    let output = input.to_wire_format(&mut write_wire, &mut compression_map);

                    assert!(output.is_ok());
                    assert_eq!(expected.as_slice(), write_wire.current());
                }
            }
        }
    }

    test_int_to_wire!(to_wire_u8, u8, U8_BYTE_COUNT);
    test_int_to_wire!(to_wire_u16, u16, U16_BYTE_COUNT);
    test_int_to_wire!(to_wire_u32, u32, U32_BYTE_COUNT);
    test_int_to_wire!(to_wire_u648, u64, U64_BYTE_COUNT);
    test_int_to_wire!(to_wire_u128, u128, U128_BYTE_COUNT);

    test_int_to_wire!(to_wire_i8, i8, I8_BYTE_COUNT);
    test_int_to_wire!(to_wire_i16, i16, I16_BYTE_COUNT);
    test_int_to_wire!(to_wire_i32, i32, I32_BYTE_COUNT);
    test_int_to_wire!(to_wire_i648, i64, I64_BYTE_COUNT);
    test_int_to_wire!(to_wire_i128, i128, I128_BYTE_COUNT);
}

#[cfg(test)]
mod ux_primitives_test {
    macro_rules! test_ux_to_wire {
        ($test_name:ident, $integer:ident, $integer_byte_count:ident, $super_type:ty) => {
            #[cfg(test)]
            mod $test_name {
                use ux::$integer;

                use crate::serde::{wire::{compression_map::CompressionMap, to_wire::ToWire, write_wire::WriteWire}, const_byte_counts::*};

                #[test]
                fn per_byte_test() {
                    for u8_value in 0..u8::MAX {
                        for i in 0..($integer_byte_count as usize) {
                            let input = (<$integer>::new(u8_value as $super_type)) << (i * 8);
                            let mut expected = [0].repeat($integer_byte_count as usize);
                            expected[($integer_byte_count as usize) - 1 - i] = u8_value;

                            let mut write_wire_buffer = [0].repeat($integer_byte_count as usize);
                            let mut write_wire = WriteWire::from_bytes(&mut write_wire_buffer);
                            let mut compression_map = Some(CompressionMap::new());
                            let output = input.to_wire_format(&mut write_wire, &mut compression_map);

                            assert!(output.is_ok());
                            assert_eq!(expected.as_slice(), write_wire.current());
                        }
                    }
                }

                #[test]
                fn max_test() {
                    let input = <$integer>::MAX;
                    let expected = [u8::MAX].repeat($integer_byte_count as usize);

                    let mut write_wire_buffer = [0].repeat($integer_byte_count as usize);
                    let mut write_wire = WriteWire::from_bytes(&mut write_wire_buffer);
                    let mut compression_map = Some(CompressionMap::new());
                    let output = input.to_wire_format(&mut write_wire, &mut compression_map);

                    assert!(output.is_ok());
                    assert_eq!(expected.as_slice(), write_wire.current());
                }

                #[test]
                fn zero_test() {
                    let input = <$integer>::new(0);
                    let expected = [0_u8].repeat($integer_byte_count as usize);

                    let mut write_wire_buffer = [0].repeat($integer_byte_count as usize);
                    let mut write_wire = WriteWire::from_bytes(&mut write_wire_buffer);
                    let mut compression_map = Some(CompressionMap::new());
                    let output = input.to_wire_format(&mut write_wire, &mut compression_map);

                    assert!(output.is_ok());
                    assert_eq!(expected.as_slice(), write_wire.current());
                }

                #[test]
                fn min_test() {
                    let input = <$integer>::MIN;
                    let expected = [0_u8].repeat($integer_byte_count as usize);

                    let mut write_wire_buffer = [0].repeat($integer_byte_count as usize);
                    let mut write_wire = WriteWire::from_bytes(&mut write_wire_buffer);
                    let mut compression_map = Some(CompressionMap::new());
                    let output = input.to_wire_format(&mut write_wire, &mut compression_map);

                    assert!(output.is_ok());
                    assert_eq!(expected.as_slice(), write_wire.current());
                }
            }
        }
    }

    macro_rules! test_ix_to_wire {
        ($test_name:ident, $integer:ident, $integer_byte_count:ident, $super_type:ty) => {
            #[cfg(test)]
            mod $test_name {
                use ux::$integer;

                use crate::serde::{wire::{compression_map::CompressionMap, to_wire::ToWire, write_wire::WriteWire}, const_byte_counts::*};

                #[test]
                fn per_byte_test() {
                    for u8_value in 0..u8::MAX {
                        for i in 0..($integer_byte_count as usize) {
                            let input = (<$integer>::new(u8_value as $super_type)) << (i * 8);
                            let mut expected = [0].repeat($integer_byte_count as usize);
                            expected[($integer_byte_count as usize) - 1 - i] = u8_value;
                            // sign extension
                            if input < <$integer>::new(0) {
                                for j in 0..(($integer_byte_count as usize) - 1 - i) {
                                    expected[j] = u8::MAX;
                                }
                            }

                            let mut write_wire_buffer = [0].repeat($integer_byte_count as usize);
                            let mut write_wire = WriteWire::from_bytes(&mut write_wire_buffer);
                            let mut compression_map = Some(CompressionMap::new());
                            let output = input.to_wire_format(&mut write_wire, &mut compression_map);

                            assert!(output.is_ok());
                            assert_eq!(expected.as_slice(), write_wire.current());
                        }
                    }
                }

                #[test]
                fn max_test() {
                    let input = <$integer>::MAX;
                    let mut expected = [u8::MAX].repeat($integer_byte_count as usize);
                    expected[0] = u8::MAX / 2;

                    let mut write_wire_buffer = [0].repeat($integer_byte_count as usize);
                    let mut write_wire = WriteWire::from_bytes(&mut write_wire_buffer);
                    let mut compression_map = Some(CompressionMap::new());
                    let output = input.to_wire_format(&mut write_wire, &mut compression_map);

                    assert!(output.is_ok());
                    assert_eq!(expected.as_slice(), write_wire.current());
                }

                #[test]
                fn zero_test() {
                    let input = <$integer>::new(0);
                    let expected = [0_u8].repeat($integer_byte_count as usize);

                    let mut write_wire_buffer = [0].repeat($integer_byte_count as usize);
                    let mut write_wire = WriteWire::from_bytes(&mut write_wire_buffer);
                    let mut compression_map = Some(CompressionMap::new());
                    let output = input.to_wire_format(&mut write_wire, &mut compression_map);

                    assert!(output.is_ok());
                    assert_eq!(expected.as_slice(), write_wire.current());
                }

                #[test]
                fn min_test() {
                    let input = <$integer>::MIN;
                    let mut expected = [0_u8].repeat($integer_byte_count as usize);
                    expected[0] = 1 << 7;

                    let mut write_wire_buffer = [0].repeat($integer_byte_count as usize);
                    let mut write_wire = WriteWire::from_bytes(&mut write_wire_buffer);
                    let mut compression_map = Some(CompressionMap::new());
                    let output = input.to_wire_format(&mut write_wire, &mut compression_map);

                    assert!(output.is_ok());
                    assert_eq!(expected.as_slice(), write_wire.current());
                }
            }
        }
    }

    test_ux_to_wire!(from_wire_u24, u24, U24_BYTE_COUNT, u32);
    test_ux_to_wire!(from_wire_u40, u40, U40_BYTE_COUNT, u64);
    test_ux_to_wire!(from_wire_u48, u48, U48_BYTE_COUNT, u64);
    test_ux_to_wire!(from_wire_u56, u56, U56_BYTE_COUNT, u64);
    // FIXME: There is no From<u128> implementation for `ux` types. Until then, cannot implement ToWire for them.
    // test_ux_to_wire!(from_wire_u72, u72, U72_BYTE_COUNT, u128);
    // test_ux_to_wire!(from_wire_u80, u80, U80_BYTE_COUNT, u128);
    // test_ux_to_wire!(from_wire_u88, u88, U88_BYTE_COUNT, u128);
    // test_ux_to_wire!(from_wire_u96, u96, U96_BYTE_COUNT, u128);
    // test_ux_to_wire!(from_wire_u104, u104, U104_BYTE_COUNT, u128);
    // test_ux_to_wire!(from_wire_u112, u112, U112_BYTE_COUNT, u128);
    // test_ux_to_wire!(from_wire_u120, u120, U120_BYTE_COUNT, u128);

    test_ix_to_wire!(from_wire_i24, i24, I24_BYTE_COUNT, i32);
    test_ix_to_wire!(from_wire_i40, i40, I40_BYTE_COUNT, i64);
    test_ix_to_wire!(from_wire_i48, i48, I48_BYTE_COUNT, i64);
    test_ix_to_wire!(from_wire_i56, i56, I56_BYTE_COUNT, i64);
    // FIXME: There is no From<i128> implementation for `ux` types. Until then, cannot implement ToWire for them.
    // test_ix_to_wire!(from_wire_i72, i72, I72_BYTE_COUNT, i128);
    // test_ix_to_wire!(from_wire_i80, i80, I80_BYTE_COUNT, i128);
    // test_ix_to_wire!(from_wire_i88, i88, I88_BYTE_COUNT, i128);
    // test_ix_to_wire!(from_wire_i96, i96, I96_BYTE_COUNT, i128);
    // test_ix_to_wire!(from_wire_i104, i104, I104_BYTE_COUNT, i128);
    // test_ix_to_wire!(from_wire_i112, i112, I112_BYTE_COUNT, i128);
    // test_ix_to_wire!(from_wire_i120, i120, I120_BYTE_COUNT, i128);
}
