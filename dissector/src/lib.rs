use std::cell::UnsafeCell;

use byte_strings::concat_bytes;
use common::networking::DEFAULT_PORT;
use epan_sys::{
    _header_field_info, _value_string, col_add_str, col_clear, col_set_str,
    create_dissector_handle, dissector_add_uint, dissector_handle, field_display_e_BASE_HEX,
    ftenum_FT_UINT16, ftenum_FT_UINT8, hf_ref_type_HF_REF_TYPE_NONE, hf_register_info,
    proto_item_add_subtree, proto_plugin, proto_register_field_array, proto_register_plugin,
    proto_register_protocol, proto_register_subtree_array, proto_tree_add_item,
    tvb_captured_length, tvb_get_guint8, COL_INFO, COL_PROTOCOL, ENC_BIG_ENDIAN, ENC_NA,
};

// Useful wireshark macros
// #define HFILL -1, 0, HF_REF_TYPE_NONE, -1, NULL

// Exported plugin version information that wireshark needs
#[no_mangle]
pub static plugin_version: &[u8] = b"0.0.1\0";
#[no_mangle]
pub static plugin_release: &[u8] = b"3.2\0";
#[no_mangle]
pub static plugin_want_major: i32 = 3;
#[no_mangle]
pub static plugin_want_minor: i32 = 2;

// Main entry from wireshark
// Usually generated when compiled as c code but we'll bypass that by recreating it
#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn plugin_register() {
    static PLUGIN: proto_plugin = proto_plugin {
        register_handoff: Some(proto_reg_handoff_among_us),
        register_protoinfo: Some(proto_register_among_us),
    };
    if PROTO_AMONG_US == -1 {
        proto_register_plugin(&PLUGIN);
    }
}

static mut PROTO_AMONG_US: i32 = -1;

static mut HF_AMONGUS_HAZEL_TYPE: UnsafeCell<i32> = UnsafeCell::new(-1);

static mut ETT_AMONGUS: UnsafeCell<i32> = UnsafeCell::new(-1);

struct HfRegisterInfo(hf_register_info);
unsafe impl Sync for HfRegisterInfo {}

macro_rules! value_string {
    ($name:expr, $val:expr) => {{
        _value_string {
            strptr: concat_bytes!($name, b"\0").as_ptr() as *const i8,
            value: $val,
        }
    }};
}
const HAZEL_HEADER_NAMES: &[_value_string] = &[
    value_string!(b"Hello", 0x08),
    value_string!(b"Disconnect", 0x09),
    value_string!(b"Acknowledge", 0x0a),
    value_string!(b"Keep-Alive", 0x0c),
    value_string!(b"Unreliable", 0x00),
    value_string!(b"Reliable", 0x01),
];

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn proto_register_among_us() {
    static mut INFO: [HfRegisterInfo; 2] = [
        HfRegisterInfo(hf_register_info {
            p_id: unsafe { HF_AMONGUS_HAZEL_TYPE.get() },
            hfinfo: _header_field_info {
                name: b"Hazel Header Type\0".as_ptr() as *const i8,
                abbrev: b"amongus.hazel\0".as_ptr() as *const i8,
                type_: ftenum_FT_UINT8,
                display: field_display_e_BASE_HEX as i32,
                strings: HAZEL_HEADER_NAMES.as_ptr() as *const std::ffi::c_void,
                bitmask: 0,
                blurb: std::ptr::null(),
                id: -1,
                parent: 0,
                ref_type: hf_ref_type_HF_REF_TYPE_NONE,
                same_name_prev_id: -1,
                same_name_next: std::ptr::null_mut(),
            },
        }),
        HfRegisterInfo(hf_register_info {
            p_id: unsafe { HF_AMONGUS_HAZEL_TYPE.get() },
            hfinfo: _header_field_info {
                name: b"Hazel Packet Length\0".as_ptr() as *const i8,
                abbrev: b"amongus.hazel_len\0".as_ptr() as *const i8,
                type_: ftenum_FT_UINT16,
                display: field_display_e_BASE_HEX as i32,
                strings: std::ptr::null(),
                bitmask: 0,
                blurb: std::ptr::null(),
                id: -1,
                parent: 0,
                ref_type: hf_ref_type_HF_REF_TYPE_NONE,
                same_name_prev_id: -1,
                same_name_next: std::ptr::null_mut(),
            },
        }),
    ];
    static mut ETT: [*mut i32; 1] = [unsafe { ETT_AMONGUS.get() }];
    PROTO_AMONG_US = proto_register_protocol(
        b"Among Us Protocol\0".as_ptr() as *const i8,
        b"Among Us\0".as_ptr() as *const i8,
        b"amongus\0".as_ptr() as *const i8,
    );

    proto_register_field_array(
        PROTO_AMONG_US,
        INFO.as_mut_ptr() as *mut hf_register_info,
        INFO.len() as i32,
    );
    proto_register_subtree_array(ETT.as_ptr(), ETT.len() as i32);
}

static mut AMONG_US_HANDLE: *mut dissector_handle = std::ptr::null_mut();

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn proto_reg_handoff_among_us() {
    AMONG_US_HANDLE = create_dissector_handle(Some(dissect_among_us), PROTO_AMONG_US);
    dissector_add_uint(
        b"udp.port\0".as_ptr() as *const i8,
        DEFAULT_PORT as u32,
        AMONG_US_HANDLE,
    );
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn dissect_among_us(
    tvbuff: *mut epan_sys::tvbuff,
    packet_info: *mut epan_sys::_packet_info,
    proto_tree: *mut epan_sys::_proto_node,
    void: *mut std::ffi::c_void,
) -> i32 {
    // Dereference packet info
    let packet_info = *packet_info;

    // Set protocol column to Among Us
    col_set_str(
        packet_info.cinfo,
        COL_PROTOCOL as i32,
        b"Among Us\0".as_ptr() as *const i8,
    );

    // Clear info column
    col_clear(packet_info.cinfo, COL_INFO as i32);

    // Add protocol subtree
    let ti = proto_tree_add_item(proto_tree, PROTO_AMONG_US, tvbuff, 0, -1, ENC_NA);
    let amongus_tree = proto_item_add_subtree(ti, *ETT_AMONGUS.get());

    // Hazel header
    proto_tree_add_item(
        amongus_tree,
        *HF_AMONGUS_HAZEL_TYPE.get(),
        tvbuff,
        0,
        1,
        ENC_BIG_ENDIAN,
    );

    let sent_to_server = packet_info.destport == DEFAULT_PORT as u32;
    let header_type = tvb_get_guint8(tvbuff, 0);
    col_set_str(
        packet_info.cinfo,
        COL_INFO as i32,
        if sent_to_server {
            b"C -> S\0"
        } else {
            b"S -> C\0"
        }
        .as_ptr() as *const i8,
    );

    // Return captured length
    tvb_captured_length(tvbuff) as i32
}
