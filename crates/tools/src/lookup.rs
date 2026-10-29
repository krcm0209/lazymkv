//! Finds where each tool is installed (design part 3.6): the settings, the resources folder, the tool's default
//! install folder, then `PATH`, skipping what a non-elevated process of the user could write (S18, Ruling W18).

use std::env::consts::EXE_SUFFIX;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use lazymkv_core::job::Tool;
use lazymkv_core::settings::Settings;
use lazymkv_platform::Host;

use crate::record::with_suffix;

/// Where `find_tool` found a tool (design part 3.6, S18). Def: T11. Use: T23, T26.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Found {
    /// The owner named it in the settings.
    Settings,
    /// It sits in the resources folder beside the engine.
    Resources,
    /// It sits in the tool's default install folder.
    InstallFolder,
    /// It sits in a folder of the `PATH`.
    Path,
}

/// A tool and the place `find_tool` found it (design part 3.6, S18). Def: T11. Use: T23, T26.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolLocation {
    /// Which tool.
    pub tool: Tool,
    /// The program's path.
    pub path: PathBuf,
    /// Which source gave the path.
    pub found: Found,
}

/// The program file name of `tool` on this operating system, such as `mkvmerge.exe` on Windows (design part 3.6).
/// MakeMKV's is `makemkvcon64`. Def: T11. Use: T13, T14, T23.
pub fn exe_name(tool: Tool) -> String {
    let stem = match tool {
        Tool::Makemkvcon => "makemkvcon64",
        Tool::Mkvmerge => "mkvmerge",
        Tool::Ffprobe => "ffprobe",
        Tool::Ffmpeg => "ffmpeg",
        Tool::Java => "java",
    };
    format!("{stem}{}", std::env::consts::EXE_SUFFIX)
}

/// Finds `tool` by the order of design part 3.6 and returns the first hit (S18; Ruling W18):
/// 1. the path in `settings.tool_paths`, taken as given even when a non-elevated process could write it; when that
///    path is not a file and `EXE_SUFFIX` is not empty, the path with `EXE_SUFFIX` appended, when that is a file
///    (engine.md:583);
/// 2. the tool's file name in `resources`, when that is a file and `host.user_writable(resources)` is false;
/// 3. `host.default_install_path(tool)`, whose install folder is read from the registry on Windows, never from the
///    environment (Ruling W18, amended after the whole-branch review (F1)), when that is a file; it is not judged,
///    since Windows' own access lists protect the Program Files install folders (design part 3.6);
/// 4. the tool's file name in the first absolute folder of `path_var` that holds it as a file, where
///    `host.user_writable` calls neither the folder nor the file writable; relative and empty entries are dropped
///    (engine.md:580).
///
/// None when no source has it. Def: T11, T3-8. Use: T23, T26.
pub fn find_tool(
    tool: Tool,
    settings: &Settings,
    resources: Option<&Path>,
    host: &dyn Host,
    path_var: Option<&OsStr>,
) -> Option<ToolLocation> {
    let hit = |path: PathBuf, found: Found| Some(ToolLocation { tool, path, found });
    let paths = &settings.tool_paths;
    let named = match tool {
        Tool::Makemkvcon => &paths.makemkvcon,
        Tool::Mkvmerge => &paths.mkvmerge,
        Tool::Ffprobe => &paths.ffprobe,
        Tool::Ffmpeg => &paths.ffmpeg,
        Tool::Java => &paths.java,
    };
    if let Some(path) = named {
        let given = PathBuf::from(path);
        if !given.is_file() && !EXE_SUFFIX.is_empty() {
            let suffixed = with_suffix(&given, EXE_SUFFIX);
            if suffixed.is_file() {
                return hit(suffixed, Found::Settings);
            }
        }
        return hit(given, Found::Settings);
    }
    let name = exe_name(tool);
    if let Some(folder) = resources {
        let path = folder.join(&name);
        if path.is_file() && !host.user_writable(folder) {
            return hit(path, Found::Resources);
        }
    }
    if let Some(path) = host.default_install_path(tool).filter(|path| path.is_file()) {
        return hit(path, Found::InstallFolder);
    }
    path_folders(path_var)
        .into_iter()
        .find_map(|folder| {
            let file = folder.join(&name);
            (file.is_file() && !host.user_writable(&folder) && !host.user_writable(&file)).then_some(file)
        })
        .and_then(|path| hit(path, Found::Path))
}

/// The absolute folders of `path_var`, in order; relative and empty entries are dropped (engine.md:580).
fn path_folders(path_var: Option<&OsStr>) -> Vec<PathBuf> {
    path_var.into_iter().flat_map(std::env::split_paths).filter(|folder| folder.is_absolute()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;
    use lazymkv_core::settings::ToolPaths;
    use std::fs;

    /// A host with an optional default install path and the paths it calls user-writable, compared exactly.
    struct TestHost(Option<PathBuf>, Vec<PathBuf>);

    impl Host for TestHost {
        fn now_unix(&self) -> u64 { 0 }
        fn now_ms(&self) -> u64 { 0 }
        fn local_date(&self) -> String { String::new() }
        fn local_time(&self, _: u64) -> String { String::new() }
        fn makemkv_app_key(&self) -> Option<String> { None }
        fn makemkv_debug_log(&self) -> Option<PathBuf> { None }
        fn running_processes(&self) -> Vec<String> { Vec::new() }
        fn volume_label(&self, _: &Path) -> Option<String> { None }
        fn default_install_path(&self, _: Tool) -> Option<PathBuf> { self.0.clone() }
        fn user_writable(&self, path: &Path) -> bool { self.1.iter().any(|writable| writable == path) }
    }

    fn named(path: &str) -> Settings {
        Settings { tool_paths: ToolPaths { mkvmerge: Some(path.to_string()), ..ToolPaths::default() }, ..Settings::default() }
    }

    /// Writes mkvmerge's file name into `root/<folder>` and returns (folder, file).
    fn place(root: &Path, folder: &str) -> (PathBuf, PathBuf) {
        let dir = root.join(folder);
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join(exe_name(Tool::Mkvmerge));
        fs::write(&file, b"x").unwrap();
        (dir, file)
    }

    #[test]
    fn exe_names_carry_the_platform_suffix() {
        let suffix = std::env::consts::EXE_SUFFIX;
        assert_eq!(exe_name(Tool::Makemkvcon), format!("makemkvcon64{suffix}"));
        assert_eq!(exe_name(Tool::Mkvmerge), format!("mkvmerge{suffix}"));
        assert_eq!(exe_name(Tool::Ffprobe), format!("ffprobe{suffix}"));
        assert_eq!(exe_name(Tool::Ffmpeg), format!("ffmpeg{suffix}"));
        assert_eq!(exe_name(Tool::Java), format!("java{suffix}"));
    }

    #[test]
    fn the_settings_path_is_taken_as_given() {
        let found = find_tool(Tool::Mkvmerge, &named(r"C:\Tools\mkvmerge.exe"), None, &TestHost(None, vec![]), None);
        assert_eq!(
            found,
            Some(ToolLocation { tool: Tool::Mkvmerge, path: PathBuf::from(r"C:\Tools\mkvmerge.exe"), found: Found::Settings })
        );
    }

    /// Design part 3.6's order holds under W18. The default install folder is used even when the host calls it
    /// writable: the rule judges only `PATH` and `resources`, and Windows protects Program Files.
    #[test]
    fn the_order_stays_settings_resources_install_folder_path() {
        let root = temp_dir("lookup-order");
        let (resources, in_resources) = place(&root, "resources");
        let (install_folder, install) = place(&root, "install");
        let (on_path, in_path) = place(&root, "path");
        let path_var = std::env::join_paths([root.join("empty"), on_path]).unwrap();
        let find = |settings: &Settings, resources: Option<&Path>, host: &TestHost| {
            find_tool(Tool::Mkvmerge, settings, resources, host, Some(path_var.as_os_str())).map(|l| (l.path, l.found))
        };
        let host = TestHost(Some(install.clone()), vec![]);
        let default = Settings::default();
        assert_eq!(find(&named("given.exe"), Some(&resources), &host), Some((PathBuf::from("given.exe"), Found::Settings)));
        assert_eq!(find(&default, Some(&resources), &host), Some((in_resources, Found::Resources)));
        assert_eq!(find(&default, None, &host), Some((install.clone(), Found::InstallFolder)));
        let judged = TestHost(Some(install.clone()), vec![install_folder, install.clone()]);
        assert_eq!(find(&default, None, &judged), Some((install, Found::InstallFolder)));
        assert_eq!(find(&default, None, &TestHost(None, vec![])), Some((in_path, Found::Path)));
    }

    #[test]
    fn a_relative_or_empty_path_entry_is_skipped() {
        let root = temp_dir("lookup-relative");
        let (on_path, file) = place(&root, "path");
        let path_var = std::env::join_paths([PathBuf::new(), PathBuf::from("relative"), on_path.clone()]).unwrap();
        assert_eq!(path_folders(Some(path_var.as_os_str())), vec![on_path]);
        let found = find_tool(Tool::Mkvmerge, &Settings::default(), None, &TestHost(None, vec![]), Some(path_var.as_os_str()));
        assert_eq!(found.map(|l| (l.path, l.found)), Some((file, Found::Path)));
    }

    #[test]
    fn a_writable_path_folder_is_skipped_for_the_next_one() {
        let root = temp_dir("lookup-writable-folder");
        let (first, _) = place(&root, "first");
        let (second, file) = place(&root, "second");
        let host = TestHost(None, vec![first.clone()]);
        let both = std::env::join_paths([&first, &second]).unwrap();
        let found = find_tool(Tool::Mkvmerge, &Settings::default(), None, &host, Some(both.as_os_str()));
        assert_eq!(found.map(|l| (l.path, l.found)), Some((file, Found::Path)));
        let only_first = std::env::join_paths([&first]).unwrap();
        assert_eq!(find_tool(Tool::Mkvmerge, &Settings::default(), None, &host, Some(only_first.as_os_str())), None);
    }

    #[test]
    fn a_writable_program_file_in_a_protected_folder_is_skipped() {
        let root = temp_dir("lookup-writable-file");
        let (first, first_file) = place(&root, "first");
        let (second, file) = place(&root, "second");
        let host = TestHost(None, vec![first_file]);
        let both = std::env::join_paths([&first, &second]).unwrap();
        let found = find_tool(Tool::Mkvmerge, &Settings::default(), None, &host, Some(both.as_os_str()));
        assert_eq!(found.map(|l| (l.path, l.found)), Some((file, Found::Path)));
    }

    #[test]
    fn a_writable_resources_folder_is_skipped() {
        let root = temp_dir("lookup-writable-resources");
        let (resources, _) = place(&root, "resources");
        let (on_path, file) = place(&root, "path");
        let host = TestHost(None, vec![resources.clone()]);
        let path_var = std::env::join_paths([&on_path]).unwrap();
        let found = find_tool(Tool::Mkvmerge, &Settings::default(), Some(&resources), &host, Some(path_var.as_os_str()));
        assert_eq!(found.map(|l| (l.path, l.found)), Some((file, Found::Path)));
        assert_eq!(find_tool(Tool::Mkvmerge, &Settings::default(), Some(&resources), &host, None), None);
    }

    #[test]
    fn a_settings_path_is_taken_as_given_even_when_writable() {
        let root = temp_dir("lookup-writable-settings");
        let (folder, file) = place(&root, "named");
        let host = TestHost(None, vec![folder, file.clone()]);
        let found = find_tool(Tool::Mkvmerge, &named(file.to_str().unwrap()), None, &host, None);
        assert_eq!(found, Some(ToolLocation { tool: Tool::Mkvmerge, path: file, found: Found::Settings }));
    }

    #[test]
    fn a_settings_path_without_its_suffix_finds_the_file_with_it() {
        let root = temp_dir("lookup-suffix");
        let (folder, file) = place(&root, "named");
        let host = TestHost(None, vec![]);
        // Windows: `<folder>\mkvmerge` finds `mkvmerge.exe`. Elsewhere EXE_SUFFIX is empty and the given path is the file.
        let given = folder.join("mkvmerge");
        let found = find_tool(Tool::Mkvmerge, &named(given.to_str().unwrap()), None, &host, None);
        assert_eq!(found.map(|l| (l.path, l.found)), Some((file, Found::Settings)));
        // Neither the path nor the path with the suffix is a file: the given path, which the callers' is_file filter
        // then counts as missing (design part 3.7).
        let absent = folder.join("absent");
        let found = find_tool(Tool::Mkvmerge, &named(absent.to_str().unwrap()), None, &host, None);
        assert_eq!(found.map(|l| l.path), Some(absent));
    }

    #[test]
    fn a_missing_tool_is_none() {
        let root = temp_dir("lookup-missing");
        fs::create_dir_all(root.join("resources")).unwrap();
        fs::write(root.join("resources").join(exe_name(Tool::Ffprobe)), b"x").unwrap();
        let path_var = std::env::join_paths([root.join("path")]).unwrap();
        let host = TestHost(Some(root.join("nowhere").join(exe_name(Tool::Mkvmerge))), vec![]);

        let found =
            find_tool(Tool::Mkvmerge, &Settings::default(), Some(&root.join("resources")), &host, Some(path_var.as_os_str()));

        assert_eq!(found, None);
        assert_eq!(find_tool(Tool::Mkvmerge, &Settings::default(), None, &TestHost(None, vec![]), None), None);
    }

    #[test]
    fn found_serialises_in_snake_case() {
        assert_eq!(serde_json::to_string(&Found::InstallFolder).unwrap(), "\"install_folder\"");
        assert_eq!(serde_json::to_string(&Found::Path).unwrap(), "\"path\"");
    }
}
