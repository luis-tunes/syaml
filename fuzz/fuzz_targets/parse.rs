#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = syaml::from_slice::<syaml::Value>(data);
});
