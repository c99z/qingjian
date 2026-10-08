//! 随包和用户词库启停；导入、移除作为独立操作立即执行。
mod operations;
use crate::adapters::dictionaries;
use crate::ui::pages::{PageContext, SettingsPage};
use adw::prelude::*;
use qingjian_platform::{Config, dirs::linux};
use std::cell::Cell;
use std::rc::Rc;

pub struct DictionariesPage {
    page: adw::PreferencesPage,

    ctx: PageContext,

    group: adw::PreferencesGroup,

    status: adw::ActionRow,

    rows: std::cell::RefCell<Vec<adw::PreferencesRow>>,

    generation: Cell<u64>,

    weak: std::cell::RefCell<std::rc::Weak<Self>>,
}
impl DictionariesPage {
    pub fn new(ctx: &PageContext) -> Rc<Self> {
        let page = adw::PreferencesPage::new();
        let group = adw::PreferencesGroup::builder()
            .title("附加词库")
            .description(
                "开关修改后点「应用」。主词库始终可用；随包领域词库可以启停，只有用户词库可移除。",
            )
            .build();
        page.add(&group);
        let action = adw::PreferencesGroup::builder().title("导入与移除").description("独立操作，立即修改词库文件；导入支持 .qj、TSV 和 Rime .dict.yaml。搜狗 .scel 请先用项目转换脚本转换。").build();
        let status = adw::ActionRow::builder()
            .title("导入用户词库")
            .subtitle("尚未导入")
            .build();
        let button = gtk::Button::with_label("选择文件…");
        button.set_valign(gtk::Align::Center);
        status.add_suffix(&button);
        action.add(&status);
        page.add(&action);
        let this = Rc::new(Self {
            page,
            ctx: ctx.clone(),
            group,
            status,
            rows: Default::default(),
            generation: Cell::new(0),
            weak: Default::default(),
        });
        *this.weak.borrow_mut() = Rc::downgrade(&this);
        let weak = Rc::downgrade(&this);
        button.connect_clicked(move |button| {
            if let Some(page) = weak.upgrade() {
                operations::choose_import(&page, button);
            }
        });
        this
    }
    fn reload_list(&self) {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        let weak = self.weak.borrow().clone();
        gtk::glib::MainContext::default().spawn_local(async move {
            let result = gtk::gio::spawn_blocking(move || {
                dictionaries::list(linux::resource_root().as_deref(), &linux::user_dir())
            })
            .await;
            let Some(this) = weak.upgrade() else {
                return;
            };
            if this.generation.get() != generation {
                return;
            }
            let Ok(items) = result else {
                this.status.set_subtitle("词库列表读取失败");
                return;
            };
            for row in this.rows.borrow_mut().drain(..) {
                this.group.remove(&row);
            }
            let config = this
                .ctx
                .store
                .borrow()
                .draft()
                .unwrap_or_else(|_| this.ctx.store.borrow().config().clone());
            for item in items {
                let row = adw::SwitchRow::builder()
                    .title(&item.stem)
                    .subtitle(&item.description)
                    .build();
                row.set_tooltip_text(Some(&item.path.display().to_string()));
                row.set_active(if item.bundled {
                    config.dictionaries.is_domain_enabled(&item.stem)
                } else {
                    config.dictionaries.is_enabled(&item.stem)
                });
                let c = this.ctx.clone();
                let name = item.stem.clone();
                let bundled = item.bundled;
                row.connect_active_notify(move |row| {
                    let mut values = match c.store.borrow().draft() {
                        Ok(c) => {
                            if bundled {
                                c.dictionaries.domains
                            } else {
                                c.dictionaries.disabled
                            }
                        }
                        Err(_) => return,
                    };
                    values.retain(|value| value != &name);
                    if row.is_active() == bundled {
                        values.push(name.clone());
                    }
                    let strings: Vec<&str> = values.iter().map(String::as_str).collect();
                    c.set_array(
                        "dictionaries",
                        if bundled { "domains" } else { "disabled" },
                        &strings,
                    );
                });
                this.group.add(&row);
                this.rows.borrow_mut().push(row.upcast());
                if !item.bundled {
                    let remove_row = adw::ActionRow::builder()
                        .title(format!("移除 {}", item.stem))
                        .build();
                    let button = gtk::Button::with_label("移除");
                    button.set_valign(gtk::Align::Center);
                    let weak = this.weak.borrow().clone();
                    let name = item.stem;
                    button.connect_clicked(move |button| {
                        if let Some(this) = weak.upgrade() {
                            operations::confirm_remove(&this, &name, button);
                        }
                    });
                    remove_row.add_suffix(&button);
                    this.group.add(&remove_row);
                    this.rows.borrow_mut().push(remove_row.upcast());
                }
            }
        });
    }
}
impl SettingsPage for DictionariesPage {
    fn title(&self) -> &'static str {
        "词库"
    }
    fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }
    fn refresh(&self, _: &Config) {
        self.reload_list();
    }
    fn on_saved(&self) {
        self.reload_list();
    }
    fn clear_errors(&self) {}
    fn mark_error(&self, _: &str, _: &str) -> bool {
        false
    }
}
