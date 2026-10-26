"""
Unit tests for ConfigManager.
"""

from smart_file_organizer.core.config import ConfigManager
from smart_file_organizer.core.models import CollisionStrategy


def test_default_categories_loaded():
    cfg = ConfigManager()
    assert "Images" in cfg.categories
    assert "Documents" in cfg.categories
    assert "Audio" in cfg.categories
    assert ".jpg" in cfg.categories["Images"]
    assert ".pdf" in cfg.categories["Documents"]


def test_extension_category_lookup():
    cfg = ConfigManager()
    assert cfg.get_category_for_extension(".png") == "Images"
    assert cfg.get_category_for_extension("docx") == "Documents"
    assert cfg.get_category_for_extension(".py") == "Code"
    assert cfg.get_category_for_extension(".xyz123") == "Others"


def test_ignore_pattern_matching():
    cfg = ConfigManager()
    assert cfg.is_ignored(".git") is True
    assert cfg.is_ignored("desktop.ini") is True
    assert cfg.is_ignored("video.mp4.crdownload") is True
    assert cfg.is_ignored("temp.tmp") is True
    assert cfg.is_ignored("document.pdf") is False


def test_size_bucket_determination():
    cfg = ConfigManager()
    assert cfg.get_size_folder(500) == "Tiny (< 1MB)"
    assert cfg.get_size_folder(5 * 1024 * 1024) == "Small (1MB - 10MB)"
    assert cfg.get_size_folder(50 * 1024 * 1024) == "Medium (10MB - 100MB)"
    assert cfg.get_size_folder(500 * 1024 * 1024) == "Large (100MB - 1GB)"
    assert cfg.get_size_folder(2 * 1024 * 1024 * 1024) == "Huge (> 1GB)"
