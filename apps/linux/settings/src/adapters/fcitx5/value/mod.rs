//! Fcitx5 配置值在 D-Bus 上的线格式：`GetConfig` 回 `a{sv}`，键名字段值；
//! 颜色、字体在 fcitx5 里都是字符串。这里只承接 classicui 实际用到的三种类型（字符串 / 布尔 / 整数）。

mod item;
pub use item::ConfigValue;
mod map;
pub use map::ConfigMap;

use gio::glib;

/// 从 `GetConfig` 的字段描述（`a(sa(sssva{sv}))`）里取某个字段的枚举选项，
/// 返回按描述顺序排的 `(值, 显示名)`；该字段不是枚举或没有描述时为空。
pub fn enum_options(description: &glib::Variant, field: &str) -> Vec<(String, String)> {
    for section in 0..description.n_children() {
        let fields = description.child_value(section).child_value(1);
        for index in 0..fields.n_children() {
            let entry = fields.child_value(index);
            if entry.child_value(0).str() != Some(field) {
                continue;
            }
            let options = entry.child_value(4);
            let names = string_map(&options, "Enum");
            let labels = string_map(&options, "EnumI18n");
            return names
                .into_iter()
                .map(|(index, name)| {
                    let label = labels
                        .iter()
                        .find(|(i, _)| *i == index)
                        .map(|(_, l)| l.clone())
                        .unwrap_or_else(|| name.clone());
                    (name, label)
                })
                .collect();
        }
    }
    Vec::new()
}

/// 字段描述里 `a{sv}` 选项下的一个 `a{sv}` 子表（下标串 → 值）。
fn string_map(options: &glib::Variant, key: &str) -> Vec<(u32, String)> {
    for index in 0..options.n_children() {
        let entry = options.child_value(index);
        if entry.child_value(0).str() != Some(key) {
            continue;
        }
        let Some(inner) = entry.child_value(1).get::<glib::Variant>() else {
            continue;
        };
        let mut pairs: Vec<(u32, String)> = (0..inner.n_children())
            .filter_map(|i| {
                let item = inner.child_value(i);
                let index_variant = item.child_value(0);
                let index = index_variant.str()?.parse().ok()?;
                let value = item.child_value(1).get::<glib::Variant>()?;
                Some((index, value.get::<String>()?))
            })
            .collect();
        pairs.sort_by_key(|(index, _)| *index);
        return pairs;
    }
    Vec::new()
}

#[cfg(test)]
mod tests;
