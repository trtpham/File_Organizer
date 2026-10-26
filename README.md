# Smart File Organizer 📂 (Enterprise Edition v2.0)

[![Python Version](https://img.shields.io/badge/python-3.10%2B-blue.svg)](https://www.python.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Tests Status](https://img.shields.io/badge/tests-19%2F19%20passed-success.svg)](tests/)
[![Architecture](https://img.shields.io/badge/architecture-Clean%20%2F%20Zero--Dependency-orange.svg)](docs/ARCHITECTURE.md)

**Smart File Organizer (SFO)** là ứng dụng máy tính toàn diện, chuyên nghiệp giúp dọn dẹp, sắp xếp và tối ưu hóa không gian lưu trữ một cách thông minh, an toàn tuyệt đối. 

Ứng dụng sở hữu **giao diện đồ họa Desktop (Modern Dark GUI)** trực quan kết hợp cùng **giao diện dòng lệnh (CLI)** mạnh mẽ, hoạt động độc lập mà **không cần cài thêm bất kỳ thư viện ngoài nào (Zero External Dependencies)**.

---

## 🌟 Điểm nổi bật & Tính năng chính

- 🧹 **Tổ chức tệp tin đa năng (Multi-Strategy Organization):**
  - Tự động phân loại hơn **150+ định dạng tệp** vào các nhóm danh mục rõ ràng (`Images`, `Documents`, `Spreadsheets`, `Presentations`, `Audio`, `Videos`, `Archives`, `Executables`, `Code`, `Fonts`, `3D_and_CAD`, `Torrents`, `Others`).
  - Hỗ trợ sắp xếp theo **Thời gian** (`YYYY/MM` hoặc `YYYY-MM-DD`).
  - Hỗ trợ sắp xếp theo **Kích thước** (`Tiny <1MB`, `Small`, `Medium`, `Large`, `Huge >1GB`).
  - Hỗ trợ quy tắc biểu thức chính quy tùy biến (**Custom Regex Matching**).

- 🔍 **Phát hiện trùng lặp 3 tầng siêu tốc (3-Tiered Deduplication):**
  - Khắc phục triệt để tình trạng nghẽn đĩa và đứng máy của các tool thông thường:
    - **Tầng 1 (Size Clustering):** Nhóm file theo kích thước byte chính xác (loại bỏ >90% file không trùng khỏi I/O đĩa).
    - **Tầng 2 (Fast Partial Hash):** Chỉ đọc và băm 64KB đầu tiên của các ứng viên cùng kích thước.
    - **Tầng 3 (Full SHA-256 Hash):** Băm toàn phần bảo đảm an toàn 100% dữ liệu.
  - Tự động cách ly file trùng vào thư mục `_Duplicates/` hoặc xóa an toàn, bảo vệ file gốc tốt nhất theo tiêu chí chọn lựa (cũ nhất, mới nhất, đường dẫn ngắn nhất).

- 🛡️ **An toàn tuyệt đối & Hoàn tác 100% (Transactional Rollback Engine):**
  - **Chế độ quét thử (Dry-Run):** Xem trước toàn bộ kế hoạch thay đổi dưới dạng bảng chi tiết mà không ghi một byte nào vào ổ cứng.
  - **Hoàn tác 1-chạm (Undo / Rollback):** Mọi phiên làm việc đều được ghi nhật ký dạng giao dịch (SQLite & JSON Journal). Chỉ với một cú nhấp chuột hoặc lệnh `--undo`, toàn bộ tệp tin được khôi phục về đúng vị trí ban đầu và tự động dọn sạch thư mục rỗng.
  - **Chống ghi đè vô tình (Smart Collision Resolution):** Tự động phát hiện trùng tên và đánh số thứ tự an toàn `file (1).ext`.

- ⚡ **Trình giám sát tự động thời gian thực (Background Watcher):**
  - Chạy ngầm như một daemon theo dõi thư mục (ví dụ `Downloads`).
  - Tích hợp bộ ổn định tải xuống (**Download Stabilization Debounce**): chỉ kích hoạt dọn dẹp khi file đã tải xong hoàn toàn (bỏ qua `.crdownload`, `.tmp`, `.part`).

- 🖥️ **Giao diện kép (Desktop Modern GUI & Rich CLI):**
  - **Desktop GUI:** Tkinter/ttk với Dark Theme hiện đại, bảng điều khiển thẻ thống kê, cây xem trước tương tác, trung tâm khử trùng lặp và tab lịch sử hoàn tác.
  - **Rich CLI:** Đầy đủ cờ tham số chuẩn POSIX/GNU, hiển thị màu sắc ANSI, bảng biểu trực quan và xuất báo cáo HTML Dashboard.

---

## 📁 Cấu trúc thư mục dự án

```
File_Organizer/
├── .github/
│   └── workflows/
│       └── ci.yml                 # Pipeline CI chạy kiểm thử tự động trên Windows, Ubuntu, macOS
├── config/
│   └── default_config.json        # Danh mục 150+ định dạng file và cấu hình chuẩn
├── docs/
│   ├── PRD.md                     # Tài liệu yêu cầu sản phẩm chi tiết
│   ├── ARCHITECTURE.md            # Tài liệu thiết kế kiến trúc kỹ thuật
│   ├── CLI_USAGE.md               # Sổ tay hướng dẫn dòng lệnh chi tiết
│   └── GUI_GUIDE.md               # Hướng dẫn sử dụng giao diện Desktop
├── smart_file_organizer/          # Gói mã nguồn ứng dụng (Clean Architecture)
│   ├── core/                      # Động cơ nghiệp vụ lõi (Zero UI Dependency)
│   │   ├── models.py              # FileInfo, ActionRecord, SessionLog, Enums
│   │   ├── config.py              # Quản lý cấu hình & nạp overrides
│   │   ├── scanner.py             # Quét thư mục đệ quy & lọc file ẩn/hệ thống
│   │   ├── hasher.py              # Băm 3 tầng siêu tốc (Size -> 64KB -> Full SHA256)
│   │   ├── deduplicator.py        # Phát hiện trùng lặp & lập kế hoạch xử lý
│   │   ├── organizer.py           # Thuật toán phân loại theo Ext, Date, Size, Regex
│   │   ├── executor.py            # Thực thi giao dịch an toàn & xử lý va chạm tên
│   │   ├── undo_manager.py        # Ghi nhật ký SQLite/JSON & rollback 100%
│   │   ├── watcher.py             # Giám sát nền với cơ chế Debounce
│   │   └── reporter.py            # Báo cáo Console, Markdown và HTML Dashboard
│   ├── cli/                       # Giao diện dòng lệnh
│   │   ├── parser.py              # Xử lý đối số dòng lệnh
│   │   ├── formatter.py           # Định dạng màu sắc ANSI & bảng ASCII
│   │   └── app.py                 # Điều phối các lệnh CLI
│   └── gui/                       # Giao diện đồ họa Desktop
│       ├── theme.py               # Dark Theme hiện đại cho Tkinter/ttk
│       ├── components.py          # Thẻ thống kê (StatCard), Bảng dữ liệu (Treeview)
│       └── app.py                 # Cửa sổ chính Desktop đa tab
├── tests/                         # Bộ kiểm thử tự động toàn diện (19/19 Tests Pass)
├── main.py                        # Điểm khởi chạy thống nhất (Tự mở GUI nếu chạy trực tiếp)
├── pyproject.toml                 # Cấu hình đóng gói chuẩn PEP 621
├── setup.py                       # Hỗ trợ cài đặt pip install -e .
├── LICENSE                        # Giấy phép MIT
└── README.md                      # Tài liệu dự án
```

---

## 🚀 Cài đặt & Khởi chạy

### 1. Yêu cầu hệ thống
- **Hệ điều hành:** Windows 10/11, macOS, hoặc Linux.
- **Python:** Phiên bản 3.10 trở lên.
- **Thư viện ngoài:** **0** (Hoạt động hoàn chỉnh với thư viện chuẩn có sẵn của Python).

### 2. Tải về (Clone Repository)
```bash
git clone https://github.com/trtpham/File_Organizer.git
cd File_Organizer
```

---

## 🖥️ Hướng dẫn sử dụng Giao diện Desktop (GUI)

Khởi chạy giao diện Desktop bằng lệnh:
```bash
python main.py
```
*(hoặc `python main.py --gui`)*

Giao diện đồ họa bao gồm 4 tab chuyên dụng:
1. **🧹 Organize & Clean:** Chọn thư mục nguồn/đích, chọn chiến lược dọn dẹp, bấm **Preview Actions** để xem trước dạng bảng và bấm **Execute Now**.
2. **🔍 Duplicate Center:** Quét tìm các file trùng lặp nội dung, xem dung lượng lãng phí và chuyển file thừa sang thư mục `_Duplicates/`.
3. **⏪ Undo & History:** Xem danh sách tất cả các phiên đã dọn dẹp, chọn phiên bất kỳ và bấm **Undo Selected Session** để hoàn tác lại nguyên trạng vị trí ban đầu.
4. **⚙️ Rules & Settings:** Tra cứu và tùy biến danh sách định dạng file của các nhóm danh mục.

---

## ⌨️ Hướng dẫn sử dụng Dòng lệnh (CLI)

### Các câu lệnh phổ biến:

- **Quét thử chế độ Dry-Run (Khuyên dùng khi bắt đầu):**
  ```bash
  python main.py --source "C:/Users/Raven/Downloads" --dry-run
  ```

- **Dọn dẹp và sắp xếp thật vào các thư mục danh mục:**
  ```bash
  python main.py --source "C:/Users/Raven/Downloads"
  ```

- **Sắp xếp ảnh hoặc tài liệu theo Năm/Tháng (`YYYY/MM`):**
  ```bash
  python main.py --source "D:/Photos" --by-date
  ```

- **Sắp xếp tệp tin theo nhóm kích thước (Nhỏ, Vừa, Lớn, Khổng lồ):**
  ```bash
  python main.py --source "D:/Downloads" --by-size
  ```

- **Tìm và cách ly file trùng lặp nội dung:**
  ```bash
  python main.py --source "D:/Storage" --dedupe
  ```

- **Hoàn tác phiên dọn dẹp gần nhất (Rollback 100%):**
  ```bash
  python main.py --undo
  ```

- **Xem lịch sử các phiên dọn dẹp trước đây:**
  ```bash
  python main.py --history
  ```

- **Chạy chế độ tự động ngầm theo dõi thư mục Downloads:**
  ```bash
  python main.py --source "C:/Users/Raven/Downloads" --watch
  ```

- **Xuất báo cáo thống kê ra file HTML Dashboard:**
  ```bash
  python main.py --source "C:/Users/Raven/Downloads" --report "report.html"
  ```

---

## 🧪 Chạy bộ kiểm thử tự động (Unit & Integration Tests)

Dự án đi kèm bộ test đầy đủ phủ kín tất cả các module nghiệp vụ lõi:
```bash
python -m pytest -v tests/
```

Kết quả:
```text
tests/test_cli.py::test_cli_help PASSED                                  [  5%]
tests/test_cli.py::test_cli_dry_run PASSED                               [ 10%]
tests/test_cli.py::test_cli_history_empty PASSED                         [ 15%]
tests/test_config.py::test_default_categories_loaded PASSED              [ 21%]
tests/test_config.py::test_extension_category_lookup PASSED              [ 26%]
tests/test_config.py::test_ignore_pattern_matching PASSED                [ 31%]
tests/test_config.py::test_size_bucket_determination PASSED              [ 36%]
tests/test_deduplicator.py::test_deduplicator_finds_exact_duplicates PASSED [ 42%]
tests/test_deduplicator.py::test_plan_deduplication_actions PASSED       [ 47%]
tests/test_executor.py::test_dry_run_leaves_files_untouched PASSED       [ 52%]
tests/test_executor.py::test_live_execution_moves_files PASSED           [ 57%]
tests/test_executor.py::test_collision_auto_rename PASSED                [ 63%]
tests/test_hasher.py::test_partial_and_full_hash PASSED                  [ 68%]
tests/test_hasher.py::test_identical_files_produce_identical_hashes PASSED [ 73%]
tests/test_organizer.py::test_organize_by_extension PASSED               [ 78%]
tests/test_organizer.py::test_organize_by_size PASSED                    [ 84%]
tests/test_scanner.py::test_scanner_filters_ignored_files PASSED         [ 89%]
tests/test_scanner.py::test_scanner_extracts_correct_metadata PASSED     [ 94%]
tests/test_undo.py::test_undo_restores_all_files PASSED                  [100%]

============================= 19 passed in 0.77s ==============================
```

---

## 🤝 Đóng góp & Phát triển

Mọi đóng góp, báo cáo lỗi hoặc ý tưởng tính năng mới đều được chào đón nồng nhiệt!
1. Fork repository.
2. Tạo nhánh tính năng mới (`git checkout -b feature/AmazingFeature`).
3. Commit thay đổi của bạn (`git commit -m 'feat: Add some AmazingFeature'`).
4. Push lên nhánh (`git push origin feature/AmazingFeature`).
5. Mở một Pull Request.

---

## 📄 Bản quyền (License)

Dự án được phân phối dưới giấy phép **MIT License**. Xem file [`LICENSE`](LICENSE) để biết thêm chi tiết.

---

## 👤 Tác giả

- **Tác giả:** Pham Minh Triet (Raven)
- **Email:** `trietpham1912@gmail.com`
- **GitHub:** [@trtpham](https://github.com/trtpham)
- **Repository:** [https://github.com/trtpham/File_Organizer](https://github.com/trtpham/File_Organizer)
