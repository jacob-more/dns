use std::net::{Ipv4Addr, Ipv6Addr};

use mac_address::MacAddress;

/// https://datatracker.ietf.org/doc/html/rfc1035#section-5
pub trait ToPresentation {
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>);
}

// #################### BUILT-IN PRIMITIVE TYPES ####################

macro_rules! std_to_token_impl {
    ($int_type:ty) => {
        impl ToPresentation for $int_type {
            #[inline]
            fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
                out_buffer.push(self.to_string())
            }
        }
    }
}

std_to_token_impl!(u8);
std_to_token_impl!(u16);
std_to_token_impl!(u32);
std_to_token_impl!(u64);
std_to_token_impl!(u128);

std_to_token_impl!(i8);
std_to_token_impl!(i16);
std_to_token_impl!(i32);
std_to_token_impl!(i64);
std_to_token_impl!(i128);

// #################### UX PRIMITIVE TYPES ####################

macro_rules! ux_to_token_impl {
    ($int_type:ident) => {
        use ux::$int_type;

        impl ToPresentation for $int_type {
            #[inline]
            fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
                out_buffer.push(self.to_string())
            }
        }
    }
}

ux_to_token_impl!(u1);
ux_to_token_impl!(u2);
ux_to_token_impl!(u3);
ux_to_token_impl!(u4);
ux_to_token_impl!(u5);
ux_to_token_impl!(u6);
ux_to_token_impl!(u7);
ux_to_token_impl!(u9);
ux_to_token_impl!(u10);
ux_to_token_impl!(u11);
ux_to_token_impl!(u12);
ux_to_token_impl!(u13);
ux_to_token_impl!(u14);
ux_to_token_impl!(u15);
ux_to_token_impl!(u17);
ux_to_token_impl!(u18);
ux_to_token_impl!(u19);
ux_to_token_impl!(u20);
ux_to_token_impl!(u21);
ux_to_token_impl!(u22);
ux_to_token_impl!(u23);
ux_to_token_impl!(u24);
ux_to_token_impl!(u25);
ux_to_token_impl!(u26);
ux_to_token_impl!(u27);
ux_to_token_impl!(u28);
ux_to_token_impl!(u29);
ux_to_token_impl!(u30);
ux_to_token_impl!(u31);
ux_to_token_impl!(u33);
ux_to_token_impl!(u34);
ux_to_token_impl!(u35);
ux_to_token_impl!(u36);
ux_to_token_impl!(u37);
ux_to_token_impl!(u38);
ux_to_token_impl!(u39);
ux_to_token_impl!(u40);
ux_to_token_impl!(u41);
ux_to_token_impl!(u42);
ux_to_token_impl!(u43);
ux_to_token_impl!(u44);
ux_to_token_impl!(u45);
ux_to_token_impl!(u46);
ux_to_token_impl!(u47);
ux_to_token_impl!(u48);
ux_to_token_impl!(u49);
ux_to_token_impl!(u50);
ux_to_token_impl!(u51);
ux_to_token_impl!(u52);
ux_to_token_impl!(u53);
ux_to_token_impl!(u54);
ux_to_token_impl!(u55);
ux_to_token_impl!(u56);
ux_to_token_impl!(u57);
ux_to_token_impl!(u58);
ux_to_token_impl!(u59);
ux_to_token_impl!(u60);
ux_to_token_impl!(u61);
ux_to_token_impl!(u62);
ux_to_token_impl!(u63);
ux_to_token_impl!(u65);
ux_to_token_impl!(u66);
ux_to_token_impl!(u67);
ux_to_token_impl!(u68);
ux_to_token_impl!(u69);
ux_to_token_impl!(u70);
ux_to_token_impl!(u71);
ux_to_token_impl!(u72);
ux_to_token_impl!(u73);
ux_to_token_impl!(u74);
ux_to_token_impl!(u75);
ux_to_token_impl!(u76);
ux_to_token_impl!(u77);
ux_to_token_impl!(u78);
ux_to_token_impl!(u79);
ux_to_token_impl!(u80);
ux_to_token_impl!(u81);
ux_to_token_impl!(u82);
ux_to_token_impl!(u83);
ux_to_token_impl!(u84);
ux_to_token_impl!(u85);
ux_to_token_impl!(u86);
ux_to_token_impl!(u87);
ux_to_token_impl!(u88);
ux_to_token_impl!(u89);
ux_to_token_impl!(u90);
ux_to_token_impl!(u91);
ux_to_token_impl!(u92);
ux_to_token_impl!(u93);
ux_to_token_impl!(u94);
ux_to_token_impl!(u95);
ux_to_token_impl!(u96);
ux_to_token_impl!(u97);
ux_to_token_impl!(u98);
ux_to_token_impl!(u99);
ux_to_token_impl!(u100);
ux_to_token_impl!(u101);
ux_to_token_impl!(u102);
ux_to_token_impl!(u103);
ux_to_token_impl!(u104);
ux_to_token_impl!(u105);
ux_to_token_impl!(u106);
ux_to_token_impl!(u107);
ux_to_token_impl!(u108);
ux_to_token_impl!(u109);
ux_to_token_impl!(u110);
ux_to_token_impl!(u111);
ux_to_token_impl!(u112);
ux_to_token_impl!(u113);
ux_to_token_impl!(u114);
ux_to_token_impl!(u115);
ux_to_token_impl!(u116);
ux_to_token_impl!(u117);
ux_to_token_impl!(u118);
ux_to_token_impl!(u119);
ux_to_token_impl!(u120);
ux_to_token_impl!(u121);
ux_to_token_impl!(u122);
ux_to_token_impl!(u123);
ux_to_token_impl!(u124);
ux_to_token_impl!(u125);
ux_to_token_impl!(u126);
ux_to_token_impl!(u127);

ux_to_token_impl!(i1);
ux_to_token_impl!(i2);
ux_to_token_impl!(i3);
ux_to_token_impl!(i4);
ux_to_token_impl!(i5);
ux_to_token_impl!(i6);
ux_to_token_impl!(i7);
ux_to_token_impl!(i9);
ux_to_token_impl!(i10);
ux_to_token_impl!(i11);
ux_to_token_impl!(i12);
ux_to_token_impl!(i13);
ux_to_token_impl!(i14);
ux_to_token_impl!(i15);
ux_to_token_impl!(i17);
ux_to_token_impl!(i18);
ux_to_token_impl!(i19);
ux_to_token_impl!(i20);
ux_to_token_impl!(i21);
ux_to_token_impl!(i22);
ux_to_token_impl!(i23);
ux_to_token_impl!(i24);
ux_to_token_impl!(i25);
ux_to_token_impl!(i26);
ux_to_token_impl!(i27);
ux_to_token_impl!(i28);
ux_to_token_impl!(i29);
ux_to_token_impl!(i30);
ux_to_token_impl!(i31);
ux_to_token_impl!(i33);
ux_to_token_impl!(i34);
ux_to_token_impl!(i35);
ux_to_token_impl!(i36);
ux_to_token_impl!(i37);
ux_to_token_impl!(i38);
ux_to_token_impl!(i39);
ux_to_token_impl!(i40);
ux_to_token_impl!(i41);
ux_to_token_impl!(i42);
ux_to_token_impl!(i43);
ux_to_token_impl!(i44);
ux_to_token_impl!(i45);
ux_to_token_impl!(i46);
ux_to_token_impl!(i47);
ux_to_token_impl!(i48);
ux_to_token_impl!(i49);
ux_to_token_impl!(i50);
ux_to_token_impl!(i51);
ux_to_token_impl!(i52);
ux_to_token_impl!(i53);
ux_to_token_impl!(i54);
ux_to_token_impl!(i55);
ux_to_token_impl!(i56);
ux_to_token_impl!(i57);
ux_to_token_impl!(i58);
ux_to_token_impl!(i59);
ux_to_token_impl!(i60);
ux_to_token_impl!(i61);
ux_to_token_impl!(i62);
ux_to_token_impl!(i63);
ux_to_token_impl!(i65);
ux_to_token_impl!(i66);
ux_to_token_impl!(i67);
ux_to_token_impl!(i68);
ux_to_token_impl!(i69);
ux_to_token_impl!(i70);
ux_to_token_impl!(i71);
ux_to_token_impl!(i72);
ux_to_token_impl!(i73);
ux_to_token_impl!(i74);
ux_to_token_impl!(i75);
ux_to_token_impl!(i76);
ux_to_token_impl!(i77);
ux_to_token_impl!(i78);
ux_to_token_impl!(i79);
ux_to_token_impl!(i80);
ux_to_token_impl!(i81);
ux_to_token_impl!(i82);
ux_to_token_impl!(i83);
ux_to_token_impl!(i84);
ux_to_token_impl!(i85);
ux_to_token_impl!(i86);
ux_to_token_impl!(i87);
ux_to_token_impl!(i88);
ux_to_token_impl!(i89);
ux_to_token_impl!(i90);
ux_to_token_impl!(i91);
ux_to_token_impl!(i92);
ux_to_token_impl!(i93);
ux_to_token_impl!(i94);
ux_to_token_impl!(i95);
ux_to_token_impl!(i96);
ux_to_token_impl!(i97);
ux_to_token_impl!(i98);
ux_to_token_impl!(i99);
ux_to_token_impl!(i100);
ux_to_token_impl!(i101);
ux_to_token_impl!(i102);
ux_to_token_impl!(i103);
ux_to_token_impl!(i104);
ux_to_token_impl!(i105);
ux_to_token_impl!(i106);
ux_to_token_impl!(i107);
ux_to_token_impl!(i108);
ux_to_token_impl!(i109);
ux_to_token_impl!(i110);
ux_to_token_impl!(i111);
ux_to_token_impl!(i112);
ux_to_token_impl!(i113);
ux_to_token_impl!(i114);
ux_to_token_impl!(i115);
ux_to_token_impl!(i116);
ux_to_token_impl!(i117);
ux_to_token_impl!(i118);
ux_to_token_impl!(i119);
ux_to_token_impl!(i120);
ux_to_token_impl!(i121);
ux_to_token_impl!(i122);
ux_to_token_impl!(i123);
ux_to_token_impl!(i124);
ux_to_token_impl!(i125);
ux_to_token_impl!(i126);
ux_to_token_impl!(i127);

// #################### OTHER COMMON TYPES ####################

std_to_token_impl!(Ipv4Addr);
std_to_token_impl!(Ipv6Addr);
std_to_token_impl!(MacAddress);
