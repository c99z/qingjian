//! 导入坏文件、同名替换与移除边界。
use super::{import, remove};
#[test]
fn invalid_import_does_not_destroy_existing_dictionary() {
    let dir = tempfile::tempdir().unwrap();
    let user = dir.path().join("user");
    let source = dir.path().join("law.tsv");
    std::fs::write(&source, "合同\the tong\t100\n").unwrap();
    import(&source, &user, "law", false).unwrap();
    let target = user.join("dicts/law.qj");
    let original = std::fs::read(&target).unwrap();
    assert!(import(&source, &user, "law", false).is_err());
    std::fs::write(&source, "invalid").unwrap();
    assert!(import(&source, &user, "law", true).is_err());
    assert_eq!(std::fs::read(&target).unwrap(), original);
    assert!(remove(&user, "../law").is_err());
    remove(&user, "law").unwrap();
    assert!(!target.exists());
}
