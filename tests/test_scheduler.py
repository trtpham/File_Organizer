from smart_file_organizer.core.scheduler import WindowsTaskScheduler


def test_windows_task_scheduler_query():
    is_supp = WindowsTaskScheduler.is_supported()
    assert isinstance(is_supp, bool)

    # Listing SFO tasks should return a list without raising error
    tasks = WindowsTaskScheduler.list_sfo_tasks()
    assert isinstance(tasks, list)
