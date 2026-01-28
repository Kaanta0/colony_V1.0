mod scan;

use iced::widget::{
    button, column, container, row, scrollable, text, text_input, Column, Row,
};
use iced::{color, Element, Fill, Font, Length, Theme};

use scan::{AppCategory, Application};

pub fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .window_size((1000.0, 700.0))
        .run()
}

#[derive(Debug, Clone, PartialEq)]
enum Category {
    All,
    Development,
    Graphics,
    Network,
    Office,
    Multimedia,
    System,
    Utility,
    Game,
    Other,
}

impl Category {
    fn label(&self) -> &'static str {
        match self {
            Category::All => "All",
            Category::Development => "Development",
            Category::Graphics => "Graphics",
            Category::Network => "Network",
            Category::Office => "Office",
            Category::Multimedia => "Multimedia",
            Category::System => "System",
            Category::Utility => "Utilities",
            Category::Game => "Games",
            Category::Other => "Other",
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            Category::All => "🧭",
            Category::Development => "💻",
            Category::Graphics => "🎨",
            Category::Network => "🌐",
            Category::Office => "📄",
            Category::Multimedia => "🎬",
            Category::System => "⚙️",
            Category::Utility => "🧰",
            Category::Game => "🎮",
            Category::Other => "📦",
        }
    }

    fn matches(&self, app_category: &AppCategory) -> bool {
        match self {
            Category::All => true,
            Category::Development => matches!(app_category, AppCategory::Development),
            Category::Graphics => matches!(app_category, AppCategory::Graphics),
            Category::Network => matches!(app_category, AppCategory::Network),
            Category::Office => matches!(app_category, AppCategory::Office),
            Category::Multimedia => matches!(app_category, AppCategory::Multimedia),
            Category::System => matches!(app_category, AppCategory::System),
            Category::Utility => matches!(app_category, AppCategory::Utility),
            Category::Game => matches!(app_category, AppCategory::Game),
            Category::Other => matches!(app_category, AppCategory::Other),
        }
    }
}

impl Default for Category {
    fn default() -> Self {
        Category::All
    }
}

struct App {
    applications: Vec<Application>,
    search_query: String,
    selected_category: Category,
    status_message: String,
}

impl Default for App {
    fn default() -> Self {
        let applications = scan::scan_applications().unwrap_or_else(|e| {
            eprintln!("Scan error: {e}");
            Vec::new()
        });

        let status_message = format!("{} applications found", applications.len());

        Self {
            applications,
            search_query: String::new(),
            selected_category: Category::All,
            status_message,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    SearchChanged(String),
    CategorySelected(Category),
    Rescan,
    LaunchApp(String),
}

impl App {
    fn title(&self) -> String {
        String::from("Colony Launcher")
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::SearchChanged(query) => {
                self.search_query = query;
            }
            Message::CategorySelected(category) => {
                self.selected_category = category;
            }
            Message::Rescan => {
                match scan::scan_applications() {
                    Ok(apps) => {
                        self.status_message = format!("{} applications found", apps.len());
                        self.applications = apps;
                    }
                    Err(e) => {
                        self.status_message = format!("Error: {e}");
                    }
                }
            }
            Message::LaunchApp(exec) => {
                self.status_message = format!("Launching...");

                #[cfg(windows)]
                {
                    // On Windows, use cmd /C start to launch
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "start", "", &exec])
                        .spawn();
                }

                #[cfg(not(windows))]
                {
                    // On Linux/macOS, parse and execute
                    let parts: Vec<&str> = exec.split_whitespace().collect();
                    if let Some((cmd, args)) = parts.split_first() {
                        let _ = std::process::Command::new(cmd)
                            .args(args)
                            .spawn();
                    }
                }
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let sidebar = self.view_sidebar();
        let content = self.view_content();

        let main_layout = row![sidebar, content].spacing(0);

        container(main_layout)
            .width(Fill)
            .height(Fill)
            .into()
    }

    fn view_sidebar(&self) -> Element<'_, Message> {
        let title = text("Colony")
            .size(28)
            .font(Font::MONOSPACE);

        let categories = [
            Category::All,
            Category::Development,
            Category::Graphics,
            Category::Network,
            Category::Office,
            Category::Multimedia,
            Category::System,
            Category::Utility,
            Category::Game,
            Category::Other,
        ];

        let category_buttons: Vec<Element<'_, Message>> = categories
            .into_iter()
            .map(|cat| self.view_category_button(cat))
            .collect();

        let category_list = Column::with_children(category_buttons).spacing(4);
        let category_section = column![
            text("Catégories").size(12).color(color!(0x888899)),
            container(text("")).height(6),
            container(text(""))
                .height(1)
                .width(Fill)
                .style(|_theme| container::Style {
                    background: Some(color!(0x2a2a4e).into()),
                    ..Default::default()
                }),
            container(text("")).height(6),
            category_list,
        ]
        .spacing(0);

        let rescan_btn = button(text("Rescan").size(14))
            .on_press(Message::Rescan)
            .padding([8, 16])
            .width(Fill);
        let action_section = column![
            text("Actions").size(12).color(color!(0x888899)),
            container(text("")).height(6),
            container(text(""))
                .height(1)
                .width(Fill)
                .style(|_theme| container::Style {
                    background: Some(color!(0x2a2a4e).into()),
                    ..Default::default()
                }),
            container(text("")).height(10),
            rescan_btn,
        ]
        .spacing(0);

        let sidebar_content = column![
            title,
            container(text("")).height(24),
            category_section,
            container(text("")).height(Length::Fill),
            action_section,
        ]
        .spacing(8)
        .padding(16)
        .width(200);

        container(sidebar_content)
            .style(|_theme| container::Style {
                background: Some(color!(0x1a1a2e).into()),
                ..Default::default()
            })
            .height(Fill)
            .into()
    }

    fn view_category_button(&self, category: Category) -> Element<'_, Message> {
        let is_selected = self.selected_category == category;
        let app_count = self.category_count(&category);

        let indicator = container(text(""))
            .width(4)
            .height(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(
                    if is_selected {
                        color!(0x6f6bff)
                    } else {
                        iced::Color::TRANSPARENT
                    }
                    .into(),
                ),
                ..Default::default()
            });

        let label = text(format!("{} {}", category.icon(), category.label())).size(14);
        let count_badge = container(
            text(app_count.to_string())
                .size(12)
                .color(color!(0xe6e6ff)),
        )
        .padding([2, 8])
        .style(|_theme| container::Style {
            background: Some(color!(0x2f2f55).into()),
            border: iced::Border::default().rounded(12),
            ..Default::default()
        });

        let content = row![
            indicator,
            container(label).padding([0, 8, 0, 10]),
            container(text("")).width(Length::Fill),
            count_badge,
        ]
        .align_y(iced::Alignment::Center)
        .spacing(6);

        let btn = button(content)
            .on_press(Message::CategorySelected(category))
            .padding([8, 8])
            .width(Fill)
            .style(move |theme, status| {
                if is_selected {
                    button::Style {
                        background: Some(color!(0x2e2e52).into()),
                        text_color: color!(0xffffff),
                        border: iced::Border::default().rounded(6),
                        ..button::primary(theme, status)
                    }
                } else {
                    button::Style {
                        background: Some(iced::Color::TRANSPARENT.into()),
                        text_color: color!(0x888899),
                        border: iced::Border::default().rounded(6),
                        ..button::secondary(theme, status)
                    }
                }
            });

        btn.into()
    }

    fn category_count(&self, category: &Category) -> usize {
        self.applications
            .iter()
            .filter(|app| category.matches(&app.category))
            .count()
    }

    fn view_content(&self) -> Element<'_, Message> {
        let search = text_input("Search applications...", &self.search_query)
            .on_input(Message::SearchChanged)
            .padding(12)
            .size(16)
            .width(Fill);

        let status = text(&self.status_message)
            .size(12)
            .color(color!(0x888899));

        let header = row![search, status]
            .spacing(16)
            .align_y(iced::Alignment::Center);

        let app_grid = self.view_app_grid();

        let content = column![
            header,
            container(text("")).height(16),
            app_grid
        ]
        .spacing(8)
        .padding(24)
        .width(Fill);

        container(content)
            .style(|_theme| container::Style {
                background: Some(color!(0x0f0f1a).into()),
                ..Default::default()
            })
            .width(Fill)
            .height(Fill)
            .into()
    }

    fn view_app_grid(&self) -> Element<'_, Message> {
        let filtered: Vec<&Application> = self.filtered_applications();

        if filtered.is_empty() {
            return container(
                text("No applications found")
                    .size(16)
                    .color(color!(0x666677)),
            )
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill)
            .into();
        }

        let mut rows: Vec<Element<'_, Message>> = Vec::new();

        for chunk in filtered.chunks(4) {
            let mut row_items: Vec<Element<'_, Message>> = Vec::new();

            for app in chunk {
                row_items.push(Self::view_app_card(app));
            }

            while row_items.len() < 4 {
                row_items.push(container(column![]).width(Fill).into());
            }

            rows.push(Row::with_children(row_items).spacing(12).into());
        }

        let grid = Column::with_children(rows).spacing(12);

        scrollable(grid).height(Fill).into()
    }

    fn view_app_card(app: &Application) -> Element<'_, Message> {
        let icon_char = app.name.chars().next().unwrap_or('?').to_uppercase().next().unwrap_or('?');

        let icon = text(icon_char.to_string())
            .size(32)
            .font(Font::MONOSPACE)
            .color(color!(0x8888ff));

        let name = text(app.name.clone())
            .size(13)
            .color(color!(0xffffff));

        let card_content = column![
            container(icon)
                .width(Fill)
                .center_x(Fill),
            container(text("")).height(8),
            container(name)
                .width(Fill)
                .center_x(Fill),
        ]
        .spacing(4)
        .padding(16)
        .width(Fill);

        let exec = app.exec.clone();
        button(card_content)
            .on_press(Message::LaunchApp(exec))
            .padding(0)
            .width(Fill)
            .style(|_theme, status| {
                let bg = match status {
                    button::Status::Hovered => color!(0x2a2a4e),
                    button::Status::Pressed => color!(0x3a3a5e),
                    _ => color!(0x1a1a2e),
                };
                button::Style {
                    background: Some(bg.into()),
                    text_color: color!(0xffffff),
                    border: iced::Border::default().rounded(12),
                    ..Default::default()
                }
            })
            .into()
    }

    fn filtered_applications(&self) -> Vec<&Application> {
        let query = self.search_query.to_lowercase();
        self.applications
            .iter()
            .filter(|app| {
                // Category filter
                if !self.selected_category.matches(&app.category) {
                    return false;
                }
                // Search filter
                if query.is_empty() {
                    return true;
                }
                app.name.to_lowercase().contains(&query)
            })
            .collect()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}
