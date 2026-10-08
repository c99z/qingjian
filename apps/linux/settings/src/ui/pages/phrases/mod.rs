//! 自定义短语列表：新增、编辑、停用与删除都进入统一草稿。
mod editor;
use crate::ui::pages::{PageContext, SettingsPage};
use adw::prelude::*;
use qingjian_core::CustomPhrase;
use qingjian_platform::Config;
use std::cell::RefCell;
use std::rc::Rc;

pub struct PhrasesPage {
    page: adw::PreferencesPage,

    group: adw::PreferencesGroup,

    rows: RefCell<Vec<adw::ActionRow>>,

    phrases: RefCell<Vec<CustomPhrase>>,

    ctx: PageContext,

    weak: RefCell<std::rc::Weak<Self>>,
}
impl PhrasesPage {
    pub fn new(ctx: &PageContext) -> Rc<Self> {
        let page = adw::PreferencesPage::new();
        let group = adw::PreferencesGroup::builder().title("自定义短语").description("输入码为 1–32 个小写字母，候选位置为 1–9。修改点击「应用」后保存；文本保留空格和换行。").build();
        let add = gtk::Button::with_label("新增短语");
        group.set_header_suffix(Some(&add));
        page.add(&group);
        let this = Rc::new(Self {
            page,
            group,
            rows: Default::default(),
            phrases: Default::default(),
            ctx: ctx.clone(),
            weak: Default::default(),
        });
        *this.weak.borrow_mut() = Rc::downgrade(&this);
        let weak = Rc::downgrade(&this);
        add.connect_clicked(move |button| {
            if let Some(this) = weak.upgrade() {
                editor::open(&this, None, button);
            }
        });
        this
    }
    fn update_draft(&self) {
        self.ctx
            .store
            .borrow_mut()
            .set_phrases(self.phrases.borrow().clone());
        (self.ctx.notify)();
        self.rebuild();
    }
    fn rebuild(&self) {
        for row in self.rows.borrow_mut().drain(..) {
            self.group.remove(&row);
        }
        for (index, phrase) in self.phrases.borrow().iter().enumerate() {
            let row = adw::ActionRow::builder()
                .title(format!("{} · 第 {} 位", phrase.code, phrase.position))
                .subtitle(CustomPhrase::preview(&phrase.text, 80))
                .build();
            let toggle = gtk::Switch::new();
            toggle.set_active(phrase.enabled);
            toggle.set_valign(gtk::Align::Center);
            toggle.set_tooltip_text(Some("启用此短语"));
            let weak = self.weak.borrow().clone();
            toggle.connect_active_notify(move |toggle| {
                if let Some(this) = weak.upgrade() {
                    if let Some(phrase) = this.phrases.borrow_mut().get_mut(index) {
                        phrase.enabled = toggle.is_active();
                    }
                    this.update_draft();
                }
            });
            row.add_suffix(&toggle);
            let edit = gtk::Button::with_label("编辑");
            edit.set_valign(gtk::Align::Center);
            let weak = self.weak.borrow().clone();
            edit.connect_clicked(move |button| {
                if let Some(this) = weak.upgrade() {
                    editor::open(&this, Some(index), button);
                }
            });
            row.add_suffix(&edit);
            let delete = gtk::Button::with_label("删除");
            delete.set_valign(gtk::Align::Center);
            let weak = self.weak.borrow().clone();
            delete.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.phrases.borrow_mut().remove(index);
                    this.update_draft();
                }
            });
            row.add_suffix(&delete);
            self.group.add(&row);
            self.rows.borrow_mut().push(row);
        }
    }
}
impl SettingsPage for PhrasesPage {
    fn title(&self) -> &'static str {
        "短语"
    }
    fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }
    fn refresh(&self, config: &Config) {
        *self.phrases.borrow_mut() = config.custom_phrases.clone();
        self.rebuild();
    }
    fn clear_errors(&self) {
        self.group.remove_css_class("error");
    }
    fn mark_error(&self, field: &str, message: &str) -> bool {
        if field == "custom_phrases" {
            self.group.add_css_class("error");
            self.group.set_tooltip_text(Some(message));
            return true;
        }
        false
    }
}
