use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::scan::{AppCategory, AppOrigin, Application};

#[derive(Debug, Clone)]
pub struct Section {
    pub name: String,
    pub icon: String,
    pub filter: SectionFilter,
}

#[derive(Debug, Clone)]
pub struct SectionFilter {
    origin: OriginFilter,
    category: Option<AppCategory>,
}

#[derive(Debug, Clone, Copy)]
enum OriginFilter {
    Any,
    WindowsOnly,
    NonWindows,
    ColonyOnly,
    ExternalOnly,
}

impl SectionFilter {
    pub fn matches(&self, app: &Application) -> bool {
        match self.origin {
            OriginFilter::Any => {}
            OriginFilter::WindowsOnly => {
                if app.origin != AppOrigin::Windows {
                    return false;
                }
            }
            OriginFilter::NonWindows => {
                if app.origin == AppOrigin::Windows {
                    return false;
                }
            }
            OriginFilter::ColonyOnly => {
                if app.origin != AppOrigin::Colony {
                    return false;
                }
            }
            OriginFilter::ExternalOnly => {
                if app.origin != AppOrigin::External {
                    return false;
                }
            }
        }

        if let Some(category) = &self.category {
            if &app.category != category {
                return false;
            }
        }

        true
    }
}

#[derive(Debug, Deserialize)]
struct SectionConfig {
    name: String,
    icon: String,
    origin: Option<String>,
    category: Option<String>,
}

impl SectionConfig {
    fn into_section(self) -> Section {
        Section {
            name: self.name,
            icon: self.icon,
            filter: SectionFilter {
                origin: parse_origin(self.origin.as_deref()),
                category: parse_category(self.category.as_deref()),
            },
        }
    }
}

pub fn load_sections() -> Vec<Section> {
    let path = Path::new("config/categories.json");
    match fs::read_to_string(path) {
        Ok(contents) => match serde_json::from_str::<Vec<SectionConfig>>(&contents) {
            Ok(configs) => {
                let sections: Vec<Section> = configs.into_iter().map(SectionConfig::into_section).collect();
                if sections.is_empty() {
                    eprintln!("[sections] Config loaded but no sections found, using defaults.");
                    default_sections()
                } else {
                    sections
                }
            }
            Err(error) => {
                eprintln!("[sections] Failed to parse {:?}: {error}", path);
                default_sections()
            }
        },
        Err(error) => {
            eprintln!("[sections] Failed to read {:?}: {error}", path);
            default_sections()
        }
    }
}

fn parse_origin(origin: Option<&str>) -> OriginFilter {
    match origin.map(|value| value.trim().to_lowercase()) {
        Some(value) if value == "windows" || value == "windows_only" => OriginFilter::WindowsOnly,
        Some(value) if value == "non_windows" || value == "nonwindows" => OriginFilter::NonWindows,
        Some(value) if value == "colony" => OriginFilter::ColonyOnly,
        Some(value) if value == "external" => OriginFilter::ExternalOnly,
        Some(value) if value == "any" || value == "all" => OriginFilter::Any,
        Some(value) => {
            eprintln!("[sections] Unknown origin filter '{value}', defaulting to 'any'.");
            OriginFilter::Any
        }
        None => OriginFilter::Any,
    }
}

fn parse_category(category: Option<&str>) -> Option<AppCategory> {
    match category.map(|value| value.trim().to_lowercase()) {
        None => None,
        Some(value) if value == "all" || value == "any" => None,
        Some(value) => match value.as_str() {
            "development" => Some(AppCategory::Development),
            "graphics" => Some(AppCategory::Graphics),
            "network" => Some(AppCategory::Network),
            "office" => Some(AppCategory::Office),
            "multimedia" => Some(AppCategory::Multimedia),
            "system" => Some(AppCategory::System),
            "utility" | "utilities" => Some(AppCategory::Utility),
            "game" | "games" => Some(AppCategory::Game),
            "other" => Some(AppCategory::Other),
            _ => {
                eprintln!("[sections] Unknown category '{value}', ignoring.");
                None
            }
        },
    }
}

fn default_sections() -> Vec<Section> {
    vec![
        Section {
            name: "All".to_string(),
            icon: "\u{f00a}".to_string(),
            filter: SectionFilter {
                origin: OriginFilter::NonWindows,
                category: None,
            },
        },
        Section {
            name: "Windows".to_string(),
            icon: "\u{f17a}".to_string(),
            filter: SectionFilter {
                origin: OriginFilter::WindowsOnly,
                category: None,
            },
        },
        Section {
            name: "Development".to_string(),
            icon: "\u{f121}".to_string(),
            filter: SectionFilter {
                origin: OriginFilter::NonWindows,
                category: Some(AppCategory::Development),
            },
        },
        Section {
            name: "Graphics".to_string(),
            icon: "\u{f1fc}".to_string(),
            filter: SectionFilter {
                origin: OriginFilter::NonWindows,
                category: Some(AppCategory::Graphics),
            },
        },
        Section {
            name: "Network".to_string(),
            icon: "\u{f0ac}".to_string(),
            filter: SectionFilter {
                origin: OriginFilter::NonWindows,
                category: Some(AppCategory::Network),
            },
        },
        Section {
            name: "Office".to_string(),
            icon: "\u{f0f6}".to_string(),
            filter: SectionFilter {
                origin: OriginFilter::NonWindows,
                category: Some(AppCategory::Office),
            },
        },
        Section {
            name: "Multimedia".to_string(),
            icon: "\u{f008}".to_string(),
            filter: SectionFilter {
                origin: OriginFilter::NonWindows,
                category: Some(AppCategory::Multimedia),
            },
        },
        Section {
            name: "System".to_string(),
            icon: "\u{f085}".to_string(),
            filter: SectionFilter {
                origin: OriginFilter::NonWindows,
                category: Some(AppCategory::System),
            },
        },
        Section {
            name: "Utilities".to_string(),
            icon: "\u{f0ad}".to_string(),
            filter: SectionFilter {
                origin: OriginFilter::NonWindows,
                category: Some(AppCategory::Utility),
            },
        },
        Section {
            name: "Games".to_string(),
            icon: "\u{f11b}".to_string(),
            filter: SectionFilter {
                origin: OriginFilter::NonWindows,
                category: Some(AppCategory::Game),
            },
        },
        Section {
            name: "Other".to_string(),
            icon: "\u{f128}".to_string(),
            filter: SectionFilter {
                origin: OriginFilter::NonWindows,
                category: Some(AppCategory::Other),
            },
        },
    ]
}
