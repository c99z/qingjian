//! 统计页：服务最新值、离线文件与更新时间；查询不改配置草稿。

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gio, glib};
use qingjian_platform::Config;
use qingjian_platform::dirs::linux;

use crate::adapters::statistics;
use crate::ui::pages::{PageContext, SettingsPage};

pub struct StatisticsPage {
    page: adw::PreferencesPage,

    source: adw::ActionRow,

    usage: Vec<adw::ActionRow>,

    vocabulary: Vec<adw::ActionRow>,

    ctx: PageContext,

    weak: RefCell<Weak<Self>>,

    generation: std::cell::Cell<u64>,
}

impl StatisticsPage {
    pub fn new(ctx: &PageContext) -> Rc<Self> {
        let page = adw::PreferencesPage::new();
        let input = adw::PreferencesGroup::new();
        input.set_title("输入量");
        let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
        refresh.set_tooltip_text(Some("刷新统计"));
        input.set_header_suffix(Some(&refresh));
        let source = adw::ActionRow::new();
        source.set_title("数据来源");
        input.add(&source);
        let usage = ["今日", "最近七天", "累计"]
            .iter()
            .map(|title| row(&input, title))
            .collect();
        page.add(&input);
        let words = adw::PreferencesGroup::new();
        words.set_title("词汇学习");
        words.set_description(Some("当前辅助语言的词汇记录"));
        let vocabulary = ["见过", "看熟", "上屏过", "打出过", "本周新见"]
            .iter()
            .map(|title| row(&words, title))
            .collect();
        page.add(&words);
        let this = Rc::new(Self {
            page,
            source,
            usage,
            vocabulary,
            ctx: ctx.clone(),
            weak: RefCell::new(Weak::new()),
            generation: std::cell::Cell::new(0),
        });
        *this.weak.borrow_mut() = Rc::downgrade(&this);
        let weak = Rc::downgrade(&this);
        refresh.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.load();
            }
        });
        let weak = Rc::downgrade(&this);
        this.page.connect_map(move |_| {
            if let Some(this) = weak.upgrade() {
                this.load();
            }
        });
        this
    }

    fn load(&self) {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        self.source.set_subtitle("读取中…");
        let admin = self.ctx.admin.clone();
        let user_dir = linux::user_dir();
        let language = self
            .ctx
            .store
            .borrow()
            .config()
            .general
            .learning_language
            .clone();
        let weak = self.weak.borrow().clone();
        glib::MainContext::default().spawn_local(async move {
            let result =
                gio::spawn_blocking(move || statistics::load(&admin, &user_dir, &language)).await;
            let Some(this) = weak.upgrade() else {
                return;
            };
            if this.generation.get() != generation {
                return;
            }
            match result {
                Ok(Ok(snapshot)) => {
                    this.source.set_subtitle(&snapshot.source);
                    for (row, counts) in this.usage.iter().zip([
                        snapshot.report.usage.today,
                        snapshot.report.usage.week,
                        snapshot.report.usage.total,
                    ]) {
                        row.set_subtitle(&format!(
                            "{} 汉字 · {} 中文词 · {} 英文词 · {} 次上屏",
                            counts.hanzi, counts.words, counts.english_words, counts.commits
                        ));
                    }
                    let counts = snapshot.report.vocabulary;
                    for (row, count) in this.vocabulary.iter().zip([
                        counts.seen,
                        counts.familiar,
                        counts.committed,
                        counts.used,
                        counts.new_this_week,
                    ]) {
                        row.set_subtitle(&format!("{count} 个词"));
                    }
                }
                other => {
                    let error = match other {
                        Ok(Err(error)) => error,
                        _ => "后台任务中断".to_owned(),
                    };
                    this.source
                        .set_subtitle(&format!("读取失败：{error}；下方保留上次结果"));
                }
            }
        });
    }
}

fn row(group: &adw::PreferencesGroup, title: &str) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(title);
    row.set_subtitle("尚未读取");
    group.add(&row);
    row
}

impl SettingsPage for StatisticsPage {
    fn title(&self) -> &'static str {
        "统计"
    }
    fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }
    fn refresh(&self, _: &Config) {}
    fn clear_errors(&self) {}
    fn mark_error(&self, _: &str, _: &str) -> bool {
        false
    }
    fn on_saved(&self) {
        if self.page.is_mapped() {
            self.load();
        }
    }
}
