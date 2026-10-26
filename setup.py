from setuptools import setup, find_packages

setup(
    name="smart-file-organizer",
    version="2.0.0",
    packages=find_packages(),
    include_package_data=True,
    package_data={
        "": ["*.json"],
    },
    entry_points={
        "console_scripts": [
            "sfo=smart_file_organizer.cli.app:run_cli",
            "smart-file-organizer=smart_file_organizer.cli.app:run_cli",
        ],
    },
)
