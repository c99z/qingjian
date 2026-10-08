//! 青简设置入口：GTK4/libadwaita 原生设置应用，见 `ui::app`。

fn main() -> gtk::glib::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();
    qingjian_linux_settings::ui::app::run()
}
