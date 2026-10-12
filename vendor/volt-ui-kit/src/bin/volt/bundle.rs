//! Deliberately narrow local macOS packaging; no shell, recursive assets or publish.
// The parser stays available elsewhere to report an explicit unsupported platform.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]
use super::error;
#[cfg(target_os = "macos")]
use std::process::Command;
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
};
const MAX_BINARY: u64 = 1024 * 1024 * 1024;
const MAX_RESOURCE: u64 = 64 * 1024 * 1024;
const USAGE_KEYS: &[&str] = &[
    "NSCameraUsageDescription",
    "NSMicrophoneUsageDescription",
    "NSLocationUsageDescription",
    "NSContactsUsageDescription",
    "NSCalendarsUsageDescription",
    "NSCalendarsFullAccessUsageDescription",
    "NSCalendarsWriteOnlyAccessUsageDescription",
    "NSRemindersUsageDescription",
    "NSRemindersFullAccessUsageDescription",
    "NSPhotoLibraryUsageDescription",
    "NSPhotoLibraryAddUsageDescription",
    "NSAppleEventsUsageDescription",
    "NSSpeechRecognitionUsageDescription",
];
pub const HELP: &str = "cargo volt bundle --binary PATH --output PATH.app --name NAME --identifier REVERSE.DNS.ID --version X.Y.Z --minimum-system-version X.Y[.Z]\n  [--build-number N[.N[.N]]] [--resource FILE ...] [--icon FILE.icns]\n  [--usage NSCameraUsageDescription=TEXT ...] [--sign ad-hoc|none]\nmacOS only; prebuilt Mach-O, system dylibs only, no overwrite; local ad-hoc signing is NOT notarization";

#[derive(Debug)]
pub struct BundleOptions {
    pub binary: PathBuf,
    pub output: PathBuf,
    pub name: String,
    pub identifier: String,
    pub version: String,
    pub minimum: String,
    pub build: String,
    pub resources: Vec<PathBuf>,
    pub icon: Option<PathBuf>,
    pub usage: Vec<(String, String)>,
    pub sign: bool,
}
impl BundleOptions {
    pub fn parse(args: &[String]) -> io::Result<Self> {
        let mut fields = std::collections::HashMap::new();
        let mut resources = Vec::new();
        let mut usage = Vec::new();
        let mut i = 0;
        while i < args.len() {
            let flag = args[i].as_str();
            let value = args
                .get(i + 1)
                .ok_or_else(|| error(format!("missing value for {flag}")))?;
            match flag {
                "--resource" => {
                    if resources.len() >= 64 {
                        return Err(error("at most 64 resources"));
                    }
                    resources.push(PathBuf::from(value));
                }
                "--usage" => {
                    let (key, text) = value
                        .split_once('=')
                        .ok_or_else(|| error("--usage requires KEY=TEXT"))?;
                    if usage.len() >= USAGE_KEYS.len() {
                        return Err(error("too many usage descriptions"));
                    }
                    usage.push((key.to_owned(), text.to_owned()));
                }
                "--binary"
                | "--output"
                | "--name"
                | "--identifier"
                | "--version"
                | "--minimum-system-version"
                | "--build-number"
                | "--icon"
                | "--sign" => {
                    if fields.insert(flag, value.clone()).is_some() {
                        return Err(error(format!("duplicate {flag}")));
                    }
                }
                _ => return Err(error(format!("unknown bundle option {flag}"))),
            }
            i += 2;
        }
        let mut required = |key| {
            fields
                .remove(key)
                .ok_or_else(|| error(format!("{key} required")))
        };
        let binary = required("--binary")?.into();
        let output = required("--output")?.into();
        let name = required("--name")?;
        let identifier = required("--identifier")?;
        let version = required("--version")?;
        let minimum = required("--minimum-system-version")?;
        let build = fields
            .remove("--build-number")
            .unwrap_or_else(|| "1".to_owned());
        let sign = match fields.remove("--sign").as_deref().unwrap_or("ad-hoc") {
            "ad-hoc" => true,
            "none" => false,
            _ => {
                return Err(error(
                    "--sign must be ad-hoc or none; Developer ID is a separate release step",
                ))
            }
        };
        Ok(Self {
            binary,
            output,
            name,
            identifier,
            version,
            minimum,
            build,
            resources,
            icon: fields.remove("--icon").map(PathBuf::from),
            usage,
            sign,
        })
    }
    /// The executable file name, shown by Activity Monitor and crash reports:
    /// the app name reduced to ASCII letters, digits, spaces, `-` and `_`.
    pub fn executable_name(&self) -> String {
        let name: String = self
            .name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_'))
            .collect();
        let name = name.trim();
        if name.is_empty() {
            "VoltApp".into()
        } else {
            name.into()
        }
    }
    pub fn validate(&self) -> io::Result<()> {
        text(&self.name, 128)?;
        if self.identifier.len() > 255
            || !self.identifier.contains('.')
            || self
                .identifier
                .split('.')
                .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
        {
            return Err(error(
                "identifier must contain nonempty reverse-DNS ASCII letters/digits/- segments",
            ));
        }
        if self.version.split('.').count() != 3 {
            return Err(error("version must be X.Y.Z"));
        }
        numeric_version(&self.version)?;
        let build = numeric_version(&self.build)?;
        if build[0] == 0 || build[0] > 9999 || build[1] > 99 || build[2] > 99 {
            return Err(error(
                "build number requires first component 1-9999, remaining components 0-99",
            ));
        }
        numeric_version(&self.minimum)?;
        if self.minimum.split('.').count() < 2 {
            return Err(error("minimum system version must be X.Y[.Z]"));
        }
        if self.output.extension().and_then(|v| v.to_str()) != Some("app")
            || self.output.file_stem().is_none_or(|v| v.is_empty())
        {
            return Err(error("output must be a named .app directory"));
        }
        let binary = regular(&self.binary, MAX_BINARY)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if binary.permissions().mode() & 0o111 == 0 {
                return Err(error("binary must be executable"));
            }
        }
        #[cfg(not(unix))]
        let _ = binary;
        let mut magic = [0; 4];
        File::open(&self.binary)?.read_exact(&mut magic)?;
        if !matches!(
            magic,
            [0xcf, 0xfa, 0xed, 0xfe]
                | [0xfe, 0xed, 0xfa, 0xcf]
                | [0xce, 0xfa, 0xed, 0xfe]
                | [0xfe, 0xed, 0xfa, 0xce]
                | [0xca, 0xfe, 0xba, 0xbe]
                | [0xbe, 0xba, 0xfe, 0xca]
                | [0xca, 0xfe, 0xba, 0xbf]
                | [0xbf, 0xba, 0xfe, 0xca]
        ) {
            return Err(error(
                "binary must be a prebuilt Mach-O executable, not a script",
            ));
        }
        if self.resources.len() > 64 {
            return Err(error("at most 64 resources"));
        }
        let mut names = HashSet::new();
        for path in self.resources.iter().chain(self.icon.iter()) {
            regular(path, MAX_RESOURCE)?;
            let name = filename(path)?;
            // Case-fold collision check also protects the usual macOS filesystem.
            if !names.insert(name.to_lowercase()) {
                return Err(error("duplicate resource/icon filename"));
            }
        }
        if let Some(icon) = &self.icon {
            if icon.extension().and_then(|v| v.to_str()) != Some("icns") {
                return Err(error("icon must be .icns"));
            }
            let mut header = [0; 8];
            File::open(icon)?.read_exact(&mut header)?;
            if &header[..4] != b"icns"
                || u32::from_be_bytes(header[4..8].try_into().unwrap()) as u64
                    != fs::metadata(icon)?.len()
            {
                return Err(error("invalid ICNS header/length"));
            }
        }
        let mut keys = HashSet::new();
        for (key, value) in &self.usage {
            if !USAGE_KEYS.contains(&key.as_str()) || !keys.insert(key) {
                return Err(error("unsupported/duplicate privacy usage key"));
            }
            text(value, 4096)?;
        }
        Ok(())
    }
    pub fn plist(&self) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n");
        for (key, value) in [
            ("CFBundleInfoDictionaryVersion", "6.0"),
            ("CFBundlePackageType", "APPL"),
            ("CFBundleExecutable", &self.executable_name()),
            ("CFBundleName", &self.name),
            ("CFBundleDisplayName", &self.name),
            ("CFBundleIdentifier", &self.identifier),
            ("CFBundleShortVersionString", &self.version),
            ("CFBundleVersion", &self.build),
            ("LSMinimumSystemVersion", &self.minimum),
            ("NSPrincipalClass", "NSApplication"),
        ] {
            entry(&mut xml, key, value);
        }
        if let Some(icon) = &self.icon {
            entry(
                &mut xml,
                "CFBundleIconFile",
                icon.file_name().unwrap().to_str().unwrap(),
            );
        }
        for (key, value) in &self.usage {
            entry(&mut xml, key, value);
        }
        xml.push_str("<key>NSHighResolutionCapable</key><true/>\n</dict></plist>\n");
        xml
    }
}
fn text(value: &str, limit: usize) -> io::Result<()> {
    if value.trim().is_empty() || value.len() > limit || value.chars().any(char::is_control) {
        return Err(error(
            "metadata must be nonblank, bounded and free of control characters",
        ));
    }
    Ok(())
}
fn numeric_version(value: &str) -> io::Result<[u32; 3]> {
    let parts: Vec<_> = value.split('.').collect();
    if parts.is_empty() || parts.len() > 3 || value.len() > 32 {
        return Err(error("version must contain 1-3 numeric components"));
    }
    let mut result = [0; 3];
    for (i, p) in parts.iter().enumerate() {
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return Err(error("version component must be numeric"));
        }
        result[i] = p
            .parse()
            .map_err(|_| error("version component too large"))?;
    }
    Ok(result)
}
fn entry(xml: &mut String, key: &str, value: &str) {
    xml.push_str(&format!(
        "<key>{}</key><string>{}</string>\n",
        escape(key),
        escape(value)
    ));
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn regular(path: &Path, maximum: u64) -> io::Result<fs::Metadata> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_file() || m.len() > maximum {
        return Err(error(format!(
            "{} must be a bounded regular file (no symlinks/directories)",
            path.display()
        )));
    }
    Ok(m)
}
fn filename(path: &Path) -> io::Result<&str> {
    let name = path
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| error("resource filename must be UTF-8"))?;
    text(name, 255)?;
    if name.contains(['/', '\\']) {
        return Err(error("invalid resource filename"));
    }
    Ok(name)
}
struct Cleanup {
    path: PathBuf,
    keep: bool,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
pub(super) fn copy(source: &Path, dest: &Path, maximum: u64, executable: bool) -> io::Result<()> {
    regular(source, maximum)?;
    let source = File::open(source)?;
    if !source.metadata()?.is_file() {
        return Err(error("opened source is not a regular file"));
    }
    let mut out = File::options().write(true).create_new(true).open(dest)?;
    if io::copy(&mut source.take(maximum + 1), &mut out)? > maximum {
        return Err(error("source grew beyond copy limit"));
    }
    out.flush()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            dest,
            fs::Permissions::from_mode(if executable { 0o755 } else { 0o644 }),
        )?;
    }
    #[cfg(not(unix))]
    let _ = executable;
    Ok(())
}
/// Pure construction, used by tests; production run additionally audits and signs.
pub fn assemble(options: &BundleOptions) -> io::Result<()> {
    options.validate()?;
    // Atomic reservation refuses existing directories, files and dangling symlinks.
    fs::create_dir(&options.output)?;
    let mut cleanup = Cleanup {
        path: options.output.clone(),
        keep: false,
    };
    let contents = options.output.join("Contents");
    fs::create_dir(&contents)?;
    fs::create_dir(contents.join("MacOS"))?;
    fs::create_dir(contents.join("Resources"))?;
    copy(
        &options.binary,
        &contents.join("MacOS").join(options.executable_name()),
        MAX_BINARY,
        true,
    )?;
    for source in options.resources.iter().chain(options.icon.iter()) {
        copy(
            source,
            &contents.join("Resources").join(filename(source)?),
            MAX_RESOURCE,
            false,
        )?;
    }
    fs::write(contents.join("Info.plist"), options.plist())?;
    fs::write(contents.join("PkgInfo"), b"APPL????")?;
    cleanup.keep = true;
    Ok(())
}
pub fn validate_headers(output: &str) -> io::Result<()> {
    let mut count = 0;
    for line in output.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields
            .first()
            .is_some_and(|v| v.starts_with("MH_MAGIC") || v.starts_with("MH_CIGAM"))
        {
            if fields.get(4) != Some(&"EXECUTE") {
                return Err(error("Mach-O slice is not an executable"));
            }
            count += 1;
        }
    }
    if count == 0 {
        return Err(error("missing executable Mach-O header receipt"));
    }
    Ok(())
}
pub fn validate_libraries(output: &str) -> io::Result<()> {
    let mut count = 0;
    for line in output.lines().filter(|line| line.starts_with('\t')) {
        let path = line
            .trim()
            .split(" (compatibility version")
            .next()
            .unwrap_or("");
        if !(path.starts_with("/System/Library/") || path.starts_with("/usr/lib/"))
            || Path::new(path)
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err(error(format!("non-system library {path}: framework/dylib embedding is not supported by this slice")));
        }
        count += 1;
    }
    if count == 0 {
        return Err(error("no dependency receipt from otool"));
    }
    Ok(())
}
pub fn validate_deployment(output: &str, minimum: &str) -> io::Result<()> {
    let declared = numeric_version(minimum)?;
    let mut found = 0;
    let mut build = false;
    let mut legacy = false;
    for line in output.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.first() == Some(&"cmd") {
            build = fields.get(1) == Some(&"LC_BUILD_VERSION");
            legacy = fields.get(1) == Some(&"LC_VERSION_MIN_MACOSX");
            if fields
                .get(1)
                .is_some_and(|v| v.starts_with("LC_VERSION_MIN_") && *v != "LC_VERSION_MIN_MACOSX")
            {
                return Err(error(
                    "binary contains a non-macOS legacy deployment record",
                ));
            }
        }
        if build && fields.first() == Some(&"platform") && fields.get(1) != Some(&"1") {
            return Err(error("binary is not built for macOS"));
        }
        if (build && fields.first() == Some(&"minos"))
            || (legacy && fields.first() == Some(&"version"))
        {
            let actual = numeric_version(
                fields
                    .get(1)
                    .ok_or_else(|| error("invalid deployment receipt"))?,
            )?;
            if actual > declared {
                return Err(error("declared minimum is below the binary's compiled deployment target; rebuild with MACOSX_DEPLOYMENT_TARGET or raise --minimum-system-version"));
            }
            found += 1;
        }
    }
    if found == 0 {
        return Err(error("missing macOS deployment target in Mach-O"));
    }
    Ok(())
}
#[cfg(target_os = "macos")]
fn tool(program: &str, args: &[&str], path: &Path) -> io::Result<String> {
    let output = Command::new(program)
        .args(args)
        .arg(path.canonicalize()?)
        .output()?;
    if !output.status.success() {
        return Err(error(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    String::from_utf8(output.stdout).map_err(|_| error("invalid UTF-8 tool receipt"))
}
pub fn run(options: BundleOptions) -> io::Result<()> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = options;
        Err(error("cargo volt bundle currently supports macOS only"))
    }
    #[cfg(target_os = "macos")]
    {
        assemble(&options)?;
        let mut cleanup = Cleanup {
            path: options.output.clone(),
            keep: false,
        };
        let executable = options
            .output
            .join("Contents/MacOS")
            .join(options.executable_name());
        tool(
            "/usr/bin/plutil",
            &["-lint"],
            &options.output.join("Contents/Info.plist"),
        )?;
        validate_headers(&tool("/usr/bin/otool", &["-hv"], &executable)?)?;
        validate_libraries(&tool("/usr/bin/otool", &["-L"], &executable)?)?;
        validate_deployment(
            &tool("/usr/bin/otool", &["-l"], &executable)?,
            &options.minimum,
        )?;
        if options.sign {
            tool(
                "/usr/bin/codesign",
                &["--force", "--sign", "-"],
                &options.output,
            )?;
            tool(
                "/usr/bin/codesign",
                &["--verify", "--strict"],
                &options.output,
            )?;
        }
        cleanup.keep = true;
        println!(
            "Created {} ({})",
            options.output.canonicalize()?.display(),
            if options.sign {
                "local ad-hoc signature verified; NOT notarized"
            } else {
                "no bundle signing requested; NOT notarized"
            }
        );
        Ok(())
    }
}
