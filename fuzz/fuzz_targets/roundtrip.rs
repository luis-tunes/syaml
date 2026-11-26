#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(v) = syaml::from_slice::<syaml::Value>(data) else {
        return;
    };
    let Ok(s) = syaml::to_string(&v) else {
        return;
    };
    let w: syaml::Value = syaml::from_str(&s).unwrap();
    assert_eq!(v, w);
});
