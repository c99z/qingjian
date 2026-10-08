//! 构造内存里的 Variant 验证解析：classicui 配置读取、枚举描述、回写打包。

use gio::glib;
use gio::glib::prelude::*;

use super::{ConfigMap, ConfigValue, enum_options};

/// 造一个 `a{sv}`。
fn map_variant(entries: &[(&str, glib::Variant)]) -> glib::Variant {
    let dict = glib::VariantDict::new(None);
    for (key, value) in entries {
        dict.insert_value(key, value);
    }
    dict.end()
}

#[test]
fn config_map_roundtrip() {
    let variant = map_variant(&[
        ("Font", "Sans Bold 12".to_variant()),
        ("Theme", "default".to_variant()),
        ("Vertical Candidate List", false.to_variant()),
        ("ForceWaylandDPI", 0i32.to_variant()),
    ]);
    let map = ConfigMap::from_variant(&variant);
    assert_eq!(map.text("Font"), Some("Sans Bold 12"));
    assert_eq!(map.text("Theme"), Some("default"));
    assert_eq!(map.boolean("Vertical Candidate List"), Some(false));
    assert_eq!(map.get("ForceWaylandDPI"), Some(&ConfigValue::Int(0)));

    let packed = ConfigMap::from_variant(&map.to_variant());
    assert_eq!(packed.text("Font"), Some("Sans Bold 12"));
    assert_eq!(packed.boolean("Vertical Candidate List"), Some(false));
}

#[test]
fn set_overwrites_or_appends() {
    let mut map = ConfigMap::default();
    map.set("Font", ConfigValue::Text("Sans 10".to_owned()));
    map.set("Font", ConfigValue::Text("Sans 11".to_owned()));
    map.set("Theme", ConfigValue::Text("default".to_owned()));
    assert_eq!(map.0.len(), 2);
    assert_eq!(map.text("Font"), Some("Sans 11"));
}

#[test]
fn enum_options_pairs_values_with_labels() {
    // a{sv}：Enum / EnumI18n 两个下标串→值的表
    let enum_map = map_variant(&[
        ("0", "default".to_variant()),
        ("1", "default-dark".to_variant()),
    ]);
    let label_map = map_variant(&[
        ("0", "Default".to_variant()),
        ("1", "Default Dark".to_variant()),
    ]);
    let options = map_variant(&[("Enum", enum_map), ("EnumI18n", label_map)]);
    let field = glib::Variant::tuple_from_iter([
        "Theme".to_variant(),
        "String".to_variant(),
        "Theme".to_variant(),
        // 描述里的缺省值是 v：变体里再包一层
        glib::Variant::from_variant(&"default".to_variant()),
        options,
    ]);
    let fields = glib::Variant::array_from_iter_with_type(
        glib::VariantTy::new("(sssva{sv})").unwrap(),
        [field],
    );
    let section = glib::Variant::tuple_from_iter(["ClassicUIConfig".to_variant(), fields]);
    let description = glib::Variant::array_from_iter_with_type(
        glib::VariantTy::new("(sa(sssva{sv}))").unwrap(),
        [section],
    );
    assert_eq!(
        enum_options(&description, "Theme"),
        vec![
            ("default".to_owned(), "Default".to_owned()),
            ("default-dark".to_owned(), "Default Dark".to_owned()),
        ]
    );
    assert!(enum_options(&description, "Font").is_empty());
}

#[test]
fn real_fcitx_values_are_strings_and_nested_unknown_values_survive() {
    let nested = map_variant(&[("future", "keep".to_variant())]);
    let variant = map_variant(&[
        ("Vertical Candidate List", "True".to_variant()),
        ("Unknown", nested),
    ]);
    let map = ConfigMap::from_variant(&variant);
    assert_eq!(map.boolean("Vertical Candidate List"), Some(true));
    let readback = ConfigMap::from_variant(&map.to_variant());
    assert_eq!(readback, map);
    assert_eq!(
        readback.child("Unknown").unwrap().text("future"),
        Some("keep")
    );
}
