//! ConfigMap 的定义与实现。

use crate::adapters::fcitx5::value::ConfigValue;
use gio::glib;

/// 一个配置节的全部字段，保留返回顺序。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigMap(pub Vec<(String, ConfigValue)>);

impl ConfigMap {
    /// 解析 `GetConfig` 第一个出参（`v` 包着的 `a{sv}`）。
    pub fn from_variant(variant: &glib::Variant) -> Self {
        let mut map = Self::default();
        for index in 0..variant.n_children() {
            let entry = variant.child_value(index);
            let key_variant = entry.child_value(0);
            let Some(key) = key_variant.str() else {
                continue;
            };
            let Some(inner) = entry.child_value(1).get::<glib::Variant>() else {
                continue;
            };
            if let Some(value) = ConfigValue::from_variant(&inner) {
                map.0.push((key.to_owned(), value));
            }
        }
        map
    }

    /// 打包成 `a{sv}`（`SetConfig` 的第二个入参 `v` 的内容）。
    pub fn to_variant(&self) -> glib::Variant {
        let dict = glib::VariantDict::new(None);
        for (key, value) in &self.0 {
            dict.insert_value(key, &value.to_variant());
        }
        dict.end()
    }

    pub fn get(&self, key: &str) -> Option<&ConfigValue> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn text(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(ConfigValue::Text(text)) => Some(text),
            _ => None,
        }
    }

    pub fn boolean(&self, key: &str) -> Option<bool> {
        match self.get(key) {
            Some(ConfigValue::Bool(on)) => Some(*on),
            Some(ConfigValue::Text(value)) if value.eq_ignore_ascii_case("true") => Some(true),
            Some(ConfigValue::Text(value)) if value.eq_ignore_ascii_case("false") => Some(false),
            _ => None,
        }
    }

    pub fn child(&self, key: &str) -> Option<Self> {
        match self.get(key) {
            Some(ConfigValue::Raw(value)) if value.type_().as_str() == "a{sv}" => {
                Some(Self::from_variant(value))
            }
            _ => None,
        }
    }

    /// 覆盖或追加一个字段。
    pub fn set(&mut self, key: &str, value: ConfigValue) {
        if let Some((_, slot)) = self.0.iter_mut().find(|(k, _)| k == key) {
            *slot = value;
        } else {
            self.0.push((key.to_owned(), value));
        }
    }
}
