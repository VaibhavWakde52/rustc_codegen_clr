#![feature(lang_items,adt_const_params,associated_type_defaults,core_intrinsics,start)]
#![allow(internal_features,incomplete_features,unused_variables,dead_code)]
#![no_std]
include!("../common.rs");
fn main(){
    let slice:&mut [u8] = unsafe{core::slice::from_raw_parts_mut(malloc(64) as *mut _,64)};
    black_box(slice);
}