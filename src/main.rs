mod scan;

use iced::widget::{
    button, column, container, row, scrollable, text, text_input, Column, Row,
};
use iced::font::Weight;
use iced::{color, Element, Fill, Font, Length, Theme};

use scan::{AppCategory, AppOrigin, Application};

pub fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .font(include_bytes!(
            "ui/assets/fonts/JetBrainsMonoNerdFont/JetBrainsMonoNerdFont-Regular.ttf"
        ))
        .font(include_bytes!(
            "ui/assets/fonts/JetBrainsMonoNerdFont/JetBrainsMonoNerdFont-Medium.ttf"
        ))
        .font(include_bytes!(
            "ui/assets/fonts/JetBrainsMonoNerdFont/JetBrainsMonoNerdFont-Bold.ttf"
        ))
        .default_font(app_font())
        .window_size((1000.0, 700.0))
        .run()
}

const APP_FONT_NAME: &str = "JetBrainsMono Nerd Font";

fn app_font() -> Font {
    Font::with_name(APP_FONT_NAME)
}

fn app_font_with_weight(weight: Weight) -> Font {
    Font { weight, ..app_font() }
}

#[derive(Debug, Clone, PartialEq)]
enum Category {
    All,
    Windows,
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
            Category::Windows => "Windows",
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
            Category::All => "\u{f00a}",
            Category::Windows => "\u{f17a}",
            Category::Development => "\u{f121}",
            Category::Graphics => "\u{f1fc}",
            Category::Network => "\u{f0ac}",
            Category::Office => "\u{f0f6}",
            Category::Multimedia => "\u{f008}",
            Category::System => "\u{f085}",
            Category::Utility => "\u{f0ad}",
            Category::Game => "\u{f11b}",
            Category::Other => "\u{f128}",
        }
    }

    fn matches(&self, app_category: &AppCategory) -> bool {
        match self {
            Category::All => true,
            Category::Windows => false,
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
            .size(30)
            .font(app_font_with_weight(Weight::Bold));

        let category_header = text("Catégories")
            .size(13)
            .font(app_font())
            .color(color!(0x8a8aa3));

        let categories = [
            Category::All,
            Category::Windows,
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

        let rescan_btn = button(text("Rescan").size(13).font(app_font()))
            .on_press(Message::Rescan)
            .padding([8, 16])
            .width(Fill);

        let sidebar_content = column![
            title,
            container(text("")).height(24),
            category_header,
            category_list,
            container(text("")).height(Length::Fill),
            rescan_btn,
        ]
        .spacing(10)
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

        let text_color = if is_selected {
            color!(0xffffff)
        } else {
            color!(0x9a9ab5)
        };

        let indicator = container(text(""))
            .width(4)
            .height(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(
                    if is_selected {
                        color!(0x6b6bd6)
                    } else {
                        iced::Color::TRANSPARENT
                    }
                    .into(),
                ),
                ..Default::default()
            });

        let icon = text(category.icon())
            .size(15)
            .font(app_font())
            .color(text_color);

        let label = text(category.label())
            .size(14)
            .font(app_font())
            .color(text_color);

        let content = row![indicator, icon, label]
            .spacing(10)
            .align_y(iced::Alignment::Center);

        let btn = button(content)
            .on_press(Message::CategorySelected(category))
            .padding([10, 14])
            .width(Fill)
            .style(move |theme, status| {
                if is_selected {
                    button::Style {
                        background: Some(color!(0x3a3a5e).into()),
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

    fn view_content(&self) -> Element<'_, Message> {
        let search = text_input("Search applications...", &self.search_query)
            .on_input(Message::SearchChanged)
            .padding(12)
            .size(16)
            .width(Fill);

        let status = text(&self.status_message)
            .size(12)
            .font(app_font())
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
            .font(app_font_with_weight(Weight::Medium))
            .color(color!(0x8888ff));

        let name = text(app.name.clone())
            .size(14)
            .font(app_font())
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
                match self.selected_category {
                    Category::Windows => {
                        if app.origin != AppOrigin::Windows {
                            return false;
                        }
                    }
                    _ => {
                        if app.origin == AppOrigin::Windows {
                            return false;
                        }
                        if !self.selected_category.matches(&app.category) {
                            return false;
                        }
                    }
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
