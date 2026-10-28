//! Finds where each tool is installed (design part 3.6): the settings, the resources folder, the tool's default
//! install folder, then `PATH`.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use lazymkv_core::job::Tool;
use lazymkv_core::settings::Settings;
use lazymkv_platform::Host;

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

/// Finds `tool` by the order of design part 3.6 and returns the first hit:
/// 1. the path in `settings.tool_paths`, taken as given and not checked;
/// 2. the tool's file name in `resources`, when that is a file;
/// 3. `host.default_install_path(tool)`, when that is a file;
/// 4. the first folder of `path_var` holding the tool's file name as a file.
///
/// None when no source has it. The skip of folders the owner's account can write to is pull request 3's (S18).
/// Def: T11. Use: T23, T26.
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
        return hit(PathBuf::from(path), Found::Settings);
    }
    let name = exe_name(tool);
    if let Some(path) = resources.map(|folder| folder.join(&name)).filter(|path| path.is_file()) {
        return hit(path, Found::Resources);
    }
    if let Some(path) = host.default_install_path(tool).filter(|path| path.is_file()) {
        return hit(path, Found::InstallFolder);
    }
    path_var
        .into_iter()
        .flat_map(std::env::split_paths)
        .map(|folder| folder.join(&name))
        .find(|path| path.is_file())
        .and_then(|path| hit(path, Found::Path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;
    use lazymkv_core::settings::ToolPaths;
    use std::fs;

    struct TestHost(Option<PathBuf>);

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
        fn user_writable(&self, _: &Path) -> bool { false }
    }

    fn named(path: &str) -> Settings {
        Settings { tool_paths: ToolPaths { mkvmerge: Some(path.to_string()), ..ToolPaths::default() }, ..Settings::default() }
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
        let found = find_tool(Tool::Mkvmerge, &named(r"C:\Tools\mkvmerge.exe"), None, &TestHost(None), None);
        assert_eq!(
            found,
            Some(ToolLocation { tool: Tool::Mkvmerge, path: PathBuf::from(r"C:\Tools\mkvmerge.exe"), found: Found::Settings })
        );
    }

    #[test]
    fn the_lookup_order_is_settings_resources_install_folder_path() {
        let root = temp_dir("lookup-order");
        let place = |folder: &str| {
            let path = root.join(folder).join(exe_name(Tool::Mkvmerge));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, b"x").unwrap();
            path
        };
        let (resources, install, on_path) = (place("resources"), place("install"), place("path"));
        let path_var = std::env::join_paths([root.join("empty"), root.join("path")]).unwrap();
        let find = |settings: &Settings, resources: Option<&Path>, host: &TestHost| {
            find_tool(Tool::Mkvmerge, settings, resources, host, Some(path_var.as_os_str())).map(|l| (l.path, l.found))
        };
        let host = TestHost(Some(install.clone()));
        let default = Settings::default();

        assert_eq!(
            find(&named("given.exe"), Some(&root.join("resources")), &host),
            Some((PathBuf::from("given.exe"), Found::Settings))
        );
        assert_eq!(find(&default, Some(&root.join("resources")), &host), Some((resources, Found::Resources)));
        assert_eq!(find(&default, None, &host), Some((install, Found::InstallFolder)));
        assert_eq!(find(&default, None, &TestHost(None)), Some((on_path, Found::Path)));
    }

    #[test]
    fn a_missing_tool_is_none() {
        let root = temp_dir("lookup-missing");
        fs::create_dir_all(root.join("resources")).unwrap();
        fs::write(root.join("resources").join(exe_name(Tool::Ffprobe)), b"x").unwrap();
        let path_var = std::env::join_paths([root.join("path")]).unwrap();
        let host = TestHost(Some(root.join("nowhere").join(exe_name(Tool::Mkvmerge))));

        let found =
            find_tool(Tool::Mkvmerge, &Settings::default(), Some(&root.join("resources")), &host, Some(path_var.as_os_str()));

        assert_eq!(found, None);
        assert_eq!(find_tool(Tool::Mkvmerge, &Settings::default(), None, &TestHost(None), None), None);
    }

    #[test]
    fn found_serialises_in_snake_case() {
        assert_eq!(serde_json::to_string(&Found::InstallFolder).unwrap(), "\"install_folder\"");
        assert_eq!(serde_json::to_string(&Found::Path).unwrap(), "\"path\"");
    }
}
