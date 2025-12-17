use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LanguageKind {
    Compile,
    Interpret,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommandTemplate {
    pub bin: String,
    pub args: Vec<String>,
}

impl CommandTemplate {
    pub fn validate(&self, context: &str) -> Result<(), String> {
        let bin_lower = self.bin.to_lowercase();
        let dangerous_shells = [
            "/bin/sh",
            "/bin/bash",
            "/bin/dash",
            "/bin/zsh",
            "/usr/bin/sh",
        ];

        for shell in &dangerous_shells {
            if bin_lower.contains(shell) {
                if self.args.iter().any(|arg| arg.trim() == "-c") {
                    return Err(format!(
                        "{}: using shell '{}' with '-c' flag is prohibited (shell injection risk)",
                        context, shell
                    ));
                }
            }
        }

        let bin_without_tokens = self
            .bin
            .replace("<INPUT>", "")
            .replace("<OUTPUT>", "")
            .replace("<TARGET>", "");

        let dangerous_chars = [
            '|', '&', ';', '`', '$', '(', ')', '{', '}', '<', '>', '\n', '\r',
        ];
        for ch in dangerous_chars {
            if bin_without_tokens.contains(ch) {
                return Err(format!(
                    "{}: binary path contains dangerous character '{}' (shell injection risk)",
                    context, ch
                ));
            }
        }

        for (i, arg) in self.args.iter().enumerate() {
            if arg.trim() == "-c" && i + 1 < self.args.len() {
                let next_arg = &self.args[i + 1];
                if next_arg.contains("<TARGET>") || next_arg.contains("<INPUT>") {
                    return Err(format!(
                        "{}: '-c' flag followed by user-controlled token '{}' (command injection risk)",
                        context, next_arg
                    ));
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LanguageDefinition {
    pub name: String,
    pub kind: LanguageKind,
    pub compile: Option<CommandTemplate>,
    pub run: CommandTemplate,
}

impl LanguageDefinition {
    pub fn validate(&self) -> Result<(), String> {
        self.run
            .validate(&format!("language '{}' run command", self.name))?;

        if let Some(ref compile) = self.compile {
            compile.validate(&format!("language '{}' compile command", self.name))?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LanguageConfig {
    pub languages: Vec<LanguageDefinition>,
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(serde_yaml::Error),
    MissingLanguage(String),
    InvalidCommand { language: String, reason: String },
    MissingConfig(String),
}

impl LanguageConfig {
    pub fn from_file(path: impl AsRef<Path>) -> Result<LanguageConfig, ConfigError> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(ConfigError::MissingConfig(path.display().to_string()));
        }

        let data = fs::read_to_string(path).map_err(ConfigError::Io)?;
        let config: LanguageConfig = serde_yaml::from_str(&data).map_err(ConfigError::Parse)?;

        for lang in &config.languages {
            lang.validate()
                .map_err(|reason| ConfigError::InvalidCommand {
                    language: lang.name.clone(),
                    reason,
                })?;
        }

        Ok(config)
    }

    pub fn get_language(&self, name: &str) -> Result<LanguageDefinition, ConfigError> {
        let map: HashMap<_, _> = self
            .languages
            .iter()
            .map(|lang| (lang.name.clone(), lang.clone()))
            .collect();

        map.get(name)
            .cloned()
            .ok_or_else(|| ConfigError::MissingLanguage(name.to_string()))
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(err) => write!(f, "failed to read config: {}", err),
            ConfigError::Parse(err) => write!(f, "failed to parse config: {}", err),
            ConfigError::MissingLanguage(lang) => write!(f, "language not found: {}", lang),
            ConfigError::InvalidCommand { language, reason } => {
                write!(f, "invalid command for language '{}': {}", language, reason)
            }
            ConfigError::MissingConfig(path) => write!(f, "config not found: {}", path),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_command_template() {
        let template = CommandTemplate {
            bin: "/usr/bin/gcc".to_string(),
            args: vec![
                "gcc".to_string(),
                "-o".to_string(),
                "<OUTPUT>".to_string(),
                "<INPUT>".to_string(),
            ],
        };
        assert!(template.validate("test").is_ok());
    }

    #[test]
    fn test_valid_template_with_target_token() {
        let template = CommandTemplate {
            bin: "<TARGET>".to_string(),
            args: vec!["<TARGET>".to_string()],
        };
        assert!(template.validate("test").is_ok());
    }

    #[test]
    fn test_shell_with_c_flag_rejected() {
        let template = CommandTemplate {
            bin: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), "echo hello".to_string()],
        };
        let result = template.validate("test");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("shell injection risk"));
    }

    #[test]
    fn test_bash_with_c_flag_rejected() {
        let template = CommandTemplate {
            bin: "/bin/bash".to_string(),
            args: vec!["-c".to_string(), "<TARGET>".to_string()],
        };
        let result = template.validate("test");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("shell injection risk"));
    }

    #[test]
    fn test_pipe_in_bin_rejected() {
        let template = CommandTemplate {
            bin: "/usr/bin/cat | /bin/sh".to_string(),
            args: vec![],
        };
        let result = template.validate("test");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("dangerous character '|'"));
    }

    #[test]
    fn test_semicolon_in_bin_rejected() {
        let template = CommandTemplate {
            bin: "/usr/bin/gcc; rm -rf /".to_string(),
            args: vec![],
        };
        let result = template.validate("test");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("dangerous character ';'"));
    }

    #[test]
    fn test_command_substitution_rejected() {
        let template = CommandTemplate {
            bin: "/usr/bin/$(whoami)".to_string(),
            args: vec![],
        };
        let result = template.validate("test");
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_python_interpreter() {
        let template = CommandTemplate {
            bin: "/usr/bin/python3".to_string(),
            args: vec!["python3".to_string(), "<TARGET>".to_string()],
        };
        assert!(template.validate("test").is_ok());
    }
}
