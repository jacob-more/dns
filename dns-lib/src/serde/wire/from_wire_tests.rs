#[cfg(test)]
mod built_in_primitives_test {
    macro_rules! test_int_from_wire {
        ($test_name:ident, $integer:ident, $integer_byte_count:ident) => {
            #[cfg(test)]
            mod $test_name {
                use crate::serde::{
                    const_byte_counts::*,
                    wire::{from_wire::FromWire, read_wire::ReadWire},
                };

                #[test]
                fn per_byte_test() {
                    for u8_value in 0..u8::MAX {
                        for i in 0..($integer_byte_count as usize) {
                            let mut input = [0].repeat($integer_byte_count as usize);
                            input[($integer_byte_count as usize) - 1 - i] = u8_value;
                            let expected = (u8_value as $integer) << (i * 8);

                            let mut read_wire = ReadWire::from_bytes(&input);
                            let output = <$integer>::from_wire_format(&mut read_wire);

                            assert!(output.is_ok());
                            let output = output.unwrap();
                            assert_eq!(expected, output);
                            assert!(read_wire.is_end_reached());
                        }
                    }
                }

                #[test]
                fn max_test() {
                    let input = <$integer>::MAX.to_be_bytes();
                    let expected = <$integer>::MAX;

                    let mut read_wire = ReadWire::from_bytes(&input);
                    let output = <$integer>::from_wire_format(&mut read_wire);

                    assert!(output.is_ok());
                    let output = output.unwrap();
                    assert_eq!(expected, output);
                    assert!(read_wire.is_end_reached());
                }

                #[test]
                fn zero_test() {
                    let input = (0 as $integer).to_be_bytes();
                    let expected = 0 as $integer;

                    let mut read_wire = ReadWire::from_bytes(&input);
                    let output = <$integer>::from_wire_format(&mut read_wire);

                    assert!(output.is_ok());
                    let output = output.unwrap();
                    assert_eq!(expected, output);
                    assert!(read_wire.is_end_reached());
                }

                #[test]
                fn min_test() {
                    let input = <$integer>::MAX.to_be_bytes();
                    let expected = <$integer>::MAX;

                    let mut read_wire = ReadWire::from_bytes(&input);
                    let output = <$integer>::from_wire_format(&mut read_wire);

                    assert!(output.is_ok());
                    let output = output.unwrap();
                    assert_eq!(expected, output);
                    assert!(read_wire.is_end_reached());
                }
            }
        };
    }

    test_int_from_wire!(from_wire_u8, u8, U8_BYTE_COUNT);
    test_int_from_wire!(from_wire_u16, u16, U16_BYTE_COUNT);
    test_int_from_wire!(from_wire_u32, u32, U32_BYTE_COUNT);
    test_int_from_wire!(from_wire_u648, u64, U64_BYTE_COUNT);
    test_int_from_wire!(from_wire_u128, u128, U128_BYTE_COUNT);

    test_int_from_wire!(from_wire_i8, i8, I8_BYTE_COUNT);
    test_int_from_wire!(from_wire_i16, i16, I16_BYTE_COUNT);
    test_int_from_wire!(from_wire_i32, i32, I32_BYTE_COUNT);
    test_int_from_wire!(from_wire_i648, i64, I64_BYTE_COUNT);
    test_int_from_wire!(from_wire_i128, i128, I128_BYTE_COUNT);
}

#[cfg(test)]
mod ux_primitives_test {
    macro_rules! test_ux_from_wire {
        ($test_name:ident, $integer:ident, $integer_byte_count:ident, $super_integer:ident) => {
            #[cfg(test)]
            mod $test_name {
                use ux::$integer;

                use crate::serde::{
                    const_byte_counts::*,
                    wire::{from_wire::FromWire, read_wire::ReadWire},
                };

                #[test]
                fn per_byte_test() {
                    for u8_value in 0..u8::MAX {
                        for i in 0..($integer_byte_count as usize - 1) {
                            let mut input = [0].repeat($integer_byte_count as usize);
                            input[($integer_byte_count as usize) - 1 - i] = u8_value;
                            let expected = <$integer>::new((u8_value as $super_integer) << (i * 8));

                            let mut read_wire = ReadWire::from_bytes(&input);
                            let output = <$integer>::from_wire_format(&mut read_wire);

                            assert!(output.is_ok());
                            let output = output.unwrap();
                            assert_eq!(expected, output);
                            assert!(read_wire.is_end_reached());
                        }
                    }
                }

                #[test]
                fn max_test() {
                    let input = [u8::MAX].repeat($integer_byte_count as usize);
                    let expected = <$integer>::MAX;

                    let mut read_wire = ReadWire::from_bytes(&input);
                    let output = <$integer>::from_wire_format(&mut read_wire);

                    assert!(output.is_ok());
                    let output = output.unwrap();
                    assert_eq!(expected, output);
                    assert!(read_wire.is_end_reached());
                }

                #[test]
                fn zero_test() {
                    let input = [0_u8].repeat($integer_byte_count as usize);
                    let expected = <$integer>::new(0);

                    let mut read_wire = ReadWire::from_bytes(&input);
                    let output = <$integer>::from_wire_format(&mut read_wire);

                    assert!(output.is_ok());
                    let output = output.unwrap();
                    assert_eq!(expected, output);
                    assert!(read_wire.is_end_reached());
                }

                #[test]
                fn min_test() {
                    let input = [0_u8].repeat($integer_byte_count as usize);
                    let expected = <$integer>::MIN;

                    let mut read_wire = ReadWire::from_bytes(&input);
                    let output = <$integer>::from_wire_format(&mut read_wire);

                    assert!(output.is_ok());
                    let output = output.unwrap();
                    assert_eq!(expected, output);
                    assert!(read_wire.is_end_reached());
                }
            }
        };
    }

    macro_rules! test_ix_from_wire {
        ($test_name:ident, $integer:ident, $integer_byte_count:ident, $super_integer:ident) => {
            #[cfg(test)]
            mod $test_name {
                use ux::$integer;

                use crate::serde::{
                    const_byte_counts::*,
                    wire::{from_wire::FromWire, read_wire::ReadWire},
                };

                #[test]
                fn per_byte_test() {
                    for u8_value in 0..u8::MAX {
                        for i in 0..($integer_byte_count as usize - 1) {
                            let mut input = [0].repeat($integer_byte_count as usize);
                            input[($integer_byte_count as usize) - 1 - i] = u8_value;
                            let expected = <$integer>::new((u8_value as $super_integer) << (i * 8));

                            let mut read_wire = ReadWire::from_bytes(&input);
                            let output = <$integer>::from_wire_format(&mut read_wire);

                            assert!(output.is_ok());
                            let output = output.unwrap();
                            assert_eq!(expected, output);
                            assert!(read_wire.is_end_reached());
                        }
                    }
                }

                #[test]
                fn max_test() {
                    let mut input = [u8::MAX].repeat($integer_byte_count as usize);
                    input[0] = u8::MAX / 2;
                    let expected = <$integer>::MAX;

                    let mut read_wire = ReadWire::from_bytes(&input);
                    let output = <$integer>::from_wire_format(&mut read_wire);

                    assert!(output.is_ok());
                    let output = output.unwrap();
                    assert_eq!(expected, output);
                    assert!(read_wire.is_end_reached());
                }

                #[test]
                fn zero_test() {
                    let input = [0_u8].repeat($integer_byte_count as usize);
                    let expected = <$integer>::new(0);

                    let mut read_wire = ReadWire::from_bytes(&input);
                    let output = <$integer>::from_wire_format(&mut read_wire);

                    assert!(output.is_ok());
                    let output = output.unwrap();
                    assert_eq!(expected, output);
                    assert!(read_wire.is_end_reached());
                }

                #[test]
                fn min_test() {
                    let mut input = [0_u8].repeat($integer_byte_count as usize);
                    input[0] = 1 << 7;
                    let expected = <$integer>::MIN;

                    let mut read_wire = ReadWire::from_bytes(&input);
                    let output = <$integer>::from_wire_format(&mut read_wire);

                    assert!(output.is_ok());
                    let output = output.unwrap();
                    assert_eq!(expected, output);
                    assert!(read_wire.is_end_reached());
                }
            }
        };
    }

    test_ux_from_wire!(from_wire_u24, u24, U24_BYTE_COUNT, u32);
    test_ux_from_wire!(from_wire_u40, u40, U40_BYTE_COUNT, u64);
    test_ux_from_wire!(from_wire_u48, u48, U48_BYTE_COUNT, u64);
    test_ux_from_wire!(from_wire_u56, u56, U56_BYTE_COUNT, u64);
    test_ux_from_wire!(from_wire_u72, u72, U72_BYTE_COUNT, u128);
    test_ux_from_wire!(from_wire_u80, u80, U80_BYTE_COUNT, u128);
    test_ux_from_wire!(from_wire_u88, u88, U88_BYTE_COUNT, u128);
    test_ux_from_wire!(from_wire_u96, u96, U96_BYTE_COUNT, u128);
    test_ux_from_wire!(from_wire_u104, u104, U104_BYTE_COUNT, u128);
    test_ux_from_wire!(from_wire_u112, u112, U112_BYTE_COUNT, u128);
    test_ux_from_wire!(from_wire_u120, u120, U120_BYTE_COUNT, u128);

    test_ix_from_wire!(from_wire_i24, i24, I24_BYTE_COUNT, i32);
    test_ix_from_wire!(from_wire_i40, i40, I40_BYTE_COUNT, i64);
    test_ix_from_wire!(from_wire_i48, i48, I48_BYTE_COUNT, i64);
    test_ix_from_wire!(from_wire_i56, i56, I56_BYTE_COUNT, i64);
    test_ix_from_wire!(from_wire_i72, i72, I72_BYTE_COUNT, i128);
    test_ix_from_wire!(from_wire_i80, i80, I80_BYTE_COUNT, i128);
    test_ix_from_wire!(from_wire_i88, i88, I88_BYTE_COUNT, i128);
    test_ix_from_wire!(from_wire_i96, i96, I96_BYTE_COUNT, i128);
    test_ix_from_wire!(from_wire_i104, i104, I104_BYTE_COUNT, i128);
    test_ix_from_wire!(from_wire_i112, i112, I112_BYTE_COUNT, i128);
    test_ix_from_wire!(from_wire_i120, i120, I120_BYTE_COUNT, i128);
}
