//! 修饰键输入行：EntryRow 包一层本地校验——合法写法才进草稿，非法写法标红并让窗口挡住「应用」。
//! 写法与配置文件一致：`control+shift+option`（`alt` / `ctrl` 等别名也认），见 [`Modifiers`]。

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use qingjian_platform::Modifiers;

use super::{PageContext, SyncGuard, clear_row_error, mark_row_error};

pub struct ModifierRow {
    row: adw::EntryRow,

    /// 字段路径，如 `shortcut.translation`。
    field: &'static str,

    invalid: Rc<Cell<bool>>,
}

impl ModifierRow {
    pub fn new(ctx: &PageContext, field: &'static str, title: &str, syncing: &SyncGuard) -> Self {
        let row = adw::EntryRow::new();
        row.set_title(title);
        row.set_tooltip_text(Some(
            "修饰键用 + 连接：control / shift / option（即 Alt），如 shift+option",
        ));
        let invalid = Rc::new(Cell::new(false));
        {
            let invalid = invalid.clone();
            let ctx = ctx.clone();
            let syncing = syncing.clone();
            row.connect_changed(move |row| {
                if syncing.get() {
                    return;
                }
                let (section, key) = field.split_once('.').expect("字段路径带分节");
                match row.text().trim().parse::<Modifiers>() {
                    Ok(modifiers) => {
                        invalid.set(false);
                        clear_row_error(row);
                        ctx.set(section, key, modifiers.key());
                    }
                    Err(_) => {
                        invalid.set(true);
                        mark_row_error(row, "写法不合法：修饰键用 + 连接，如 shift+option");
                        (ctx.notify)();
                    }
                }
            });
        }
        Self {
            row,
            field,
            invalid,
        }
    }

    pub fn row(&self) -> &adw::EntryRow {
        &self.row
    }

    pub fn field(&self) -> &'static str {
        self.field
    }

    pub fn invalid(&self) -> bool {
        self.invalid.get()
    }

    /// 回填（调用方已打开 syncing 旗标）：写规范写法并清掉本地错误态。
    pub fn reset(&self, modifiers: Modifiers) {
        self.invalid.set(false);
        clear_row_error(&self.row);
        self.row.set_text(&modifiers.key());
    }
}
