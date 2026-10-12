use super::bundle::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "volt-bundle-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn options(&self) -> BundleOptions {
        let binary = self.0.join("program");
        fs::write(&binary, [0xcf, 0xfa, 0xed, 0xfe, 0, 0, 0, 0]).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
        }
        BundleOptions::parse(
            &[
                "--binary",
                binary.to_str().unwrap(),
                "--output",
                self.0.join("Notes.app").to_str().unwrap(),
                "--name",
                "Notes & <Tools>",
                "--identifier",
                "dev.volt.notes",
                "--version",
                "1.2.3",
                "--minimum-system-version",
                "11.0",
            ]
            .map(str::to_owned),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn bundle_metadata_escapes_xml_and_requires_explicit_identity() {
    let f = Fixture::new();
    let options = f.options();
    let xml = options.plist();
    assert!(xml.contains("Notes &amp; &lt;Tools&gt;"));
    assert!(xml.contains("<string>dev.volt.notes</string>"));
    assert!(xml.contains("<string>11.0</string>"));
    assert!(BundleOptions::parse(&[]).is_err());
    for invalid in ["--unknown", "--name"] {
        assert!(BundleOptions::parse(&[invalid.into()]).is_err());
    }
    assert!(
        BundleOptions::parse(&["--name".into(), "A".into(), "--name".into(), "B".into()]).is_err()
    );
}
#[test]
fn bundle_rejects_invalid_metadata_and_resource_collisions() {
    let f = Fixture::new();
    let mut options = f.options();
    for id in ["", "notes", "dev..notes", "dev.volt/notes", "dev.volt.☃"] {
        options.identifier = id.into();
        assert!(options.validate().is_err());
    }
    options.identifier = "dev.volt.notes".into();
    options.version = "1.0-beta".into();
    assert!(options.validate().is_err());
    options.version = "1.2.3".into();
    options.output = f.0.join("Notes");
    assert!(options.validate().is_err());
    options.output = f.0.join("Notes.app");
    let resource = f.0.join("data.txt");
    fs::write(&resource, "data").unwrap();
    options.resources = vec![resource.clone(), resource];
    assert!(options.validate().is_err());
}
#[test]
fn bundle_structure_resources_and_no_overwrite() {
    let f = Fixture::new();
    let mut options = f.options();
    let data = f.0.join("data.txt");
    fs::write(&data, "resource bytes").unwrap();
    options.resources.push(data);
    assemble(&options).unwrap();
    assert_eq!(
        fs::read(
            options
                .output
                .join("Contents/MacOS")
                .join(options.executable_name())
        )
        .unwrap(),
        fs::read(&options.binary).unwrap()
    );
    assert_eq!(
        fs::read_to_string(options.output.join("Contents/Resources/data.txt")).unwrap(),
        "resource bytes"
    );
    let xml = fs::read(options.output.join("Contents/Info.plist")).unwrap();
    assert!(assemble(&options).is_err());
    assert_eq!(
        fs::read(options.output.join("Contents/Info.plist")).unwrap(),
        xml
    );
}
#[cfg(unix)]
#[test]
fn bundle_rejects_symlink_inputs_and_existing_symlink_output() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let mut options = f.options();
    let link = f.0.join("linked");
    symlink(&options.binary, &link).unwrap();
    options.resources.push(link);
    assert!(assemble(&options).is_err());
    options.resources.clear();
    symlink(&f.0, &options.output).unwrap();
    assert!(assemble(&options).is_err());
    assert!(options.output.is_symlink());
    assert!(options.binary.exists());
}
#[test]
fn bundle_only_accepts_system_libraries_and_compatible_deployment() {
    assert!(validate_libraries("app:\n\t/usr/lib/libSystem.B.dylib (compatibility version 1.0)\n\t/System/Library/Frameworks/AppKit.framework/AppKit (compatibility version 1.0)\n").is_ok());
    for dep in [
        "@rpath/libPrivate.dylib",
        "/tmp/custom.dylib",
        "/usr/lib/../local/lib.dylib",
    ] {
        assert!(
            validate_libraries(&format!("app:\n\t{dep} (compatibility version 1.0)\n")).is_err()
        );
    }
    assert!(
        validate_deployment("cmd LC_BUILD_VERSION\n platform 1\n minos 12.0\n", "11.0").is_err()
    );
    assert!(
        validate_deployment("cmd LC_BUILD_VERSION\n platform 1\n minos 11.0\n", "12.0").is_ok()
    );
    assert!(
        validate_deployment("cmd LC_BUILD_VERSION\n platform 2\n minos 11.0\n", "12.0").is_err()
    );
    assert!(validate_deployment("not a Mach-O build receipt", "12.0").is_err());
}
#[test]
fn bundle_rejects_nonexecutables_privacy_overrides_and_case_collisions() {
    let f = Fixture::new();
    let mut options = f.options();
    options
        .usage
        .push(("CFBundleExecutable".into(), "attacker".into()));
    assert!(assemble(&options).is_err());
    assert!(!options.output.exists());
    options.usage = vec![("NSCameraUsageDescription".into(), "Camera & video".into())];
    assert!(options.validate().is_ok());
    assert!(options.plist().contains("Camera &amp; video"));
    options.usage.push(options.usage[0].clone());
    assert!(options.validate().is_err());
    options.usage.clear();
    let a = f.0.join("A");
    let b_dir = f.0.join("lower");
    fs::create_dir(&b_dir).unwrap();
    let b = b_dir.join("a");
    fs::write(&a, "upper").unwrap();
    fs::write(&b, "lower").unwrap();
    options.resources = vec![a, b];
    assert!(assemble(&options).is_err());
    assert!(!options.output.exists());
    options.resources.clear();
    fs::write(&options.binary, "#!/bin/sh\nexit 0").unwrap();
    assert!(options.validate().is_err());
    assert!(validate_headers("MH_MAGIC_64 ARM64 ALL 0x00 EXECUTE 30 3288 PIE").is_ok());
    assert!(validate_headers("MH_MAGIC_64 ARM64 ALL 0x00 DYLIB 30 3288 PIE").is_err());
    assert!(validate_headers("bogus").is_err());
}
#[test]
fn bundle_native_failure_cleanup_and_exclusive_copy() {
    let f = Fixture::new();
    let options = f.options();
    // Exclusive copies do not replace an existing destination.
    let dest = f.0.join("copy-target");
    fs::write(&dest, "do not overwrite").unwrap();
    assert!(super::bundle::copy(&options.binary, &dest, 64, false).is_err());
    assert_eq!(fs::read_to_string(&dest).unwrap(), "do not overwrite");
    let mut invalid = f.options();
    invalid.icon = Some(dest);
    assert!(assemble(&invalid).is_err());
    assert!(!invalid.output.exists());
    #[cfg(target_os = "macos")]
    {
        // Valid fixture magic but malformed Mach-O: construction succeeds, native audit fails.
        assert!(run(options).is_err());
        assert!(!invalid.output.exists());
    }
    #[cfg(not(target_os = "macos"))]
    {
        assert!(run(options).unwrap_err().to_string().contains("macOS only"));
    }
}

#[test]
fn bundle_build_number_is_independent_from_zero_major_release_and_signing_is_explicit() {
    let f = Fixture::new();
    let mut options = f.options();
    assert_eq!(options.build, "1");
    options.version = "0.3.0".into();
    assert!(options.validate().is_ok());
    for build in ["0", "10000", "1.100", "1.0.100", "beta"] {
        options.build = build.into();
        assert!(options.validate().is_err());
    }
    let args = [
        "--binary",
        options.binary.to_str().unwrap(),
        "--output",
        options.output.to_str().unwrap(),
        "--name",
        "Notes",
        "--identifier",
        "dev.volt.notes",
        "--version",
        "0.3.0",
        "--minimum-system-version",
        "11.0",
        "--sign",
        "none",
    ]
    .map(str::to_owned);
    assert!(!BundleOptions::parse(&args).unwrap().sign);
    let mut wrong = args.to_vec();
    *wrong.last_mut().unwrap() = "Developer ID".into();
    assert!(BundleOptions::parse(&wrong).is_err());
    assert!(validate_deployment("cmd LC_VERSION_MIN_IPHONEOS\n version 11.0\n", "12.0").is_err());
}

#[test]
fn executable_is_named_after_the_app_in_safe_ascii() {
    let fixture = Fixture::new();
    let mut options = fixture.options();
    for (name, expected) in [
        ("Volt Notes", "Volt Notes"),
        ("Café/Notes ✨", "CafNotes"),
        ("✨✨", "VoltApp"),
    ] {
        options.name = name.into();
        assert_eq!(options.executable_name(), expected);
        assert!(options.plist().contains(&format!(
            "<key>CFBundleExecutable</key><string>{expected}</string>"
        )));
    }
}
