use volt_config::Config;

#[test]
fn image_decoding_is_opt_in_and_old_configs_work() {
    let default: Config = toml::from_str("").unwrap();
    assert!(!default.inline_images.enabled);
    let enabled: Config = toml::from_str("[inline_images]\nenabled = true").unwrap();
    assert!(enabled.inline_images.enabled);
    let roundtrip: Config = toml::from_str(&toml::to_string(&enabled).unwrap()).unwrap();
    assert!(roundtrip.inline_images.enabled);
}
