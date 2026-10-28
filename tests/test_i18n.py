from smart_file_organizer.core.i18n import I18n, _t


def test_i18n_translation():
    # Test English default
    I18n.set_language("en")
    assert I18n.get_language() == "en"
    assert "Smart File Organizer" in _t("app_title")
    assert _t("execute_now") == "⚡ Execute Now"

    # Test Vietnamese translation
    I18n.set_language("vi")
    assert I18n.get_language() == "vi"
    assert "Doanh nghiệp" in _t("app_title")
    assert _t("execute_now") == "⚡ Thực hiện Ngay"

    # Test fallback to English on invalid language code
    I18n.set_language("fr")
    assert I18n.get_language() == "en"

    # Reset
    I18n.set_language("en")
