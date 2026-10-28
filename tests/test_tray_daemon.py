from smart_file_organizer.core.tray_daemon import WindowsStartupManager, DesktopNotifier


def test_windows_startup_manager_query():
    is_supported = WindowsStartupManager.is_supported()
    assert isinstance(is_supported, bool)

    # Querying status should not throw an exception
    enabled = WindowsStartupManager.is_autostart_enabled()
    assert isinstance(enabled, bool)


def test_desktop_notifier_runs():
    # Calling notify should safely return a boolean without unhandled exception
    res = DesktopNotifier.notify("Test SFO", "Unit Test Notification")
    assert isinstance(res, bool)
