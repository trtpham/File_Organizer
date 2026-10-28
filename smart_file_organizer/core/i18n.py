"""
Multi-Language Internationalization (i18n) Engine.
Provides complete bilingual support: English (EN) and Vietnamese (VI).
Zero external dependencies.
"""

from typing import Dict, Any, Optional

TRANSLATIONS: Dict[str, Dict[str, str]] = {
    "en": {
        # App Header & Metadata
        "app_title": "Smart File Organizer v3.0 - Enterprise Edition",
        "app_subtitle": "Clean Architecture • Visual pHash • Treemap • Cloud Sync • LAN Drop • Guardrails • 100% Rollback",

        # Tabs
        "tab_organize": "🧹 Organize",
        "tab_dedupe": "🔍 Duplicate Center",
        "tab_renamer": "🏷️ Smart Renamer",
        "tab_storage": "📊 Storage Treemap",
        "tab_sync": "💾 Sync, Cloud & LAN",
        "tab_security": "🛡️ Security & Junk",
        "tab_doctor": "🩺 Doctor & Health",
        "tab_history": "⏪ Undo & History",
        "tab_settings": "⚙️ Settings & Shell",

        # Common Controls & Labels
        "source_folder": "Source Folder:",
        "dest_folder": "Destination Folder:",
        "strategy": "Strategy:",
        "dry_run": "Dry-Run Mode (Safe Preview)",
        "smart_receipts": "Smart Financial Sort (Invoices -> Finance_2026)",
        "preview_actions": "🔍 Preview Actions",
        "execute_now": "⚡ Execute Now",
        "undo_session": "⏪ Undo Selected Session",
        "refresh": "🔄 Refresh",
        "status_ready": "Ready. Select an operation or folder to begin.",

        # Doctor Tab
        "doctor_title": "SFO Doctor System Diagnostics",
        "doctor_subtitle": "Comprehensive self-diagnostic check for disk health, permissions, registry, and system integrity.",
        "run_diagnostics": "🩺 Run System Diagnostics",
        "overall_health": "Overall System Status:",

        # Guardrails & Safety
        "guardrail_alert": "Enterprise Safety Violation",
        "protected_path_warning": "Operation blocked: Target directory contains protected operating system files.",
        "blast_radius_warning": "Blast Radius Exceeded: More than {count} files or {bytes} would be affected.",

        # LAN Drop
        "lan_drop_title": "Wireless Cross-Device LAN Drop",
        "lan_server_online": "Status: ONLINE - Open in phone's browser:",
        "lan_server_offline": "Status: Server is OFFLINE",
        "start_lan": "🚀 Start LAN Drop Server",
        "stop_lan": "🛑 Stop LAN Server",
        "open_browser": "🌐 Open in Browser",

        # Settings
        "language_select": "Application Language / Ngôn ngữ:",
        "autostart_label": "Start Smart File Organizer automatically on Windows boot",
        "context_menu_label": "Windows Explorer Context Menu Integration",
        "register_menu": "✨ Register Context Menu",
        "remove_menu": "❌ Remove Context Menu"
    },
    "vi": {
        # App Header & Metadata
        "app_title": "Smart File Organizer v3.0 - Phiên bản Doanh nghiệp",
        "app_subtitle": "Kiến trúc Sạch • Nhận diện Ảnh pHash • Treemap • Đồng bộ Cloud • Bắn file LAN • An toàn Tuyệt đối",

        # Tabs
        "tab_organize": "🧹 Sắp xếp & Dọn dẹp",
        "tab_dedupe": "🔍 Tìm File Trùng lặp",
        "tab_renamer": "🏷️ Đổi tên Hàng loạt",
        "tab_storage": "📊 Bản đồ Dung lượng",
        "tab_sync": "💾 Đồng bộ, Cloud & LAN",
        "tab_security": "🛡️ Bảo mật & Rác Dev",
        "tab_doctor": "🩺 Khám & Chẩn đoán",
        "tab_history": "⏪ Lịch sử & Khôi phục",
        "tab_settings": "⚙️ Cài đặt & Shell",

        # Common Controls & Labels
        "source_folder": "Thư mục Nguồn:   ",
        "dest_folder": "Thư mục Đích:    ",
        "strategy": "Chiến lược:",
        "dry_run": "Chế độ chạy thử (Xem trước an toàn)",
        "smart_receipts": "Phân loại Hóa đơn (Invoices -> Finance_2026)",
        "preview_actions": "🔍 Xem trước Thao tác",
        "execute_now": "⚡ Thực hiện Ngay",
        "undo_session": "⏪ Hoàn tác Phiên đã chọn",
        "refresh": "🔄 Làm mới",
        "status_ready": "Sẵn sàng. Hãy chọn thư mục hoặc tính năng để bắt đầu.",

        # Doctor Tab
        "doctor_title": "Chẩn đoán Hệ thống SFO Doctor",
        "doctor_subtitle": "Tự động kiểm tra toàn diện tốc độ ổ đĩa, phân quyền, registry và độ an toàn hệ thống.",
        "run_diagnostics": "🩺 Chạy Chẩn đoán Hệ thống",
        "overall_health": "Tình trạng Hệ thống:",

        # Guardrails & Safety
        "guardrail_alert": "Cảnh báo An toàn Doanh nghiệp",
        "protected_path_warning": "Thao tác bị chặn: Thư mục mục tiêu chứa file hệ điều hành được bảo vệ.",
        "blast_radius_warning": "Vượt ngưỡng an toàn: Thao tác ảnh hưởng hơn {count} file hoặc {bytes}.",

        # LAN Drop
        "lan_drop_title": "Bắn File Không Dây Qua Mạng LAN",
        "lan_server_online": "Trạng thái: ĐANG CHẠY - Mở link trên điện thoại:",
        "lan_server_offline": "Trạng thái: Máy chủ đang TẮT",
        "start_lan": "🚀 Bật Chia sẻ LAN",
        "stop_lan": "🛑 Tắt Chia sẻ LAN",
        "open_browser": "🌐 Mở trong Trình duyệt",

        # Settings
        "language_select": "Ngôn ngữ Giao diện / Language:",
        "autostart_label": "Tự động khởi động cùng Windows",
        "context_menu_label": "Tích hợp Menu Chuột Phải Windows Explorer",
        "register_menu": "✨ Kích hoạt Menu Chuột Phải",
        "remove_menu": "❌ Gỡ bỏ Menu Chuột Phải"
    }
}


class I18n:
    """Manages active language and string translations."""

    _current_lang: str = "en"

    @classmethod
    def set_language(cls, lang_code: str) -> None:
        """Sets active language ('en' or 'vi')."""
        if lang_code.lower() in TRANSLATIONS:
            cls._current_lang = lang_code.lower()
        else:
            cls._current_lang = "en"

    @classmethod
    def get_language(cls) -> str:
        return cls._current_lang

    @classmethod
    def t(cls, key: str, **kwargs: Any) -> str:
        """Looks up translation string with optional formatting parameters."""
        lang_dict = TRANSLATIONS.get(cls._current_lang, TRANSLATIONS["en"])
        template = lang_dict.get(key, TRANSLATIONS["en"].get(key, key))
        if kwargs:
            try:
                return template.format(**kwargs)
            except Exception:
                return template
        return template


def _t(key: str, **kwargs: Any) -> str:
    """Shorthand translation helper function."""
    return I18n.t(key, **kwargs)
