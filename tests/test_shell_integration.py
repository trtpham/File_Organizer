"""
Unit tests for WindowsShellIntegration.
"""

from smart_file_organizer.core.shell_integration import WindowsShellIntegration


def test_is_supported_returns_bool():
    supported = WindowsShellIntegration.is_supported()
    assert isinstance(supported, bool)


def test_is_installed_query():
    # Should safely return boolean without raising unhandled exceptions
    installed = WindowsShellIntegration.is_installed()
    assert isinstance(installed, bool)
