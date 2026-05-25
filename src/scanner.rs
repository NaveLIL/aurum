use regex::Regex;
use crate::types::{ScanResult, Vulnerability, RiskLevel};

pub struct Scanner {
    patterns: Vec<(Regex, RiskLevel, String)>,
}

impl Scanner {
    pub fn new(config: &crate::config::Config) -> Self {
        let mut patterns = Vec::new();

        for pat_str in &config.risky_patterns {
            if let Ok(re) = Regex::new(pat_str) {
                let (risk, desc) = match pat_str.as_str() {
                    r"rm\s+-rf\s+.*" | r"rm\s+-rf\s+[/~]*" => (RiskLevel::Dangerous, "Dangerous files deletion (rm -rf)".into()),
                    r"curl\s+.*\|\s*sh" => (RiskLevel::Dangerous, "Piping curl to sh".into()),
                    r"wget\s+.*\|\s*sh" => (RiskLevel::Dangerous, "Piping wget to sh".into()),
                    r"eval\s+" => (RiskLevel::Suspicious, "Use of eval".into()),
                    r"base64\s+-d" => (RiskLevel::Suspicious, "Base64 decoding (obfuscation?)".into()),
                    r"sudo\s+" => (RiskLevel::Warning, "Use of sudo in PKGBUILD".into()),
                    _ => (RiskLevel::Suspicious, format!("Configured risky pattern: {}", pat_str)),
                };
                patterns.push((re, risk, desc));
            }
        }

        Self { patterns }
    }

    pub fn scan(&self, package_name: &str, content: &str) -> ScanResult {
        let mut vulnerabilities = Vec::new();
        let mut max_score = 0;

        for (line_idx, line) in content.lines().enumerate() {
            for (re, risk, desc) in &self.patterns {
                if re.is_match(line) {
                    vulnerabilities.push(Vulnerability {
                        check_name: desc.clone(),
                        description: format!("Found pattern matching '{}'", re.as_str()),
                        risk_level: risk.clone(),
                        line_number: Some(line_idx + 1),
                        line_content: Some(line.trim().to_string()),
                    });
                    
                    let score = risk.score();
                    if score > max_score {
                        max_score = score;
                    }
                }
            }
        }

        ScanResult {
            package_name: package_name.to_string(),
            score: max_score,
            vulnerabilities,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_scanner_with_default_config() {
        let config = Config::default();
        let scanner = Scanner::new(&config);

        // Test normal PKGBUILD
        let safe_pkgbuild = r#"
pkgname=test-pkg
pkgver=1.0.0
build() {
    echo "Building safely"
}
"#;
        let result = scanner.scan("test-pkg", safe_pkgbuild);
        assert_eq!(result.score, 0);
        assert!(result.vulnerabilities.is_empty());

        // Test dangerous PKGBUILD with rm -rf
        let dangerous_pkgbuild = r#"
pkgname=test-pkg
pkgver=1.0.0
prepare() {
    rm -rf /
}
"#;
        let result2 = scanner.scan("test-pkg", dangerous_pkgbuild);
        assert_eq!(result2.score, 100);
        assert_eq!(result2.vulnerabilities.len(), 1);
        assert_eq!(result2.vulnerabilities[0].check_name, "Dangerous files deletion (rm -rf)");

        // Test warnings like sudo
        let sudo_pkgbuild = r#"
pkgname=test-pkg
pkgver=1.0.0
prepare() {
    sudo make install
}
"#;
        let result3 = scanner.scan("test-pkg", sudo_pkgbuild);
        assert_eq!(result3.score, 30);
        assert_eq!(result3.vulnerabilities.len(), 1);
        assert_eq!(result3.vulnerabilities[0].check_name, "Use of sudo in PKGBUILD");
    }

    #[test]
    fn test_scanner_with_custom_config_patterns() {
        let mut config = Config::default();
        config.risky_patterns.push(r"malicious_string".to_string());
        
        let scanner = Scanner::new(&config);
        let custom_pkgbuild = r#"
pkgname=test-pkg
pkgver=1.0.0
# Contains malicious_string
"#;
        let result = scanner.scan("test-pkg", custom_pkgbuild);
        assert_eq!(result.score, 60); // Suspicious (score = 60)
        assert_eq!(result.vulnerabilities.len(), 1);
        assert!(result.vulnerabilities[0].check_name.contains("Configured risky pattern"));
    }
}
