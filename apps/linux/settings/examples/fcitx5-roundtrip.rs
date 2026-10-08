//! 真实 D-Bus 往返：字体、预设主题、四种候选颜色与释义比例，结束后恢复原配置及主题。

#[path = "support/backup.rs"]
mod backup;

use std::process::ExitCode;

use qingjian_linux_settings::adapters::fcitx5::{
    self, AppearanceValues, ClassicUiPanel, Fcitx5Client, Fcitx5Error, FontDesc,
};

fn main() -> ExitCode {
    let client = match Fcitx5Client::connect() {
        Ok(client) => client,
        Err(Fcitx5Error::NotRunning) => {
            println!("Fcitx5 未运行，跳过");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("{}", error.user_message());
            return ExitCode::FAILURE;
        }
    };
    let original = match backup::Backup::new(&client) {
        Ok(backup) => backup,
        Err(error) => {
            eprintln!("备份失败：{error}");
            return ExitCode::FAILURE;
        }
    };
    let result = exercise(&client);
    let restore = original.restore(&client);
    if let Err(error) = &result {
        eprintln!("验证失败：{error}");
    }
    if let Err(error) = &restore {
        eprintln!("恢复失败：{error}");
    }
    if result.is_ok() && restore.is_ok() {
        println!("已恢复原配置及主题，往返验证通过");
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn exercise(client: &Fcitx5Client) -> Result<(), String> {
    let run = || -> Result<(), Fcitx5Error> {
        let panels = client.probe_panels()?;
        println!("面板检测：{panels:?}");
        if panels.classic_ui != ClassicUiPanel::Active {
            return Err(Fcitx5Error::PanelUnavailable);
        }
        let base = client.classic_ui()?;
        fcitx5::apply(
            client,
            &base,
            &AppearanceValues {
                font: Some(FontDesc::parse("Sans Bold 13")),
                ..AppearanceValues::default()
            },
        )?;
        println!("字体读回通过：Sans Bold 13");
        let base = client.classic_ui()?;
        fcitx5::apply(
            client,
            &base,
            &AppearanceValues {
                theme: Some("default-dark".to_owned()),
                ..AppearanceValues::default()
            },
        )?;
        println!("深色预设读回通过");
        let base = client.classic_ui()?;
        fcitx5::apply(
            client,
            &base,
            &AppearanceValues {
                normal_color: Some("#223344".to_owned()),
                highlight_color_text: Some("#ddeeff".to_owned()),
                comment_color: Some("#556677".to_owned()),
                highlight_comment_color: Some("#aabbcc".to_owned()),
                comment_size_factor: Some(100),
                ..AppearanceValues::default()
            },
        )?;
        println!("定制主题四种颜色与释义同字号实时读回通过");
        let base = client.classic_ui()?;
        fcitx5::apply(
            client,
            &base,
            &AppearanceValues {
                comment_size_factor: Some(90),
                ..AppearanceValues::default()
            },
        )?;
        println!("已选定制主题的修改与重载读回通过");
        Ok(())
    };
    run().map_err(|e| e.user_message())
}
