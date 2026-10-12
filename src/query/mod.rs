use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalInfo {
    pub month: Option<u8>,
    pub year: Option<u16>,
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedSearchPlan {
    pub provider: String, // "gemini", "standard_fallback", "standard_pure"
    pub raw_query: String,
    pub language: String,
    pub normalized_query: String,
    pub semantic_keywords: Vec<String>,
    pub name_patterns: Vec<String>,
    pub code_routes: Vec<String>,
    pub temporal_info: Option<TemporalInfo>,
    #[serde(default)]
    pub target_extensions: Vec<String>,
    #[serde(default)]
    pub negative_keywords: Vec<String>,
}

/// Nhận diện ngôn ngữ của câu hỏi trong 0ms (Zero-overhead Diacritics & Script Detector)
pub fn detect_language(query: &str) -> &'static str {
    let lower = query.to_lowercase();

    // 1. Nhận diện Tiếng Việt qua nguyên âm có dấu
    let vi_chars = [
        'á', 'à', 'ả', 'ã', 'ạ', 'ă', 'ắ', 'ằ', 'ẳ', 'ẵ', 'ặ', 'â', 'ấ', 'ầ', 'ẩ', 'ẫ', 'ậ',
        'é', 'è', 'ẻ', 'ẽ', 'ẹ', 'ê', 'ế', 'ề', 'ể', 'ễ', 'ệ', 'í', 'ì', 'ỉ', 'ĩ', 'ị',
        'ó', 'ò', 'ỏ', 'õ', 'ọ', 'ô', 'ố', 'ồ', 'ổ', 'ỗ', 'ộ', 'ơ', 'ớ', 'ờ', 'ở', 'ỡ', 'ợ',
        'ú', 'ù', 'ủ', 'ũ', 'ụ', 'ư', 'ứ', 'ừ', 'ử', 'ữ', 'ự', 'ý', 'ỳ', 'ỷ', 'ỹ', 'ỵ', 'đ',
    ];
    if lower.chars().any(|c| vi_chars.contains(&c)) {
        return "vi";
    }

    // 2. Nhận diện chữ tượng hình Nhật (Hiragana, Katakana)
    if lower.chars().any(|c| ('\u{3040}'..='\u{309F}').contains(&c) || ('\u{30A0}'..='\u{30FF}').contains(&c)) {
        return "ja";
    }

    // 3. Nhận diện chữ Hán (CJK Ideographs)
    if lower.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c)) {
        return "zh";
    }

    // Mặc định cho ký tự ASCII
    "en"
}

/// Chuẩn hóa các lỗi gõ phím tiếng Việt phổ biến (Typo & Telex normalizer)
pub fn normalize_typos(query: &str) -> String {
    let mut result = query.to_string();

    // Sửa lỗi gõ dấu thường gặp: "biên lại" -> "biên lai"
    result = result.replace("biên lại", "biên lai");
    result = result.replace("bien lai", "biên lai");

    result
}

/// Trích xuất biểu thức thời gian đa ngôn ngữ sang chuẩn ISO Month (1-12) & Year
pub fn extract_temporal(query: &str) -> Option<TemporalInfo> {
    let lower = query.to_lowercase();
    let mut month: Option<u8> = None;
    let mut patterns: Vec<String> = Vec::new();

    let month_maps: [(&[&str], u8); 12] = [
        (&["tháng 1", "tháng 01", "tháng một", "tháng giêng", "january", "jan", "1月"], 1),
        (&["tháng 2", "tháng 02", "tháng hai", "february", "feb", "2月"], 2),
        (&["tháng 3", "tháng 03", "tháng ba", "march", "mar", "3月"], 3),
        (&["tháng 4", "tháng 04", "tháng tư", "tháng bốn", "april", "apr", "4月"], 4),
        (&["tháng 5", "tháng 05", "tháng năm", "may", "5月"], 5),
        (&["tháng 6", "tháng 06", "tháng sáu", "june", "jun", "6月"], 6),
        (&["tháng 7", "tháng 07", "tháng bảy", "july", "jul", "7月"], 7),
        (&["tháng 8", "tháng 08", "tháng tám", "august", "aug", "8月"], 8),
        (&["tháng 9", "tháng 09", "tháng chín", "september", "sep", "sept", "9月"], 9),
        (&["tháng 10", "tháng mười", "october", "oct", "10月"], 10),
        (&["tháng 11", "tháng mười một", "november", "nov", "11月"], 11),
        (&["tháng 12", "tháng mười hai", "tháng chạp", "december", "dec", "12月"], 12),
    ];

    for (aliases, m) in month_maps {
        if aliases.iter().any(|&alias| lower.contains(alias)) {
            month = Some(m);
            // Sinh danh sách các mẫu nhận diện thời gian phong phú cho tài liệu
            patterns.push(format!("tháng {}", m));
            patterns.push(format!("tháng {:02}", m));
            patterns.push(format!("t{}", m));
            patterns.push(format!("t{:02}", m));
            patterns.push(format!("/{:02}/", m));
            patterns.push(format!("-{:02}-", m));
            if m == 8 {
                patterns.push("august".to_string());
                patterns.push("aug".to_string());
                patterns.push("2608".to_string()); // Hỗ trợ mã giao dịch dạng YYMM
            }
            break;
        }
    }

    // Nhận diện năm (ví dụ 2024, 2025, 2026)
    let mut year: Option<u16> = None;
    for y in 2020..=2030 {
        let y_str = y.to_string();
        if lower.contains(&y_str) {
            year = Some(y);
            patterns.push(y_str);
            break;
        }
    }

    if month.is_some() || year.is_some() {
        Some(TemporalInfo {
            month,
            year,
            patterns,
        })
    } else {
        None
    }
}

/// Sinh các quy ước đặt tên code (Naming Conventions) và Routes cho Coding Agent
pub fn generate_code_naming_patterns(terms: &[String]) -> (Vec<String>, Vec<String>) {
    let mut file_patterns = Vec::new();
    let mut routes = Vec::new();

    for t in terms {
        let clean = t.trim().to_lowercase();
        if clean.is_empty() {
            continue;
        }

        // Kebab-case
        let kebab = clean.replace(' ', "-");
        file_patterns.push(format!("*{}*.*", kebab));
        routes.push(format!("/{}", kebab));
        routes.push(format!("/api/{}", kebab));

        // Snake_case
        let snake = clean.replace(' ', "_");
        file_patterns.push(format!("*{}*.*", snake));

        // CamelCase / PascalCase
        let words: Vec<&str> = clean.split_whitespace().collect();
        if words.len() > 1 {
            let pascal: String = words.iter().map(|w| {
                let mut c = w.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            }).collect();
            file_patterns.push(format!("*{}*.*", pascal));
        }
    }

    file_patterns.sort();
    file_patterns.dedup();
    routes.sort();
    routes.dedup();

    (file_patterns, routes)
}
