use super::{Locale, TextKey};

#[test]
fn russian_is_the_default_locale() {
    assert_eq!(Locale::default(), Locale::Russian);
}

#[test]
fn both_catalogs_have_core_translations() {
    for locale in Locale::ALL {
        assert!(!locale.text(TextKey::FileMenu).is_empty());
        assert!(!locale.text(TextKey::SearchPlaceholder).is_empty());
        assert!(!locale.text(TextKey::CopyStructure).is_empty());
        assert!(!locale.text(TextKey::TypeComment).is_empty());
        assert!(!locale.text(TextKey::TypeMetadata).is_empty());
        assert!(!locale.text(TextKey::CommentHint).is_empty());
        assert!(!locale.text(TextKey::MetadataHint).is_empty());
        assert!(!locale.text(TextKey::Undo).is_empty());
        assert!(!locale.text(TextKey::Redo).is_empty());
        assert!(!locale.text(TextKey::ActionUndone).is_empty());
        assert!(!locale.text(TextKey::ActionRedone).is_empty());
    }
}

#[test]
fn container_counts_are_localized() {
    assert_eq!(Locale::Russian.object_count(2), "{2} поля");
    assert_eq!(Locale::English.object_count(2), "{2} fields");
    assert_eq!(Locale::Russian.array_count(1), "[1] элемент");
    assert_eq!(Locale::English.array_count(1), "[1] element");
}
