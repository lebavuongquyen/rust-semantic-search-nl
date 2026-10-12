use std::collections::HashMap;

/// Đại diện cho một Khái niệm Nghiệp vụ Trung gian (Domain Concept)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainConcept {
    FinPaymentReceipt, // Chứng từ nộp/thu tiền, biên lai, lệ phí
    AuthLogin,          // Đăng nhập, xác thực, credentials
    LegalContract,      // Hợp đồng, phụ lục, thỏa thuận
    HrSalary,           // Bảng lương, phiếu lương, thu nhập
    BugError,           // Lỗi, crash, thất bại, unauthorized
}

/// Mạng lưới từ điển khái niệm đa ngôn ngữ (Concept-Centric Multilingual Thesaurus)
pub struct ConceptRegistry {
    concept_map: HashMap<String, (DomainConcept, Vec<&'static str>)>,
}

impl Default for ConceptRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ConceptRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            concept_map: HashMap::new(),
        };
        registry.register_concepts();
        registry
    }

    fn register_concepts(&mut self) {
        // 1. Khái niệm Chứng từ nộp tiền / Biên lai / Lệ phí
        let receipt_synonyms = vec![
            "biên lai",
            "giấy nộp tiền",
            "giay nop tien",
            "phiếu thu",
            "phieu thu",
            "chứng từ",
            "chung tu",
            "lệ phí",
            "le phi",
            "thu phí",
            "thu phi",
            "nộp ngân sách",
            "nop ngan sach",
            "kho bạc",
            "kho bac",
            "receipt",
            "payment slip",
            "payment voucher",
            "tax invoice",
            "payment order",
            "收据",
            "缴纳凭单",
            "領収書",
            "納付書",
        ];
        self.register(DomainConcept::FinPaymentReceipt, receipt_synonyms);

        // 2. Khái niệm Đăng nhập / Xác thực (Authentication)
        let auth_synonyms = vec![
            "đăng nhập",
            "dang nhap",
            "dang-nhap",
            "login",
            "signin",
            "sign-in",
            "log-in",
            "auth",
            "authenticate",
            "credentials",
            "session",
            "tài khoản",
            "mat khau",
            "password",
            "登录",
            "ログイン",
        ];
        self.register(DomainConcept::AuthLogin, auth_synonyms);

        // 3. Khái niệm Hợp đồng / Pháp lý (Contract)
        let contract_synonyms = vec![
            "hợp đồng",
            "hop dong",
            "thỏa thuận",
            "thoa thuan",
            "phụ lục",
            "phu luc",
            "biên bản",
            "bien ban",
            "contract",
            "agreement",
            "annex",
            "amendment",
            "mou",
            "合同",
            "契約書",
        ];
        self.register(DomainConcept::LegalContract, contract_synonyms);

        // 4. Khái niệm Bảng lương / Thu nhập (Payroll)
        let salary_synonyms = vec![
            "bảng lương",
            "bang luong",
            "phiếu lương",
            "phieu luong",
            "thu nhập",
            "thu nhap",
            "tiền lương",
            "payroll",
            "payslip",
            "salary",
            "compensation",
            "wages",
            "工资条",
            "給与明細",
        ];
        self.register(DomainConcept::HrSalary, salary_synonyms);

        // 5. Khái niệm Lỗi / Sự cố (Bug & Errors)
        let error_synonyms = vec![
            "lỗi",
            "loi",
            "hỏng",
            "hong",
            "thất bại",
            "that bai",
            "không được",
            "khong duoc",
            "error",
            "bug",
            "fail",
            "failure",
            "crash",
            "exception",
            "unauthorized",
            "401",
            "403",
            "500",
            "invalid",
        ];
        self.register(DomainConcept::BugError, error_synonyms);
    }

    fn register(&mut self, concept: DomainConcept, synonyms: Vec<&'static str>) {
        for &syn in &synonyms {
            let key = syn.to_lowercase();
            self.concept_map
                .insert(key, (concept.clone(), synonyms.clone()));
        }
    }

    /// Tìm kiếm và mở rộng khái niệm dựa trên từ/cụm từ
    pub fn expand_term(&self, term: &str) -> Vec<String> {
        let clean = term.trim().to_lowercase();
        let mut results = Vec::new();
        let mut matched_concepts = std::collections::HashSet::new();

        for (key, (concept, synonyms)) in &self.concept_map {
            if clean.contains(key) {
                if matched_concepts.insert(format!("{:?}", concept)) {
                    for &s in synonyms {
                        results.push(s.to_string());
                    }
                }
            }
        }

        results.sort();
        results.dedup();
        results
    }
}
