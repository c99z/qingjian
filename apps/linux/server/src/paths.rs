//! Linux 的 XDG 路径与资源定位统一由 `qingjian_platform::dirs::linux` 实现（设置程序也用同一份），
//! 本模块只保留原有调用点。
pub use qingjian_platform::dirs::linux::{
    asset, config_path, generated, log_dir, resource_root, user_dir,
};
