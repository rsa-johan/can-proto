use std::path::Path;

#[test]
fn example_package_compiles() {
    let pkg = proto_c::compile_package(Path::new("protos/examples")).unwrap();
    assert_eq!(pkg.name, "examples");
    assert!(pkg.mod_rs.contains("pub mod common_inc;"));
    assert!(pkg.mod_rs.contains("pub mod dummy;"));

    let dummy = &pkg.units.iter().find(|(m, _)| m == "dummy").unwrap().1;
    assert!(dummy.rust.contains("use super::common_inc::*;"));
    assert!(!dummy.rust.contains("pub enum NodeId"), "included items must not be duplicated");
    assert!(dummy.rust.contains("pub const RSP_ID_MATCH: u32 = 0x209;"));
    assert!(dummy.c.contains("#include \"common_inc.h\""));
    assert!(dummy.c.contains("static inline bool GetTemperature_is_response"));
}

#[test]
fn reports_line_numbers() {
    let err = proto_c::compile_str("def {0} A:\n.x 0\n.y 0\n", "t", "t.cp").unwrap_err();
    assert_eq!(err.to_string(), "line 3: field `y` overlaps `x` at byte 0 bit 0");
}

#[test]
fn rejects_overlapping_filters() {
    let src = "filter A(n: u8):\n.id 0..3 n\ndef {0} D:\n.x 0\n\
               frame X:\n.filter A(1)\n.D\nfn F(): D\n.rsp A(1)\n";
    let err = proto_c::compile_str(src, "t", "t.cp").unwrap_err();
    assert!(err.to_string().contains("can match the same frame"), "{err}");
}

#[test]
fn signed_and_big_endian_fields() {
    let src = "#endian=big\ndef {0,1} R:\n.rpm 0..12 i16\n.t 4 little\nfn S(a: u16): Void\n";
    let g = proto_c::compile_str(src, "t", "t.cp").unwrap();
    assert!(g.rust.contains("pub rpm: i16,"));
    assert!(g.rust.contains("let v = (buf[1] as u64) | (((buf[0] & 0x0F) as u64) << 8);"), "{}", g.rust);
    assert!(g.rust.contains("to_be_bytes"));
    assert!(g.c.contains("m->rpm = (int16_t)((v & 0x800u) ? -(int64_t)(~v & 0xFFFu) - 1 : (int64_t)v);"));
}

#[test]
fn id_type_separates_filters() {
    let src = "#id=extended\nfilter A:\n.id 0..11 1\nfilter Std:\n.ext false\ndef {0} D:\n.x 0\n\
               frame X:\n.filter A\n.D\nframe Y:\n.filter A | Std\n.D\n";
    let g = proto_c::compile_str(src, "t", "t.cp").unwrap();
    assert!(g.rust.contains("pub const FILTER_EXTENDED: Option<bool> = Some(true);"));
    assert!(g.rust.contains("pub const FILTER_EXTENDED: Option<bool> = Some(false);"));
    assert!(g.c.contains("if (extended) return false;"));

    let err = proto_c::compile_str("#id=standard\nfilter H:\n.id 11 1\ndef {0} D:\n.x 0\nframe F:\n.filter H\n.D\n", "t", "t.cp")
        .unwrap_err();
    assert!(err.to_string().contains("standard (11-bit) ids have no bit 11"), "{err}");
}

#[test]
fn rejects_conflicting_pipe() {
    let src = "filter A:\n.id 0 1\nfilter B:\n.id 0 0\ndef {0} D:\n.x 0\nframe X:\n.filter A | B\n.D\n";
    let err = proto_c::compile_str(src, "t", "t.cp").unwrap_err();
    assert!(err.to_string().contains("conflicting values for id bit 0"), "{err}");
}
