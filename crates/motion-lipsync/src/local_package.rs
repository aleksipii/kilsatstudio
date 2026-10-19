//! Explicitly enrolled local Phonetic packages. Enrollment is a local trust
//! decision about package completeness, not a license, quality or sandbox audit.
//! Every package byte is pinned; unknown packages retain explicit execution.
use crate::{engine_reuse::VerifiedEnvironment, rhubarb::Recognizer, AlignmentError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub const MANIFEST: &str = "kilsat-rhubarb-package-v1.json";
const PROFILE: &str = "self-contained-phonetic-v1";
const MAX_TOTAL: u64 = 512 * 1024 * 1024;
const MAX_FILES: usize = 4096;
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Snapshot(PathBuf);
impl Drop for Snapshot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn error(message: impl std::fmt::Display) -> AlignmentError {
    AlignmentError(format!("local recognizer package: {message}"))
}
fn check(cancel: &dyn Fn() -> bool, started: Instant) -> Result<(), AlignmentError> {
    if cancel() {
        return Err(error("cancelled"));
    }
    if started.elapsed() > Duration::from_secs(30) {
        return Err(error("inspection exceeded 30 seconds"));
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Declaration {
    schema_version: u32,
    profile: String,
    executable: String,
    tool_version: String,
    platform: String,
    files: BTreeMap<String, String>,
}
pub enum Inspection {
    Eligible(LocalPackage),
    Unavailable(String),
}
pub struct LocalPackage {
    root: PathBuf,
    declaration: Declaration,
    declaration_bytes: Vec<u8>,
    runtime: String,
    environment: VerifiedEnvironment,
}

fn read_hash(
    path: &Path,
    total: &mut u64,
    cancel: &dyn Fn() -> bool,
    started: Instant,
) -> Result<String, AlignmentError> {
    check(cancel, started)?;
    let metadata = fs::symlink_metadata(path).map_err(error)?;
    if !metadata.is_file() {
        return Err(error(
            "package entries must be regular files, never symlinks",
        ));
    }
    *total = total
        .checked_add(metadata.len())
        .ok_or_else(|| error("package size overflow"))?;
    if *total > MAX_TOTAL {
        return Err(error("package exceeds 512 MiB"));
    }
    let mut file = fs::File::open(path).map_err(error)?;
    if !file.metadata().map_err(error)?.is_file() {
        return Err(error("opened package file is not regular"));
    }
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut read = 0u64;
    loop {
        check(cancel, started)?;
        let count = file.read(&mut buffer).map_err(error)?;
        if count == 0 {
            break;
        }
        read += count as u64;
        if read > metadata.len() {
            return Err(error("package file grew during inspection"));
        }
        digest.update(&buffer[..count]);
    }
    if read != metadata.len() {
        return Err(error("package file changed during inspection"));
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn inventory(
    root: &Path,
    cancel: &dyn Fn() -> bool,
) -> Result<BTreeMap<String, String>, AlignmentError> {
    let started = Instant::now();
    let mut pending = vec![root.to_path_buf()];
    let mut result = BTreeMap::new();
    let mut total = 0;
    let mut entries = 0;
    while let Some(directory) = pending.pop() {
        check(cancel, started)?;
        for entry in fs::read_dir(directory).map_err(error)? {
            check(cancel, started)?;
            entries += 1;
            if entries > MAX_FILES {
                return Err(error("package exceeds 4096 entries"));
            }
            let path = entry.map_err(error)?.path();
            let metadata = fs::symlink_metadata(&path).map_err(error)?;
            let relative = path
                .strip_prefix(root)
                .map_err(error)?
                .to_str()
                .ok_or_else(|| error("non-UTF-8 package path"))?
                .to_owned();
            if relative.contains('\\') {
                return Err(error("backslashes are ineligible in package paths"));
            }
            if relative.len() > 4096 {
                return Err(error("package path exceeds 4 KiB"));
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if relative == MANIFEST {
                if !metadata.is_file() {
                    return Err(error("manifest must be regular"));
                }
            } else {
                result.insert(relative, read_hash(&path, &mut total, cancel, started)?);
            }
        }
    }
    Ok(result)
}
fn runtime() -> Result<String, AlignmentError> {
    let path = match std::env::consts::OS {
        "linux" => "/proc/sys/kernel/osrelease",
        "macos" => "/System/Library/CoreServices/SystemVersion.plist",
        _ => return Err(error("unsupported operating-system runtime policy")),
    };
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(error)?
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() > 65536 {
        return Err(error("unexpected OS runtime identity size"));
    }
    Ok(format!(
        "{}-{}-{:x}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        Sha256::digest(bytes)
    ))
}
fn static_binary(path: &Path) -> Result<(), AlignmentError> {
    if fs::metadata(path).map_err(error)?.len() > 128 * 1024 * 1024 {
        return Err(error("executable exceeds 128 MiB"));
    }
    let mut file = fs::File::open(path).map_err(error)?;
    let mut header = [0u8; 64];
    file.read_exact(&mut header).map_err(error)?;
    if std::env::consts::OS == "macos" {
        let mut bytes = Vec::new();
        use std::io::{Seek, SeekFrom};
        file.seek(SeekFrom::Start(0)).map_err(error)?;
        file.take(128 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(error)?;
        if bytes.len() > 128 * 1024 * 1024 {
            return Err(error("executable exceeds 128 MiB"));
        }
        return macho(&bytes);
    }
    if &header[..4] != b"\x7fELF" || header[4] != 2 || header[5] != 1 {
        return Err(error("profile requires a static little-endian ELF64 native binary; scripts and wrappers are ineligible"));
    }
    let machine = u16::from_le_bytes([header[18], header[19]]);
    if !matches!(
        (std::env::consts::ARCH, machine),
        ("x86_64", 62) | ("aarch64", 183)
    ) {
        return Err(error("binary architecture mismatch"));
    }
    let offset = u64::from_le_bytes(header[32..40].try_into().map_err(error)?);
    let size = u16::from_le_bytes([header[54], header[55]]) as usize;
    let count = u16::from_le_bytes([header[56], header[57]]) as usize;
    if size != 56 || count > 1024 || offset > 128 * 1024 * 1024 {
        return Err(error("unsupported ELF program headers"));
    }
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::Start(offset)).map_err(error)?;
    for _ in 0..count {
        let mut program = [0u8; 56];
        file.read_exact(&mut program).map_err(error)?;
        if matches!(
            u32::from_le_bytes(program[..4].try_into().map_err(error)?),
            2 | 3
        ) {
            return Err(error(
                "dynamic loader/dependencies are outside this self-contained profile",
            ));
        }
    }
    Ok(())
}
fn macho(bytes: &[u8]) -> Result<(), AlignmentError> {
    fn le(bytes: &[u8], offset: usize) -> Result<u32, AlignmentError> {
        Ok(u32::from_le_bytes(
            bytes
                .get(offset..offset + 4)
                .ok_or_else(|| error("truncated Mach-O"))?
                .try_into()
                .map_err(error)?,
        ))
    }
    if bytes.starts_with(&[0xca, 0xfe, 0xba, 0xbe]) {
        let count = u32::from_be_bytes(
            bytes
                .get(4..8)
                .ok_or_else(|| error("truncated fat binary"))?
                .try_into()
                .map_err(error)?,
        ) as usize;
        if count == 0 || count > 8 {
            return Err(error("unsupported fat architecture count"));
        }
        let wanted = match std::env::consts::ARCH {
            "aarch64" => 0x0100000c,
            "x86_64" => 0x01000007,
            _ => return Err(error("unsupported architecture")),
        };
        let mut selected = None;
        for index in 0..count {
            let row = bytes
                .get(8 + index * 20..28 + index * 20)
                .ok_or_else(|| error("truncated fat header"))?;
            let cpu = u32::from_be_bytes(row[..4].try_into().map_err(error)?);
            if cpu == wanted {
                if selected.is_some() {
                    return Err(error("duplicate fat architecture"));
                }
                let offset = u32::from_be_bytes(row[8..12].try_into().map_err(error)?) as usize;
                let size = u32::from_be_bytes(row[12..16].try_into().map_err(error)?) as usize;
                selected = Some(
                    bytes
                        .get(
                            offset
                                ..offset
                                    .checked_add(size)
                                    .ok_or_else(|| error("fat size overflow"))?,
                        )
                        .ok_or_else(|| error("fat slice out of bounds"))?,
                );
            }
        }
        let selected = selected.ok_or_else(|| error("fat binary has no host architecture"))?;
        if !selected.starts_with(&[0xcf, 0xfa, 0xed, 0xfe]) {
            return Err(error("unsupported fat Mach-O slice"));
        }
        return macho(selected);
    }
    if !bytes.starts_with(&[0xcf, 0xfa, 0xed, 0xfe]) {
        return Err(error(
            "requires little-endian native Mach-O64; scripts/wrappers are ineligible",
        ));
    }
    let cpu = le(bytes, 4)?;
    if !matches!(
        (std::env::consts::ARCH, cpu),
        ("aarch64", 0x0100000c) | ("x86_64", 0x01000007)
    ) || le(bytes, 12)? != 2
    {
        return Err(error("Mach-O architecture or executable type mismatch"));
    }
    let count = le(bytes, 16)? as usize;
    let length = le(bytes, 20)? as usize;
    if count > 4096 || length > 1024 * 1024 {
        return Err(error("Mach-O load-command budget exceeded"));
    }
    let commands = bytes
        .get(32..32 + length)
        .ok_or_else(|| error("truncated Mach-O load commands"))?;
    let mut offset = 0;
    for _ in 0..count {
        let kind = le(commands, offset)?;
        let size = le(commands, offset + 4)? as usize;
        if size < 8 {
            return Err(error("invalid Mach-O load command"));
        }
        let command = commands
            .get(
                offset
                    ..offset
                        .checked_add(size)
                        .ok_or_else(|| error("command overflow"))?,
            )
            .ok_or_else(|| error("truncated load command"))?;
        if kind == 0x8000001c || kind == 0x27 {
            return Err(error(
                "rpath and dynamic-linker environment commands are ineligible",
            ));
        }
        if matches!(kind, 0xc | 0x80000018 | 0x8000001f | 0x20 | 0x80000023) {
            let start = le(command, 8)? as usize;
            if start < 24 {
                return Err(error("invalid dylib name offset"));
            }
            let raw = command
                .get(start..)
                .ok_or_else(|| error("invalid dylib name"))?;
            let end = raw
                .iter()
                .position(|b| *b == 0)
                .ok_or_else(|| error("unterminated dylib name"))?;
            let name = std::str::from_utf8(&raw[..end]).map_err(error)?;
            if !(name.starts_with("/usr/lib/") || name.starts_with("/System/Library/"))
                || Path::new(name)
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return Err(error("profile allows Apple system dylibs only; local/@rpath libraries need a separate reviewed loader policy"));
            }
        }
        if kind == 0xe {
            let start = le(command, 8)? as usize;
            let raw = command
                .get(start..)
                .ok_or_else(|| error("invalid linker name"))?;
            if !raw.starts_with(b"/usr/lib/dyld\0") {
                return Err(error("non-system dynamic linker"));
            }
        }
        offset += size;
    }
    if offset != commands.len() {
        return Err(error("Mach-O command length mismatch"));
    }
    Ok(())
}
pub fn inspect(
    executable: &Path,
    recognizer: Recognizer,
    cancel: &dyn Fn() -> bool,
) -> Result<Inspection, AlignmentError> {
    check(cancel, Instant::now())?;
    if recognizer != Recognizer::Phonetic {
        return Ok(Inspection::Unavailable("Automatic reuse requires the explicitly enrolled self-contained Phonetic package profile.".into()));
    }
    if !executable.is_absolute() {
        return Err(error("executable must be absolute"));
    }
    let root = executable
        .parent()
        .ok_or_else(|| error("executable has no package directory"))?;
    let manifest = root.join(MANIFEST);
    match fs::symlink_metadata(&manifest) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Inspection::Unavailable("Automatic reuse unavailable: package has not been explicitly enrolled. Estimate still executes normally.".into())),
        Err(e) => return Err(error(e)),
        Ok(metadata) if !metadata.is_file() || metadata.len() > 1024 * 1024 => return Err(error("manifest must be regular and within 1 MiB")),
        _ => {}
    }
    // Reject symlinked package/executable ancestors before canonicalization.
    for ancestor in executable.ancestors() {
        if fs::symlink_metadata(ancestor)
            .map_err(error)?
            .file_type()
            .is_symlink()
        {
            return Err(error("symlinked package paths are ineligible"));
        }
    }
    let mut bytes = Vec::new();
    let file = fs::File::open(&manifest).map_err(error)?;
    if !file.metadata().map_err(error)?.is_file() {
        return Err(error("opened manifest is not regular"));
    }
    file.take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() > 1024 * 1024 {
        return Err(error("manifest exceeds 1 MiB"));
    }
    let declaration: Declaration = serde_json::from_slice(&bytes).map_err(error)?;
    let name = executable
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| error("invalid executable filename"))?;
    if declaration.schema_version != 1
        || declaration.profile != PROFILE
        || declaration.executable != name
        || declaration.tool_version.trim().is_empty()
        || declaration.tool_version.len() > 1024
        || declaration.platform != format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
    {
        return Err(error("unsupported or mismatched enrollment declaration"));
    }
    let runtime = runtime()?;
    static_binary(executable)?;
    if inventory(root, cancel)? != declaration.files {
        return Err(error(
            "package inventory differs from explicit enrollment; re-enroll after reviewing changes",
        ));
    }
    let executable_hash = declaration
        .files
        .get(name)
        .ok_or_else(|| error("executable absent from inventory"))?
        .clone();
    let mut hash = Sha256::new();
    hash.update(b"kilsat-local-phonetic-package-v1\0env=LC_ALL=C;LANG=C;PATH=/usr/bin:/bin;HOME=owned-stage\0");
    hash.update(serde_json::to_vec(&declaration).map_err(error)?);
    hash.update(runtime.as_bytes());
    let environment = VerifiedEnvironment::from_inspected_package(
        format!("{:x}", hash.finalize()),
        executable_hash,
        declaration.tool_version.clone(),
    );
    Ok(Inspection::Eligible(LocalPackage {
        root: root.to_path_buf(),
        declaration,
        declaration_bytes: bytes,
        runtime,
        environment,
    }))
}
/// Explicit local enrollment: the caller asserts that this package's Phonetic
/// recognizer is self-contained. This pins bytes, not third-party audit evidence.
/// Does not execute a process; `expected_version` is checked on every cache miss.
/// Existing declarations are never overwritten.
pub fn enroll(
    executable: &Path,
    expected_version: &str,
    cancel: &dyn Fn() -> bool,
) -> Result<(), AlignmentError> {
    use std::io::Write;
    check(cancel, Instant::now())?;
    if !executable.is_absolute()
        || expected_version.trim().is_empty()
        || expected_version.len() > 1024
    {
        return Err(error(
            "enrollment requires an absolute executable and exact nonempty version",
        ));
    }
    for ancestor in executable.ancestors() {
        if fs::symlink_metadata(ancestor)
            .map_err(error)?
            .file_type()
            .is_symlink()
        {
            return Err(error("symlinked package paths are ineligible"));
        }
    }
    runtime()?;
    static_binary(executable)?;
    let root = executable
        .parent()
        .ok_or_else(|| error("missing package root"))?;
    let declaration = Declaration {
        schema_version: 1,
        profile: PROFILE.into(),
        executable: executable
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or_else(|| error("invalid executable filename"))?
            .into(),
        tool_version: expected_version.into(),
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        files: inventory(root, cancel)?,
    };
    let bytes = serde_json::to_vec_pretty(&declaration).map_err(error)?;
    if bytes.len() > 1024 * 1024 {
        return Err(error("enrollment manifest exceeds 1 MiB"));
    }
    check(cancel, Instant::now())?;
    let path = root.join(MANIFEST);
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(error)?;
    // This newly created path belongs to this operation. On failed/cancelled
    // writes, remove only that path; an existing declaration was never opened.
    let result = output
        .write_all(&bytes)
        .and_then(|()| output.sync_all())
        .map_err(error);
    drop(output);
    if let Err(failure) = result {
        let _ = fs::remove_file(&path);
        return Err(failure);
    }
    if cancel() {
        let _ = fs::remove_file(&path);
        return Err(error("cancelled"));
    }
    Ok(())
}
impl LocalPackage {
    pub fn environment(&self) -> &VerifiedEnvironment {
        &self.environment
    }
    pub fn recheck(&self, cancel: &dyn Fn() -> bool) -> Result<(), AlignmentError> {
        check(cancel, Instant::now())?;
        let path = self.root.join(MANIFEST);
        if !fs::symlink_metadata(&path).map_err(error)?.is_file() {
            return Err(error("manifest replaced by nonregular file"));
        }
        let mut bytes = Vec::new();
        let file = fs::File::open(&path).map_err(error)?;
        if !file.metadata().map_err(error)?.is_file() {
            return Err(error("opened manifest is not regular"));
        }
        file.take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(error)?;
        if runtime()? != self.runtime
            || bytes != self.declaration_bytes
            || inventory(&self.root, cancel)? != self.declaration.files
        {
            return Err(error("package/runtime changed during request"));
        }
        Ok(())
    }
    pub fn execute(
        &self,
        wav: &Path,
        text: &str,
        cancel: &dyn Fn() -> bool,
    ) -> Result<crate::rhubarb::EstimatedVisemeAlignment, AlignmentError> {
        self.recheck(cancel)?;
        let started = Instant::now();
        let snapshot = loop {
            check(cancel, started)?;
            let path = std::env::temp_dir().join(format!(
                "kilsat-phonetic-package-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => break Snapshot(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(error(e)),
            }
        };
        let mut total_copied = 0u64;
        for relative in self.declaration.files.keys() {
            check(cancel, started)?;
            // Inventory generated the names; reject escape paths again before a
            // manifest-derived path is ever used to create a snapshot file.
            if Path::new(relative)
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
            {
                return Err(error("invalid enrolled relative path"));
            }
            let source = self.root.join(relative);
            if !fs::symlink_metadata(&source).map_err(error)?.is_file() {
                return Err(error("package file replaced before snapshot"));
            }
            let destination = snapshot.0.join(relative);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(error)?;
            }
            let mut input = fs::File::open(&source).map_err(error)?;
            if !input.metadata().map_err(error)?.is_file() {
                return Err(error("opened snapshot source is not regular"));
            }
            let mut output = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&destination)
                .map_err(error)?;
            use std::io::Write;
            let mut buffer = [0u8; 65536];
            let mut copied = 0u64;
            loop {
                check(cancel, started)?;
                let count = input.read(&mut buffer).map_err(error)?;
                if count == 0 {
                    break;
                }
                copied += count as u64;
                total_copied += count as u64;
                if copied > MAX_TOTAL || total_copied > MAX_TOTAL {
                    return Err(error("snapshot grew beyond cumulative 512 MiB budget"));
                }
                output.write_all(&buffer[..count]).map_err(error)?;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(
                    &destination,
                    fs::Permissions::from_mode(if *relative == self.declaration.executable {
                        0o500
                    } else {
                        0o400
                    }),
                )
                .map_err(error)?;
            }
        }
        if inventory(&snapshot.0, cancel)? != self.declaration.files {
            return Err(error("snapshot differs from enrolled package"));
        }
        let result = crate::rhubarb::align_controlled(
            &snapshot.0.join(&self.declaration.executable),
            wav,
            text,
            Recognizer::Phonetic,
            &snapshot.0,
            cancel,
        )?;
        if inventory(&snapshot.0, cancel)? != self.declaration.files {
            return Err(error("execution changed package snapshot"));
        }
        self.recheck(cancel)?;
        if result.tool_version != self.declaration.tool_version {
            return Err(error("executed tool version differs from enrollment"));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Root(PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn native_fixture() -> Vec<u8> {
        let mut bytes = vec![0u8; 64];
        if std::env::consts::OS == "macos" {
            bytes[..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
            bytes[4..8].copy_from_slice(
                &(if std::env::consts::ARCH == "aarch64" {
                    0x0100000cu32
                } else {
                    0x01000007u32
                })
                .to_le_bytes(),
            );
            bytes[12..16].copy_from_slice(&2u32.to_le_bytes());
        } else {
            bytes[..6].copy_from_slice(b"\x7fELF\x02\x01");
            bytes[18..20].copy_from_slice(
                &(if std::env::consts::ARCH == "aarch64" {
                    183u16
                } else {
                    62u16
                })
                .to_le_bytes(),
            );
            bytes[54..56].copy_from_slice(&56u16.to_le_bytes());
        }
        bytes
    }
    fn fixture() -> (Root, PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "package-profile-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let root = Root(fs::canonicalize(path).unwrap());
        let executable = root.0.join("rhubarb");
        fs::write(&executable, native_fixture()).unwrap();
        fs::create_dir(root.0.join("resources")).unwrap();
        fs::write(
            root.0.join("resources/phones.dat"),
            b"synthetic fixture; not a recognizer/model",
        )
        .unwrap();
        (root, executable)
    }
    #[test]
    fn enrollment_is_explicit_exact_and_never_replaces_existing_declaration() {
        let (root, executable) = fixture();
        assert!(matches!(
            inspect(&executable, Recognizer::Phonetic, &|| false).unwrap(),
            Inspection::Unavailable(_)
        ));
        enroll(&executable, "synthetic-profile-test", &|| false).unwrap();
        let before = fs::read(root.0.join(MANIFEST)).unwrap();
        assert!(enroll(&executable, "different", &|| false).is_err());
        assert_eq!(fs::read(root.0.join(MANIFEST)).unwrap(), before);
        let Inspection::Eligible(package) =
            inspect(&executable, Recognizer::Phonetic, &|| false).unwrap()
        else {
            panic!("expected eligible local identity")
        };
        package.recheck(&|| false).unwrap();
        fs::write(
            root.0.join("resources/phones.dat"),
            b"changed external resource; unchanged executable",
        )
        .unwrap();
        assert!(package.recheck(&|| false).is_err());
        assert!(inspect(&executable, Recognizer::Phonetic, &|| false).is_err());
    }
    #[test]
    fn unknown_recognizer_scripts_corrupt_manifest_and_cancellation_fail_closed() {
        let (root, executable) = fixture();
        assert!(matches!(
            inspect(&executable, Recognizer::PocketSphinx, &|| false).unwrap(),
            Inspection::Unavailable(_)
        ));
        assert!(enroll(&executable, "test", &|| true).is_err());
        assert!(!root.0.join(MANIFEST).exists());
        fs::write(&executable, b"#!/bin/sh\nprintf wrapper\n").unwrap();
        assert!(enroll(&executable, "test", &|| false).is_err());
        fs::write(root.0.join(MANIFEST), b"{").unwrap();
        assert!(inspect(&executable, Recognizer::Phonetic, &|| false).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn resources_cannot_be_symlinks_and_new_unlisted_files_invalidate() {
        use std::os::unix::fs::symlink;
        let (root, executable) = fixture();
        symlink("phones.dat", root.0.join("resources/link")).unwrap();
        assert!(enroll(&executable, "test", &|| false).is_err());
        fs::remove_file(root.0.join("resources/link")).unwrap();
        enroll(&executable, "test", &|| false).unwrap();
        fs::write(root.0.join("new-resource"), b"not enrolled").unwrap();
        assert!(inspect(&executable, Recognizer::Phonetic, &|| false).is_err());
    }
    #[test]
    fn macho_parser_accepts_only_host_binary_and_fixed_system_dependencies() {
        let mut bytes = vec![0u8; 32];
        bytes[..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
        bytes[4..8].copy_from_slice(
            &(if std::env::consts::ARCH == "aarch64" {
                0x0100000cu32
            } else {
                0x01000007u32
            })
            .to_le_bytes(),
        );
        bytes[12..16].copy_from_slice(&2u32.to_le_bytes());
        assert!(macho(&bytes).is_ok());
        for name in [
            "/usr/lib/libSystem.B.dylib",
            "@rpath/local.dylib",
            "/tmp/private.dylib",
            "/usr/lib/../../tmp/private.dylib",
        ] {
            let mut candidate = bytes.clone();
            let size = 24 + name.len() + 1;
            candidate[16..20].copy_from_slice(&1u32.to_le_bytes());
            candidate[20..24].copy_from_slice(&(size as u32).to_le_bytes());
            let mut command = vec![0; size];
            command[..4].copy_from_slice(&0xcu32.to_le_bytes());
            command[4..8].copy_from_slice(&(size as u32).to_le_bytes());
            command[8..12].copy_from_slice(&24u32.to_le_bytes());
            command[24..24 + name.len()].copy_from_slice(name.as_bytes());
            candidate.extend(command);
            assert_eq!(
                macho(&candidate).is_ok(),
                name == "/usr/lib/libSystem.B.dylib"
            );
        }
        bytes[16..20].copy_from_slice(&1u32.to_le_bytes());
        assert!(macho(&bytes).is_err());
    }
}
