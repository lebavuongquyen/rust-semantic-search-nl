# Conversation Context & Problem Statement: The Genesis of SemanticSearchAgent

> Tài liệu này ghi lại toàn bộ bối cảnh cuộc trao đổi, phân tích lỗi thực tế từ chứng từ gốc, nguyên nhân khiến các công cụ AI search thông thường thất bại, và yêu cầu phát triển kiến trúc Semantic Search Agent chuyên sâu.

---

## 1. Bối Cảnh Thực Tế & Ca Kiểm Thử Điển Hình (The Catalyst Case)

### Yêu cầu tìm kiếm của người dùng:
> **Truy vấn:** `"tìm biên lại thu tiền tháng 8"`  
> **Hiện trạng:** Tính năng AI Smart Search của công cụ hiện tại **hoàn toàn không tìm ra kết quả (0 tệp tin)**.

### Chứng từ người dùng cần tìm trong thực tế:
* **Tên tệp tin:** Chứa ảnh scan / tệp PDF chứng từ thu nộp ngân sách.
* **Tiêu đề in trên chứng từ:**  
  $$\text{"GIẤY NỘP TIỀN THU PHÍ, LỆ PHÍ VÀ THU PHẠT VI PHẠM HÀNH CHÍNH"}$$
* **Thông tin định danh trên chứng từ:**
  * **Cơ quan thu / Ngân hàng:** BIDV (Ngân hàng TMCP Đầu tư và Phát triển Việt Nam) phối hợp UBND Xã Bà Điểm - TP.HCM.
  * **Mẫu số / Ký hiệu:** Mẫu `03`, Ký hiệu `C1-03/NS`, Số `2468454442`.
  * **Mã hồ sơ / Mã giao dịch:** `H29.243-260824-150060`, `G22.99.2-260824-108169`.
  * **Khoản nộp:** `L004` – `Lệ phí hộ tịch` (*Cấp bản sao Trích lục hộ tịch, bản sao Giấy khai sinh*).
  * **Người nộp:** `Lê Bá Vương Quyến` – Mã số thuế/CCCD: `079088015776`.
  * **Số tiền:** `80,000 VND` (Tám mươi ngàn đồng).
  * **Ngày ghi trên văn bản:** `Ngày 24 Tháng 08 Năm 2026`.
  * **Chữ ký số điện tử:** Ký bởi BIDV, ngày ký `24/08/2026`, CA: `HILO-CA SHA-256`.

---

## 2. Phân Tích 4 Nguyên Nhân Khiến Tìm Kiếm Cũ Thất Bại

1. **Khoảng cách từ vựng (Vocabulary Mismatch):**
   * Người dùng tìm từ dân dã: `"biên lai thu tiền"`.
   * Chứng từ dùng thuật ngữ hành chính/kho bạc: `"Giấy nộp tiền thu phí, lệ phí"`. Trong toàn bộ văn bản không hề có chữ `"biên lai"`.
2. **Bẫy lọc thời gian (Temporal Hard-Filter Trap):**
   * Hệ thống cũ thấy chữ `"tháng 8"` liền kích hoạt bộ lọc cứng: So sánh thuộc tính `file.lastModified` của hệ điều hành.
   * Nếu người dùng tải file về máy vào tháng 10, file bị ngắt bỏ ngay lập tức (`continue;`), hoàn toàn không kịp đọc nội dung.
   * Hơn nữa, trên văn bản ghi là `"Tháng 08"` (có số 0) hoặc nằm trong mã `260824`, bộ lọc cũ chỉ tìm chuỗi `"tháng 8"`.
3. **Lỗi gõ phím / Dấu tiếng Việt (Typo):**
   * Người dùng gõ nhầm `"biên lại"` (dấu nặng). Bộ lọc không có khả năng tự sửa lỗi (Fuzzy / SymSpell) nên bị kẹt.
4. **Hạn chế Đa phương thức & Kiến trúc chặn cửa (Architectural Blocker):**
   * Chạy bộ lọc từ khóa cơ học trước $\rightarrow$ lọc trượt $\rightarrow$ AI Semantic Re-ranker (mô hình Transformer) không bao giờ nhìn thấy tệp.
   * Thiếu bộ OCR chọn lọc (Selective OCR) khi tài liệu là ảnh hoặc PDF scan.

---

## 3. Các Yêu Cầu Mở Rộng Sâu Rộng Từ Người Dùng

* **Không chắp vá tạm bợ:** Cần một dự án/kiến trúc riêng biệt bằng **Rust & WebAssembly (WASM)** để phát triển lâu dài, độc lập, có thể nhúng vào mọi nền tảng.
* **Quy mô hàng ngàn đến chục ngàn files:** Không thể scan tuần tự hay OCR mù quáng làm đơ CPU/tràn RAM. Phải dùng thuật toán Index thông minh (Trigram Index, Inverted Index BM25, HNSW Vector Index, Incremental Indexing).
* **Đa ngôn ngữ & Xuyên ngôn ngữ (Cross-Lingual):** 
  * Gõ tiếng Việt tìm được file tiếng Anh/Trung/Nhật và ngược lại nhờ Không gian Vector chung (Shared Vector Space).
* **Năng lực Coding Agent:**
  * Tự nhận diện ngôn ngữ trong 0ms.
  * Tự hiểu ý định lập trình: Nói *"lỗi không đăng nhập được"* phải tự ánh xạ ra:
    * File: `dang-nhap.jsx`, `login.tsx`, `auth-service.ts`.
    * Route: `/api/auth/login`, `/dang-nhap`.
    * Trạng thái lỗi: `401 Unauthorized`, `Invalid credentials`.

---

## 4. Vị Trí Dự Án Mới
* **Đường dẫn Repository:** `E:\10_Learning\Rust\SemanticSearchAgent`
* **Công nghệ cốt lõi:** Rust 1.99, WebAssembly (`wasm32-unknown-unknown`), `wasm-bindgen`, SIMD Vector Math, Roaring Bitmaps.
