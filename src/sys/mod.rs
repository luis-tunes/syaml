#![allow(non_camel_case_types, non_snake_case, unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::bool_to_int_with_if,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_ptr_alignment,
    clippy::cast_sign_loss,
    clippy::collapsible_if,
    clippy::doc_markdown,
    clippy::fn_params_excessive_bools,
    clippy::if_not_else,
    clippy::items_after_statements,
    clippy::let_underscore_untyped,
    clippy::manual_range_contains,
    clippy::manual_swap,
    clippy::missing_panics_doc,
    clippy::missing_safety_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::nonminimal_bool,
    clippy::ptr_as_ptr,
    clippy::redundant_else,
    clippy::similar_names,
    clippy::single_match,
    clippy::single_match_else,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::unnecessary_cast,
    clippy::unreadable_literal,
    clippy::while_immutable_condition, // https://github.com/rust-lang/rust-clippy/issues/3548
)]

use core::mem::size_of;

mod libc {
    pub use core::ffi::c_void;
    pub use core::primitive::{
        i32 as c_int, i64 as c_long, i8 as c_char, u32 as c_uint, u64 as c_ulong, u8 as c_uchar,
    };
}

#[macro_use]
mod externs {
    use crate::sys::libc;
    use crate::sys::ops::{die, ForceAdd as _, ForceInto as _};
    use alloc::alloc::{self as rust, Layout};
    use core::mem::{self, MaybeUninit};
    use core::ptr;
    use core::slice;

    const HEADER: usize = {
        let need_len = mem::size_of::<usize>();
        // Round up to multiple of MALLOC_ALIGN.
        (need_len + MALLOC_ALIGN - 1) & !(MALLOC_ALIGN - 1)
    };

    // `max_align_t` may be bigger than this, but libyaml does not use `long
    // double` or u128.
    const MALLOC_ALIGN: usize = {
        let int_align = mem::align_of::<libc::c_ulong>();
        let ptr_align = mem::align_of::<usize>();
        if int_align >= ptr_align {
            int_align
        } else {
            ptr_align
        }
    };

    pub unsafe fn malloc(size: libc::c_ulong) -> *mut libc::c_void {
        let size = HEADER.force_add(size.force_into());
        let layout = Layout::from_size_align(size, MALLOC_ALIGN)
            .ok()
            .unwrap_or_else(die);
        let memory = rust::alloc(layout);
        if memory.is_null() {
            rust::handle_alloc_error(layout);
        }
        memory.cast::<usize>().write(size);
        memory.add(HEADER).cast()
    }

    pub unsafe fn realloc(ptr: *mut libc::c_void, new_size: libc::c_ulong) -> *mut libc::c_void {
        let mut memory = ptr.cast::<u8>().sub(HEADER);
        let size = memory.cast::<usize>().read();
        let layout = Layout::from_size_align_unchecked(size, MALLOC_ALIGN);
        let new_size = HEADER.force_add(new_size.force_into());
        let new_layout = Layout::from_size_align(new_size, MALLOC_ALIGN)
            .ok()
            .unwrap_or_else(die);
        memory = rust::realloc(memory, layout, new_size);
        if memory.is_null() {
            rust::handle_alloc_error(new_layout);
        }
        memory.cast::<usize>().write(new_size);
        memory.add(HEADER).cast()
    }

    pub unsafe fn free(ptr: *mut libc::c_void) {
        let memory = ptr.cast::<u8>().sub(HEADER);
        let size = memory.cast::<usize>().read();
        let layout = Layout::from_size_align_unchecked(size, MALLOC_ALIGN);
        rust::dealloc(memory, layout);
    }

    pub unsafe fn memcmp(
        lhs: *const libc::c_void,
        rhs: *const libc::c_void,
        count: libc::c_ulong,
    ) -> libc::c_int {
        let lhs = slice::from_raw_parts(lhs.cast::<u8>(), count as usize);
        let rhs = slice::from_raw_parts(rhs.cast::<u8>(), count as usize);
        lhs.cmp(rhs) as libc::c_int
    }

    pub unsafe fn memcpy(
        dest: *mut libc::c_void,
        src: *const libc::c_void,
        count: libc::c_ulong,
    ) -> *mut libc::c_void {
        ptr::copy_nonoverlapping(
            src.cast::<MaybeUninit<u8>>(),
            dest.cast::<MaybeUninit<u8>>(),
            count as usize,
        );
        dest
    }

    pub unsafe fn memmove(
        dest: *mut libc::c_void,
        src: *const libc::c_void,
        count: libc::c_ulong,
    ) -> *mut libc::c_void {
        ptr::copy(
            src.cast::<MaybeUninit<u8>>(),
            dest.cast::<MaybeUninit<u8>>(),
            count as usize,
        );
        dest
    }

    pub unsafe fn memset(
        dest: *mut libc::c_void,
        ch: libc::c_int,
        count: libc::c_ulong,
    ) -> *mut libc::c_void {
        ptr::write_bytes(dest.cast::<u8>(), ch as u8, count as usize);
        dest
    }

    pub unsafe fn strcmp(lhs: *const libc::c_char, rhs: *const libc::c_char) -> libc::c_int {
        let lhs = slice::from_raw_parts(lhs.cast::<u8>(), strlen(lhs) as usize);
        let rhs = slice::from_raw_parts(rhs.cast::<u8>(), strlen(rhs) as usize);
        lhs.cmp(rhs) as libc::c_int
    }

    pub unsafe fn strdup(src: *const libc::c_char) -> *mut libc::c_char {
        let len = strlen(src);
        let dest = malloc(len + 1);
        memcpy(dest, src.cast(), len + 1);
        dest.cast()
    }

    pub unsafe fn strlen(str: *const libc::c_char) -> libc::c_ulong {
        let mut end = str;
        while *end != 0 {
            end = end.add(1);
        }
        end.offset_from(str) as libc::c_ulong
    }

    pub unsafe fn strncmp(
        lhs: *const libc::c_char,
        rhs: *const libc::c_char,
        mut count: libc::c_ulong,
    ) -> libc::c_int {
        let mut lhs = lhs.cast::<u8>();
        let mut rhs = rhs.cast::<u8>();
        while count > 0 && *lhs != 0 && *lhs == *rhs {
            lhs = lhs.add(1);
            rhs = rhs.add(1);
            count -= 1;
        }
        if count == 0 {
            0
        } else {
            (*lhs).cmp(&*rhs) as libc::c_int
        }
    }

    macro_rules! __assert {
        (false $(,)?) => {
            $crate::sys::externs::__assert_fail(stringify!(false), file!(), line!())
        };
        ($assertion:expr $(,)?) => {
            if !$assertion {
                $crate::sys::externs::__assert_fail(stringify!($assertion), file!(), line!());
            }
        };
    }

    pub(crate) unsafe fn __assert_fail(
        __assertion: &'static str,
        __file: &'static str,
        __line: u32,
    ) -> ! {
        struct Abort;
        impl Drop for Abort {
            fn drop(&mut self) {
                panic!();
            }
        }
        let _abort_on_panic = Abort;
        panic!("{}:{}: Assertion `{}` failed.", __file, __line, __assertion);
    }
}

trait PointerExt: Sized {
    fn c_offset_from(self, origin: Self) -> isize;
}

impl<T> PointerExt for *const T {
    fn c_offset_from(self, origin: *const T) -> isize {
        (self as isize - origin as isize) / size_of::<T>() as isize
    }
}

impl<T> PointerExt for *mut T {
    fn c_offset_from(self, origin: *mut T) -> isize {
        (self as isize - origin as isize) / size_of::<T>() as isize
    }
}

#[macro_use]
mod macros;

mod api;
mod emitter;
mod ops;
mod parser;
mod reader;
mod scanner;
mod success;
mod writer;
mod yaml;

pub use crate::sys::api::{
    yaml_document_end_event_initialize, yaml_document_start_event_initialize, yaml_emitter_delete,
    yaml_emitter_initialize, yaml_emitter_set_output, yaml_emitter_set_unicode,
    yaml_emitter_set_width, yaml_event_delete, yaml_mapping_end_event_initialize,
    yaml_mapping_start_event_initialize, yaml_parser_delete, yaml_parser_initialize,
    yaml_parser_set_encoding, yaml_parser_set_input_string, yaml_scalar_event_initialize,
    yaml_sequence_end_event_initialize, yaml_sequence_start_event_initialize,
    yaml_stream_end_event_initialize, yaml_stream_start_event_initialize,
};
pub use crate::sys::emitter::yaml_emitter_emit;
pub use crate::sys::parser::yaml_parser_parse;
pub use crate::sys::writer::yaml_emitter_flush;
#[doc(hidden)]
pub use crate::sys::yaml::{
    yaml_break_t::*, yaml_emitter_state_t::*, yaml_encoding_t::*, yaml_error_type_t::*,
    yaml_event_type_t::*, yaml_mapping_style_t::*, yaml_parser_state_t::*, yaml_scalar_style_t::*,
    yaml_sequence_style_t::*, yaml_token_type_t::*,
};
pub use crate::sys::yaml::{
    yaml_emitter_state_t, yaml_emitter_t, yaml_encoding_t, yaml_error_type_t, yaml_event_t,
    yaml_mapping_style_t, yaml_mark_t, yaml_parser_state_t, yaml_parser_t, yaml_scalar_style_t,
    yaml_sequence_style_t, yaml_simple_key_t, yaml_tag_directive_t, yaml_token_t,
    yaml_token_type_t, yaml_version_directive_t, yaml_write_handler_t,
};
