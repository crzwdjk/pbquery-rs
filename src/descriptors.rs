extern crate libc;
extern crate libloading;

use std::ascii::*;

#[repr(C)]
pub struct MessageDescriptor {
    /** Magic value checked to ensure that the API is used correctly. */
    magic: u32,
    
    /** The qualified name (e.g., "namespace.Type"). */
    name: *const libc::c_char,
    /** The unqualified name as given in the .proto file (e.g., "Type"). */
    short_name: *const libc::c_char,
    /** Identifier used in generated C code. */
    c_name: *const libc::c_char,
    /** The dot-separated namespace. */
    package_name: *const libc::c_char,
    sizeof_message: usize,

    /** Number of elements in `fields`. */
    n_fields: u32,
    /** Field descriptors, sorted by tag number. */
    fields: *const FieldDescriptor,
    /** Used for looking up fields by name. */
    fields_sorted_by_name: *const u32,
    
    /** Number of elements in `field_ranges`. */
    n_field_ranges: u32,
    /** Used for looking up fields by id. */
    field_ranges: *const libc::c_void,
    
    message_init: *const libc::c_void,
    reserved1: *const libc::c_void,
    reserved2: *const libc::c_void,
    reserved3: *const libc::c_void,
}

#[link(name = "protobuf-c")]
extern {
    fn protobuf_c_message_descriptor_get_field_by_name(
        desc: *const MessageDescriptor,
        name: *const u8) -> *const FieldDescriptor;
}

use std::ffi::CString; 
impl MessageDescriptor {
    pub fn load<'lib>(lib: &'lib libloading::Library, messagename: &str)
                      -> Result<&'lib MessageDescriptor, &'static str> {

        // "transit_realtime.FeedMessage" -> "transit_realtime__feed_message__descriptor"
        let mut munged_messagename = String::new();

        let mut prev = '\0';
        for (i, c) in messagename.char_indices() {
            if c == '.' {
                munged_messagename.push('_');
                munged_messagename.push('_');
            } else if c.is_uppercase() {
                if i > 0 && prev != '.' {
                    munged_messagename.push('_');
                }
                let lc = c.to_ascii_lowercase();
                munged_messagename.push(lc);
            }
            else {
                munged_messagename.push(c);
            }
            prev = c;
        }
        let symname = format!("{}__descriptor", munged_messagename);
        unsafe {
            let s = lib.get(symname.as_bytes()).map(|x| *x);
            s.map_err(|_| "Could not load symbol")
        }
    }
    
    pub fn get_field_by_name(&self, name: &str) ->
        Option<&FieldDescriptor>
    {
        let cname = CString::new(name).unwrap();
        unsafe {
            protobuf_c_message_descriptor_get_field_by_name(
                self, cname.as_ptr()).as_ref()
        }
    }
}

#[repr(C)]
#[derive(PartialEq)]
pub enum Label { REQUIRED, OPTIONAL, REPEATED }

#[repr(C)]
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Type {
    INT32, SINT32, SFIXED32, INT64, SINT64, SFIXED64,
    UINT32, FIXED32, UINT64, FIXED64,
    FLOAT, DOUBLE,
    BOOL,
    ENUM,
    STRING, BYTES,
    MESSAGE,
}

impl Type {
    pub fn is_inty(self) -> bool {
        match self {
            Type::INT32 | Type::SINT32 | Type::SFIXED32 | Type::UINT32 |
            Type::INT64 | Type::SINT64 | Type::SFIXED64 | Type::UINT64 |
            Type::FIXED32 | Type::FIXED64 => true,
            _ => false,
        }
    }

    pub fn is_floaty(self) -> bool {
        match self {
            Type::FLOAT | Type::DOUBLE => true,
            _ => false,
        }
    }

    pub fn is_stringy(self) -> bool {
        match self {
            Type::STRING | Type::BYTES => true,
            _ => false,
        }
    }

    pub fn is_message(self) -> bool {
        if let Type::MESSAGE = self { true } else { false }
    }
}

#[repr(C)]
pub struct FieldDescriptor {
    /** Name of the field as given in the .proto file. */
    name: *const libc::c_char,
    /** Tag value of the field as given in the .proto file. */
    pub id: u32,
    /** Whether the field is `REQUIRED`, `OPTIONAL`, or `REPEATED`. */
    pub label: Label,
    /** The type of the field. */
    pub fieldtype: Type,
    quantifier_offset: u32,
    offset: u32,
    /**
     * A type-specific descriptor.
     *
     * If `type` is `PROTOBUF_C_TYPE_ENUM`, then `descriptor` points to the
     * corresponding `ProtobufCEnumDescriptor`.
     *
     * If `type` is `PROTOBUF_C_TYPE_MESSAGE`, then `descriptor` points to
     * the corresponding `ProtobufCMessageDescriptor`.
     *
     * Otherwise this field is NULL.
     */
    descriptor: *const libc::c_void, /* for MESSAGE and ENUM types */
    /** The default value for this field, if defined. May be NULL. */
    default_value: *const libc::c_void,
    /**
     * A flag word. Zero or more of the bits defined in the
     * `ProtobufCFieldFlag` enum may be set.
     */
    flags: u32,

    reserved_flags: u32,
    reserved2: *const libc::c_void,
    reserved3: *const libc::c_void,
}

impl FieldDescriptor {
    pub fn get_message_descriptor(&self)
                                  -> Option<&MessageDescriptor> {
        if self.fieldtype != Type::MESSAGE { return None }
        unsafe {
            let desc = self.descriptor as *const MessageDescriptor;
            desc.as_ref()
        }
    }
}
