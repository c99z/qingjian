//! 短语编辑对话框，非法输入留在对话框中而不覆盖草稿。
use super::PhrasesPage;
use adw::prelude::*;
use qingjian_core::CustomPhrase;
use qingjian_core::custom_phrase::validate_phrases;
use std::rc::Rc;

pub(super) fn open(page: &Rc<PhrasesPage>, index: Option<usize>, button: &gtk::Button) {
    let initial = index
        .and_then(|i| page.phrases.borrow().get(i).cloned())
        .unwrap_or(CustomPhrase {
            code: String::new(),
            text: String::new(),
            position: 1,
            enabled: true,
        });
    let dialog = adw::AlertDialog::builder()
        .heading(if index.is_some() {
            "编辑短语"
        } else {
            "新增短语"
        })
        .build();
    let form = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let code = adw::EntryRow::builder()
        .title("输入码")
        .text(&initial.code)
        .build();
    form.append(&code);
    let position = adw::SpinRow::with_range(1.0, 9.0, 1.0);
    position.set_title("候选位置");
    position.set_value(initial.position as f64);
    form.append(&position);
    let enabled = adw::SwitchRow::builder()
        .title("启用")
        .active(initial.enabled)
        .build();
    form.append(&enabled);
    let label = gtk::Label::new(Some("短语文本（支持换行）"));
    label.set_xalign(0.0);
    form.append(&label);
    let text = gtk::TextView::new();
    text.set_wrap_mode(gtk::WrapMode::WordChar);
    text.buffer().set_text(&initial.text);
    let scroll = gtk::ScrolledWindow::builder()
        .min_content_height(120)
        .child(&text)
        .build();
    form.append(&scroll);
    let error = gtk::Label::new(None);
    error.set_wrap(true);
    error.add_css_class("error");
    form.append(&error);
    dialog.set_extra_child(Some(&form));
    dialog.add_responses(&[("cancel", "返回"), ("save", "加入草稿")]);
    dialog.set_close_response("cancel");
    dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
    // 响应按钮点击前校验；非法时禁止确认，输入变化立即恢复可编辑状态。
    let check = {
        let code = code.clone();
        let position = position.clone();
        let text = text.clone();
        let enabled = enabled.clone();
        let page = page.clone();
        let error = error.clone();
        let dialog = dialog.clone();
        Rc::new(move || {
            let mut phrases = page.phrases.borrow().clone();
            let phrase = value(&code, &position, &text, &enabled);
            if let Some(i) = index {
                phrases[i] = phrase;
            } else {
                phrases.push(phrase);
            }
            match validate_phrases(&phrases) {
                Ok(()) => {
                    error.set_text("");
                    dialog.set_response_enabled("save", true);
                }
                Err(message) => {
                    error.set_text(&message);
                    dialog.set_response_enabled("save", false);
                }
            }
        })
    };
    let c = check.clone();
    code.connect_changed(move |_| c());
    let c = check.clone();
    position.connect_value_notify(move |_| c());
    let c = check.clone();
    text.buffer().connect_changed(move |_| c());
    check();
    let page = page.clone();
    dialog.connect_response(None, move |_, response| {
        if response != "save" {
            return;
        }
        let phrase = value(&code, &position, &text, &enabled);
        if let Some(i) = index {
            page.phrases.borrow_mut()[i] = phrase;
        } else {
            page.phrases.borrow_mut().push(phrase);
        }
        page.update_draft();
    });
    let parent = button.root().and_then(|r| r.downcast::<gtk::Window>().ok());
    dialog.present(parent.as_ref());
}
fn value(
    code: &adw::EntryRow,
    position: &adw::SpinRow,
    text: &gtk::TextView,
    enabled: &adw::SwitchRow,
) -> CustomPhrase {
    let buffer = text.buffer();
    CustomPhrase {
        code: code.text().to_string(),
        text: buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), true)
            .to_string(),
        position: position.value() as usize,
        enabled: enabled.is_active(),
    }
}
