//! Linux 的 XDG 路径约定：配置、用户数据、日志与随包资源定位，Server 与设置程序共用。
//!
//! 只查环境变量、不碰系统 API：`XDG_*` 不是绝对路径就忽略，回落 `$HOME` 下的缺省位置，
//! 都没有时回落当前目录。`QINGJIAN_RESOURCES` 可强制指定随包资源根（开发与打包测试用）。

use std::path::{Path, PathBuf};

/// `$XDG_<var>/qingjian`：环境变量不是绝对路径就当没设，回落 `$HOME/<fallback>`，再不行用当前目录。
fn xdg_dir(var: Option<PathBuf>, home: Option<PathBuf>, fallback: &str) -> PathBuf {
    var.filter(|p| p.is_absolute())
        .or_else(|| home.map(|h| h.join(fallback)))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("qingjian")
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

/// 用户数据目录 `$XDG_DATA_HOME/qingjian`（缺省 `~/.local/share/qingjian`）：
/// 学习数据、统计、输入日志与用户词库（`dicts/`）、码表（`codes/`）。
pub fn user_dir() -> PathBuf {
    xdg_dir(env_path("XDG_DATA_HOME"), home(), ".local/share")
}

/// 配置文件 `$XDG_CONFIG_HOME/qingjian/config.toml`（缺省 `~/.config/qingjian/config.toml`）。
pub fn config_path() -> PathBuf {
    xdg_dir(env_path("XDG_CONFIG_HOME"), home(), ".config").join("config.toml")
}

/// 运行日志目录 `$XDG_STATE_HOME/qingjian/logs`（缺省 `~/.local/state/qingjian/logs`），不负责创建。
pub fn log_dir() -> PathBuf {
    xdg_dir(env_path("XDG_STATE_HOME"), home(), ".local/state").join("logs")
}

fn home() -> Option<PathBuf> {
    env_path("HOME")
}

/// 随包资源的根目录（其下有 `data/` 与 `assets/`）：`QINGJIAN_RESOURCES` 优先，
/// 其次安装布局（exe 上两级的 `share/qingjian/resources`），最后开发布局（见 [`crate::resources::bundled_root`]）。
pub fn resource_root() -> Option<PathBuf> {
    if let Some(root) = env_path("QINGJIAN_RESOURCES") {
        return Some(root);
    }
    let executable = std::env::current_exe().ok()?;
    let prefix = executable.parent()?.parent()?;
    let installed = prefix.join("share/qingjian/resources");
    if installed.join("assets").is_dir() || installed.join("data").is_dir() {
        return Some(installed);
    }
    crate::resources::bundled_root()
}

/// 存在的文件才返回。
pub fn existing(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then_some(path)
}

/// 资源根 `data/generated/` 下的一个产品数据文件；不存在为 `None`。
pub fn generated(root: &Path, name: &str) -> Option<PathBuf> {
    existing(root.join("data/generated").join(name))
}

/// 资源根 `assets/` 下的一个随包资源文件；不存在为 `None`。
pub fn asset(root: &Path, name: &str) -> Option<PathBuf> {
    existing(root.join("assets").join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_dir_prefers_absolute_env_then_home_fallback() {
        let home = Some(PathBuf::from("/home/u"));
        assert_eq!(
            xdg_dir(Some(PathBuf::from("/data")), home.clone(), ".local/share"),
            PathBuf::from("/data/qingjian")
        );
        // 相对路径的 XDG 变量按规范忽略
        assert_eq!(
            xdg_dir(
                Some(PathBuf::from("relative")),
                home.clone(),
                ".local/share"
            ),
            PathBuf::from("/home/u/.local/share/qingjian")
        );
        assert_eq!(
            xdg_dir(None, home, ".local/share"),
            PathBuf::from("/home/u/.local/share/qingjian")
        );
        assert_eq!(
            xdg_dir(None, None, ".local/share"),
            PathBuf::from("./qingjian")
        );
    }

    #[test]
    fn generated_and_asset_only_return_existing_files() {
        let root = std::env::temp_dir().join(format!("qingjian-linux-dirs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("data/generated")).unwrap();
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::write(root.join("data/generated/dict.qj"), b"").unwrap();
        assert!(generated(&root, "dict.qj").is_some());
        assert!(generated(&root, "missing.qj").is_none());
        assert!(asset(&root, "missing.tsv").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
