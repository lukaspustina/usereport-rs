//! SDD more-useful-firefight Phase 1, C4.
//! GIVEN a Command where install_hint and what_to_look_for are both None
//! WHEN serialized with toml::to_string_pretty
//! THEN neither key appears in the output.

use usereport::command::Command;

#[test]
fn command_none_install_hint_omitted_in_serialization() {
    let cmd = Command::new("test", "test -a");
    assert!(
        cmd.install_hint().is_none(),
        "install_hint must be None for a newly constructed Command"
    );
    let toml_str = toml::to_string_pretty(&cmd).expect("serialize ok");
    assert!(
        !toml_str.contains("install_hint"),
        "install_hint must not appear in TOML when None, got: {}",
        toml_str
    );
}

#[test]
fn command_none_what_to_look_for_omitted_in_serialization() {
    let cmd = Command::new("test", "test -a");
    assert!(
        cmd.what_to_look_for().is_none(),
        "what_to_look_for must be None for a newly constructed Command"
    );
    let toml_str = toml::to_string_pretty(&cmd).expect("serialize ok");
    assert!(
        !toml_str.contains("what_to_look_for"),
        "what_to_look_for must not appear in TOML when None, got: {}",
        toml_str
    );
}
